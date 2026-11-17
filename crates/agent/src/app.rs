use imaging_common::api::ImageSummary;
use imaging_common::config::{OperatingMode, SuiteConfig};
use imaging_common::types::DeployStatus;
use std::sync::Arc;
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
    PipelineProgress {
        status: DeployStatus,
        progress_pct: u8,
        message: String,
    },
    PipelineComplete,
    PipelineError(String),
}

pub struct AgentApp {
    pub config: Option<SuiteConfig>,
    pub screen: Screen,
    pub connection: ConnectionStatus,
    pub machine_name: String,
    pub error_message: Option<String>,
    pub confirm_text: String,
    pub server_images: Vec<ImageSummary>,
    pub deploy_status: Option<DeployStatus>,
    pub deploy_progress: u8,
    pub deploy_message: String,
    pub log_lines: Vec<String>,
    net_rx: mpsc::UnboundedReceiver<NetworkEvent>,
    net_tx: mpsc::UnboundedSender<NetworkEvent>,
    runtime: Arc<tokio::runtime::Runtime>,
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
            let connect_tx = net_tx.clone();
            rt.spawn(async move {
                connect_to_server(server_config, &name, connect_tx).await;
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
            deploy_status: None,
            deploy_progress: 0,
            deploy_message: String::new(),
            log_lines: Vec::new(),
            net_rx,
            net_tx,
            runtime: rt,
        }
    }

    pub fn mode(&self) -> Option<OperatingMode> {
        self.config.as_ref().map(|c| c.mode)
    }

    pub fn resolve_deploy_provider(&self) -> Result<(std::path::PathBuf, u32), String> {
        let config = self.config.as_ref().ok_or("No configuration loaded")?;
        let provider = imaging_engine::provider::create_provider(config)
            .map_err(|e| format!("Failed to create image provider: {e}"))?;
        Ok((provider.wim_path().to_path_buf(), provider.image_index()))
    }

    pub fn start_deploy(&mut self, target_disk: u32, wim_path: std::path::PathBuf, image_index: u32) {
        let job_id = uuid::Uuid::new_v4();
        let tx = self.net_tx.clone();
        self.deploy_status = Some(DeployStatus::Pending);
        self.deploy_progress = 0;
        self.deploy_message = "Starting deploy...".into();
        self.log_lines.clear();
        self.screen = Screen::Progress;

        let request = imaging_engine::pipeline::DeployRequest {
            job_id,
            target_disk,
            wim_path,
            image_index,
        };

        self.runtime.spawn(async move {
            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<imaging_engine::pipeline::PipelineProgress>(32);

            let pipeline_tx = tx.clone();
            let forward_handle = tokio::spawn(async move {
                while let Some(p) = progress_rx.recv().await {
                    let _ = pipeline_tx.send(NetworkEvent::PipelineProgress {
                        status: p.status,
                        progress_pct: p.progress_pct,
                        message: p.message,
                    });
                }
            });

            match imaging_engine::pipeline::run_deploy(request, progress_tx).await {
                Ok(()) => {
                    let _ = tx.send(NetworkEvent::PipelineComplete);
                }
                Err(e) => {
                    let _ = tx.send(NetworkEvent::PipelineError(e.to_string()));
                }
            }

            let _ = forward_handle.await;
        });
    }

    pub fn start_capture(&mut self, source_volume: String, output_wim: std::path::PathBuf) {
        let job_id = uuid::Uuid::new_v4();
        let tx = self.net_tx.clone();
        self.deploy_status = Some(DeployStatus::Pending);
        self.deploy_progress = 0;
        self.deploy_message = "Starting capture...".into();
        self.log_lines.clear();
        self.screen = Screen::Progress;

        let capture_target = self.config.as_ref().and_then(|c| c.capture_target.clone());
        let image_name = self.config.as_ref()
            .and_then(|c| c.image.as_ref())
            .map(|i| i.name.clone())
            .unwrap_or_else(|| format!("capture-{}", self.machine_name));
        let os_family = self.config.as_ref()
            .and_then(|c| c.image.as_ref())
            .map(|i| i.os_family)
            .unwrap_or(imaging_common::config::OsFamily::Win11);
        let log_dir = std::path::PathBuf::from(r"X:\imaging\logs");

        let request = imaging_engine::pipeline::CaptureRequest {
            job_id,
            source_volume,
            output_wim,
            machine_name: self.machine_name.clone(),
            image_name,
            os_family,
            capture_target,
            log_dir,
        };

        self.runtime.spawn(async move {
            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<imaging_engine::pipeline::PipelineProgress>(32);

            let pipeline_tx = tx.clone();
            let forward_handle = tokio::spawn(async move {
                while let Some(p) = progress_rx.recv().await {
                    let _ = pipeline_tx.send(NetworkEvent::PipelineProgress {
                        status: p.status,
                        progress_pct: p.progress_pct,
                        message: p.message,
                    });
                }
            });

            match imaging_engine::pipeline::run_capture(request, progress_tx).await {
                Ok(()) => {
                    let _ = tx.send(NetworkEvent::PipelineComplete);
                }
                Err(e) => {
                    let _ = tx.send(NetworkEvent::PipelineError(e.to_string()));
                }
            }

            let _ = forward_handle.await;
        });
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
                NetworkEvent::PipelineProgress { status, progress_pct, message } => {
                    self.deploy_status = Some(status);
                    self.deploy_progress = progress_pct;
                    self.deploy_message = message.clone();
                    self.log_lines.push(message);
                }
                NetworkEvent::PipelineComplete => {
                    self.deploy_status = Some(DeployStatus::Complete);
                    self.deploy_progress = 100;
                    self.screen = Screen::Complete;
                }
                NetworkEvent::PipelineError(err) => {
                    self.error_message = Some(err);
                    self.screen = Screen::Error;
                }
            }
        }
    }
}

impl eframe::App for AgentApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.poll_network();

        let needs_repaint = self.connection == ConnectionStatus::Connecting
            || self.screen == Screen::Progress;
        if needs_repaint {
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
