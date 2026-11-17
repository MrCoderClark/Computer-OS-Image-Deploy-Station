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
                    egui::RichText::new("✕")
                        .size(48.0)
                        .color(theme::ERROR)
                        .strong(),
                );
                ui.add_space(12.0);

                ui.label(theme::heading("Operation Failed"));
                ui.add_space(8.0);

                let msg = app
                    .error_message
                    .as_deref()
                    .unwrap_or("An unexpected error occurred.");
                ui.label(theme::body(msg));

                ui.add_space(24.0);

                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new("Retry").size(13.0)).clicked() {
                        app.error_message = None;
                        app.screen = Screen::Main;
                    }
                    if ui.button(egui::RichText::new("View Log").size(13.0)).clicked() {
                        // TODO: open log viewer
                    }
                });
            });
        });
}
