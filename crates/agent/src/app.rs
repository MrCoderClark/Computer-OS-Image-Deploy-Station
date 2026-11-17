use imaging_common::config::{OperatingMode, SuiteConfig};

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

pub struct AgentApp {
    pub config: Option<SuiteConfig>,
    pub screen: Screen,
    pub connection: ConnectionStatus,
    pub machine_name: String,
    pub error_message: Option<String>,
    pub confirm_text: String,
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

        Self {
            config,
            screen: Screen::Main,
            connection,
            machine_name,
            error_message: None,
            confirm_text: String::new(),
        }
    }

    pub fn mode(&self) -> Option<OperatingMode> {
        self.config.as_ref().map(|c| c.mode)
    }
}

impl eframe::App for AgentApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
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
