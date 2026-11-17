use anyhow::Result;
use eframe::egui;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("imaging_agent=debug")
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Windows Imaging Agent")
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Windows Imaging Agent",
        options,
        Box::new(|_cc| Ok(Box::new(AgentApp::default()))),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}

#[derive(Default)]
struct AgentApp {
    config: Option<imaging_common::config::SuiteConfig>,
}

impl eframe::App for AgentApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Windows Imaging Agent");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mode = self
                        .config
                        .as_ref()
                        .map(|c| c.mode.to_string())
                        .unwrap_or_else(|| "No config".into());
                    ui.label(mode);
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("Ready");
                ui.add_space(20.0);
                ui.label("Waiting for configuration...");
            });
        });
    }
}
