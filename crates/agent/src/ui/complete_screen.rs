use eframe::egui;
use crate::app::{AgentApp, Screen};
use super::theme;

pub fn show(ctx: &egui::Context, app: &mut AgentApp) {
    theme::setup_visuals(ctx);
    super::header::show(ctx, app);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::BG).inner_margin(egui::Margin::symmetric(32, 24)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(60.0);

                ui.label(
                    egui::RichText::new("✓")
                        .size(48.0)
                        .color(theme::SUCCESS)
                        .strong(),
                );
                ui.add_space(12.0);

                ui.label(theme::heading("Operation Complete"));
                ui.add_space(8.0);
                ui.label(theme::body("The machine is ready to reboot into Windows."));
                ui.add_space(24.0);

                if ui.button(egui::RichText::new("Return to Main Screen").size(13.0)).clicked() {
                    app.screen = Screen::Main;
                }
            });
        });
}
