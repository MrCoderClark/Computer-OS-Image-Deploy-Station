use eframe::egui;
use crate::app::{AgentApp, ConnectionStatus};
use super::theme;

pub fn show(ctx: &egui::Context, app: &AgentApp) {
    egui::TopBottomPanel::top("header")
        .frame(egui::Frame::NONE.fill(theme::SURFACE).inner_margin(egui::Margin::symmetric(16, 10)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(mode) = app.mode() {
                    let color = theme::mode_color(mode);
                    let mode_text = egui::RichText::new(format!("◆ {}", mode.to_string().to_uppercase()))
                        .color(color)
                        .strong()
                        .size(12.0);
                    ui.label(mode_text);
                } else {
                    ui.label(
                        egui::RichText::new("NO CONFIG")
                            .color(theme::WARNING)
                            .strong()
                            .size(12.0),
                    );
                }

                ui.separator();

                ui.label(
                    egui::RichText::new(&app.machine_name)
                        .color(theme::TEXT)
                        .strong()
                        .size(12.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (dot_color, label) = match app.connection {
                        ConnectionStatus::Connected => (theme::SUCCESS, app.connection.label()),
                        ConnectionStatus::Connecting => (theme::WARNING, app.connection.label()),
                        ConnectionStatus::Unreachable => (theme::ERROR, app.connection.label()),
                    };

                    ui.label(egui::RichText::new(label).color(dot_color).size(11.0));
                    let dot_rect = ui.allocate_space(egui::vec2(8.0, 8.0));
                    ui.painter().circle_filled(dot_rect.1.center(), 4.0, dot_color);
                });
            });
        });
}
