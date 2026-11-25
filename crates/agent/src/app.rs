use imaging_common::api::ImageSummary;
use imaging_common::config::{OperatingMode, SuiteConfig};
use imaging_common::types::DeployStatus;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub enum Screen {
    Main,
    ConfirmDeploy,
    ConfirmCapture,
    Progress,
    Complete,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSource {
    Config,
    Server,
    UsbFallback,
}

pub enum NetworkEvent {
    Connected,
    Unreachable(String),
    ImagesLoaded(Vec<ImageSummary>),
    #[allow(dead_code)]
    Reconnecting { attempt: u8 },
    #[allow(dead_code)]
    Reconnected,
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
    pub deploy_started: Option<Instant>,
    pub log_lines: Vec<String>,
    pub log_expanded: bool,

    pub image_source: ImageSource,
    pub using_offline_fallback: bool,

    pub reconnecting: bool,
    pub reconnect_attempt: u8,

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

        let image_source = match &config {
            Some(c) if c.image.is_some() => ImageSource::Config,
            _ => ImageSource::Server,
        };

        if connection == ConnectionStatus::Connecting {
            let server_config = config.as_ref().and_then(|c| c.server.clone());
            let enrollment_key = config.as_ref().and_then(|c| c.enrollment_key.clone());
            let name = machine_name.clone();
            let connect_tx = net_tx.clone();
            rt.spawn(async move {
                connect_to_server(server_config, enrollment_key.as_deref(), &name, connect_tx).await;
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
            deploy_started: None,
            log_lines: Vec::new(),
            log_expanded: false,
            image_source,
            using_offline_fallback: false,
            reconnecting: false,
            reconnect_attempt: 0,
            net_rx,
            net_tx,
            runtime: rt,
        }
    }

    pub fn mode(&self) -> Option<OperatingMode> {
        self.config.as_ref().map(|c| c.mode)
    }

    pub fn elapsed_time(&self) -> Option<std::time::Duration> {
        self.deploy_started.map(|s| s.elapsed())
    }

    pub fn formatted_elapsed(&self) -> String {
        match self.elapsed_time() {
            Some(d) => {
                let secs = d.as_secs();
                let mins = secs / 60;
                let secs = secs % 60;
                if mins > 0 {
                    format!("{}m {}s elapsed", mins, secs)
                } else {
                    format!("{}s elapsed", secs)
                }
            }
            None => String::new(),
        }
    }

    pub fn resolve_deploy_provider(&self) -> Result<(std::path::PathBuf, u32, bool), String> {
        let config = self.config.as_ref().ok_or("No configuration loaded")?;
        let provider = imaging_engine::provider::create_provider(config)
            .map_err(|e| format!("Failed to create image provider: {e}"))?;
        Ok((
            provider.wim_path().to_path_buf(),
            provider.image_index(),
            provider.needs_download(),
        ))
    }

    /// Pick the server-catalog image this agent should deploy. Prefers a
    /// sha256 match against the suite config's baked-in metadata (built
    /// at USB-build time from the same catalog), then a name match. Falls
    /// back to the first catalog entry with a loud log if neither is set.
    fn match_server_image(&self) -> Option<ImageSummary> {
        let image = self.config.as_ref().and_then(|c| c.image.as_ref());
        if let Some(sha) = image.and_then(|i| i.sha256.as_deref()) {
            if let Some(hit) = self
                .server_images
                .iter()
                .find(|s| s.sha256.eq_ignore_ascii_case(sha))
            {
                return Some(hit.clone());
            }
        }
        if let Some(name) = image.map(|i| i.name.as_str()) {
            if let Some(hit) = self.server_images.iter().find(|s| s.name == name) {
                return Some(hit.clone());
            }
        }
        if let Some(first) = self.server_images.first() {
            tracing::warn!(
                "No sha256/name match for suite image in server catalog; falling back to first entry ({})",
                first.name
            );
            return Some(first.clone());
        }
        None
    }

    pub fn start_deploy(
        &mut self,
        target_disk: u32,
        wim_path: std::path::PathBuf,
        image_index: u32,
        needs_download: bool,
    ) {
        let job_id = uuid::Uuid::new_v4();
        let tx = self.net_tx.clone();
        self.deploy_status = Some(DeployStatus::Pending);
        self.deploy_progress = 0;
        self.deploy_message = if needs_download {
            "Preparing download...".into()
        } else {
            "Starting deploy...".into()
        };
        self.deploy_started = Some(Instant::now());
        self.log_lines.clear();
        self.log_expanded = false;
        self.screen = Screen::Progress;

        // Everything the download task needs. Resolved before the move so
        // the UI thread can surface "no server configured" / "no catalog
        // match" errors with the same shape as a download failure.
        let download_ctx = if needs_download {
            let matched = match self.match_server_image() {
                Some(m) => m,
                None => {
                    let _ = tx.send(NetworkEvent::PipelineError(
                        "Cannot download image: server catalog has no matching entry. Rebuild the USB with a populated image, or verify the server has the image loaded.".into(),
                    ));
                    return;
                }
            };
            let server = match self.config.as_ref().and_then(|c| c.server.clone()) {
                Some(sc) => sc,
                None => {
                    let _ = tx.send(NetworkEvent::PipelineError(
                        "Cannot download image: suite config has no [server] section.".into(),
                    ));
                    return;
                }
            };
            Some(DownloadContext {
                image_id: matched.id,
                expected_sha256: matched.sha256,
                size_bytes: matched.size_bytes,
                server,
                enrollment_key: self.config.as_ref().and_then(|c| c.enrollment_key.clone()),
                machine_name: self.machine_name.clone(),
            })
        } else {
            None
        };

        let request = imaging_engine::pipeline::DeployRequest {
            job_id,
            target_disk,
            wim_path: wim_path.clone(),
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

            if let Some(ctx) = download_ctx {
                if let Err(e) = download_before_deploy(&ctx, &wim_path, tx.clone()).await {
                    // Include the chain so the UI shows the real cause (e.g. a
                    // reqwest I/O error like "no space left on device" from
                    // WinPE's RAM scratch filling up), not just the outer context.
                    let _ = tx.send(NetworkEvent::PipelineError(format!("Download failed: {e:#}")));
                    let _ = forward_handle.await;
                    return;
                }
            }

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
        self.deploy_started = Some(Instant::now());
        self.log_lines.clear();
        self.log_expanded = false;
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
                    self.reconnecting = false;
                    self.reconnect_attempt = 0;
                }
                NetworkEvent::Unreachable(reason) => {
                    tracing::warn!("Server unreachable: {reason}");
                    self.connection = ConnectionStatus::Unreachable;

                    if self.mode() == Some(OperatingMode::Hybrid) && self.screen == Screen::Main {
                        self.using_offline_fallback = true;
                        self.image_source = ImageSource::UsbFallback;
                    }
                }
                NetworkEvent::ImagesLoaded(images) => {
                    tracing::info!("Loaded {} images from server", images.len());
                    self.server_images = images;
                    if self.image_source == ImageSource::Server {
                        self.image_source = ImageSource::Server;
                    }
                }
                NetworkEvent::Reconnecting { attempt } => {
                    tracing::info!("Reconnection attempt {attempt}");
                    self.reconnecting = true;
                    self.reconnect_attempt = attempt;
                    self.connection = ConnectionStatus::Connecting;
                }
                NetworkEvent::Reconnected => {
                    tracing::info!("Reconnected to server");
                    self.reconnecting = false;
                    self.reconnect_attempt = 0;
                    self.connection = ConnectionStatus::Connected;
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
            || self.screen == Screen::Progress
            || self.reconnecting;
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

struct DownloadContext {
    image_id: uuid::Uuid,
    expected_sha256: String,
    size_bytes: u64,
    server: imaging_common::config::ServerConfig,
    enrollment_key: Option<String>,
    machine_name: String,
}

async fn download_before_deploy(
    ctx: &DownloadContext,
    dest: &std::path::Path,
    tx: mpsc::UnboundedSender<NetworkEvent>,
) -> anyhow::Result<()> {
    use anyhow::Context;
    let host = ctx.server.host.as_deref().unwrap_or("127.0.0.1");
    let port = ctx.server.port;
    let cert_pem = ctx.server.cert_pem.as_deref();

    let mut client = crate::client::ServerClient::new(host, port, cert_pem);
    client
        .authenticate(&ctx.machine_name, ctx.enrollment_key.as_deref())
        .await
        .context("authenticate for download")?;

    // Throttle progress to one event per whole-percent change so we don't
    // flood the UI channel with 100k+ events over a multi-GB download.
    let last_pct = std::sync::Arc::new(std::sync::atomic::AtomicI8::new(-1));
    let last_pct_cb = last_pct.clone();
    let tx_cb = tx.clone();
    let total_from_catalog = ctx.size_bytes;

    let progress = move |done: u64, total: u64| {
        // Prefer catalog-advertised size if the stream hasn't reported one yet.
        let total = if total > 0 { total } else { total_from_catalog };
        let pct = if total > 0 {
            ((done.saturating_mul(100)) / total).min(100) as i8
        } else {
            0
        };
        let prev = last_pct_cb.load(std::sync::atomic::Ordering::Relaxed);
        if pct == prev { return; }
        last_pct_cb.store(pct, std::sync::atomic::Ordering::Relaxed);

        let msg = if total > 0 {
            format!(
                "Downloading image {:.1} / {:.1} MB",
                done as f64 / (1024.0 * 1024.0),
                total as f64 / (1024.0 * 1024.0),
            )
        } else {
            format!("Downloading image ({} bytes)", done)
        };
        let _ = tx_cb.send(NetworkEvent::PipelineProgress {
            status: DeployStatus::Pending,
            progress_pct: pct.max(0) as u8,
            message: msg,
        });
    };

    // Kick off with an initial 0% so the UI switches from "Preparing download..."
    // to "Downloading image ..." immediately, even before the first byte lands.
    let _ = tx.send(NetworkEvent::PipelineProgress {
        status: DeployStatus::Pending,
        progress_pct: 0,
        message: "Downloading image...".into(),
    });

    client
        .download_image(
            ctx.image_id,
            dest,
            Some(&ctx.expected_sha256),
            progress,
        )
        .await
        .with_context(|| format!("download image {}", ctx.image_id))?;

    let _ = tx.send(NetworkEvent::PipelineProgress {
        status: DeployStatus::Pending,
        progress_pct: 100,
        message: "Image downloaded and verified".into(),
    });

    Ok(())
}

async fn connect_to_server(
    server_config: Option<imaging_common::config::ServerConfig>,
    enrollment_key: Option<&str>,
    machine_name: &str,
    tx: mpsc::UnboundedSender<NetworkEvent>,
) {
    let (host, port, cert_pem) = match &server_config {
        Some(sc) => (
            sc.host.as_deref().unwrap_or("127.0.0.1"),
            sc.port,
            sc.cert_pem.as_deref(),
        ),
        None => ("127.0.0.1", 7443, None),
    };

    tracing::info!("Connecting to server at {host}:{port}...");

    let mut client = crate::client::ServerClient::new(host, port, cert_pem);

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

    match client.authenticate(machine_name, enrollment_key).await {
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
