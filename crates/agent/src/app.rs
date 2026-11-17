use imaging_common::api::ImageSummary;
use imaging_common::config::{OperatingMode, SuiteConfig};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ConnectionStatus {
    Connected,
    Connecting,
    Unreachable,
}

impl ConnectionStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Connected => "Server connected",
            Self::Connecting => "Connecting...",
            Self::Unreachable => "Server unreachable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Screen {
    Main,
    ConfirmDeploy,
    ConfirmCapture,
    Progress,
    Complete,
    Error,
}

pub enum NetworkEvent {
    Connected,
    Unreachable(String),
    ImagesLoaded(Vec<ImageSummary>),
}

pub struct AgentApp {
    pub config: Option<SuiteConfig>,
    pub screen: Screen,
    pub connection: ConnectionStatus,
    pub machine_name: String,
    pub error_message: Option<String>,
    pub confirm_text: String,
    pub server_images: Vec<ImageSummary>,
    net_rx: mpsc::UnboundedReceiver<NetworkEvent>,
    _runtime: Arc<tokio::runtime::Runtime>,
}

impl AgentApp {
    pub fn new(config: Option<SuiteConfig>) -> Self {
        let connection = match &config {
            Some(c) => match c.mode {
                OperatingMode::Offline => ConnectionStatus::Unreachable,
                _ => ConnectionStatus::Connecting,
            },
            None => ConnectionStatus::Unreachable,
        };

        let machine_name = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "UNKNOWN".into());

        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime"),
        );

        let (net_tx, net_rx) = mpsc::unbounded_channel();

        if connection == ConnectionStatus::Connecting {
            let server_config = config.as_ref().and_then(|c| c.server.clone());
            let name = machine_name.clone();
            rt.spawn(async move {
                connect_to_server(server_config, &name, net_tx).await;
            });
        }

        Self {
            config,
            screen: Screen::Main,
            connection,
            machine_name,
            error_message: None,
            confirm_text: String::new(),
            server_images: Vec::new(),
            net_rx,
            _runtime: rt,
        }
    }

    pub fn mode(&self) -> Option<OperatingMode> {
        self.config.as_ref().map(|c| c.mode)
    }

    fn poll_network(&mut self) {
        while let Ok(event) = self.net_rx.try_recv() {
            match event {
                NetworkEvent::Connected => {
                    tracing::info!("Connected to server");
                    self.connection = ConnectionStatus::Connected;
                }
                NetworkEvent::Unreachable(reason) => {
                    tracing::warn!("Server unreachable: {reason}");
                    self.connection = ConnectionStatus::Unreachable;
                }
                NetworkEvent::ImagesLoaded(images) => {
                    tracing::info!("Loaded {} images from server", images.len());
                    self.server_images = images;
                }
            }
        }
    }
}

impl eframe::App for AgentApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.poll_network();

        if self.connection == ConnectionStatus::Connecting {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        match self.screen {
            Screen::Main => crate::ui::main_screen::show(ctx, self),
            Screen::ConfirmDeploy => crate::ui::confirm_screen::show(ctx, self, true),
            Screen::ConfirmCapture => crate::ui::confirm_screen::show(ctx, self, false),
            Screen::Progress => crate::ui::progress_screen::show(ctx, self),
            Screen::Complete => crate::ui::complete_screen::show(ctx, self),
            Screen::Error => crate::ui::error_screen::show(ctx, self),
        }
    }
}

async fn connect_to_server(
    server_config: Option<imaging_common::config::ServerConfig>,
    machine_name: &str,
    tx: mpsc::UnboundedSender<NetworkEvent>,
) {
    let (host, port) = match &server_config {
        Some(sc) => (
            sc.host.as_deref().unwrap_or("127.0.0.1"),
            sc.port,
        ),
        None => ("127.0.0.1", 7443),
    };

    tracing::info!("Connecting to server at {host}:{port}...");

    let mut client = crate::client::ServerClient::new(host, port);

    tracing::debug!("Checking server health...");
    match client.health().await {
        Ok(resp) => {
            tracing::info!("Server healthy: v{}", resp.version);
        }
        Err(e) => {
            tracing::error!("Health check failed: {e}");
            let _ = tx.send(NetworkEvent::Unreachable(e.to_string()));
            return;
        }
    }

    match client.authenticate(machine_name).await {
        Ok(resp) => {
            tracing::info!("Authenticated, token expires at {}", resp.expires_at);
            let _ = tx.send(NetworkEvent::Connected);
        }
        Err(e) => {
            let _ = tx.send(NetworkEvent::Unreachable(format!("Auth failed: {e}")));
            return;
        }
    }

    match client.list_images().await {
        Ok(images) => {
            let _ = tx.send(NetworkEvent::ImagesLoaded(images));
        }
        Err(e) => {
            tracing::error!("Failed to load images: {e}");
        }
    }
}
