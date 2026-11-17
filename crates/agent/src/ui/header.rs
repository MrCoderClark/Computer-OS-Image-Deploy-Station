use eframe::egui;
use crate::app::AgentApp;
use super::theme;

pub fn show(ctx: &egui::Context, app: &AgentApp) {
    egui::TopBottomPanel::top("header")
        .frame(
            egui::Frame::NONE
                .fill(theme::SURFACE)
                .stroke(egui::Stroke::new(1.0_f32, theme::SURFACE_BORDER))
                .inner_margin(egui::Margin::symmetric(16, 10)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(mode) = app.mode() {
                    let color = if app.using_offline_fallback {
                        theme::WARNING
                    } else {
                        theme::mode_color(mode)
                    };
                    let mode_label = if app.using_offline_fallback {
                        format!("◆ {} (OFFLINE)", mode.to_string().to_uppercase())
                    } else {
                        format!("◆ {}", mode.to_string().to_uppercase())
                    };
                    ui.label(
                        egui::RichText::new(mode_label)
                            .color(color)
                            .strong()
                            .size(13.0),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("◆ NO CONFIG")
                            .color(theme::WARNING)
                            .strong()
                            .size(13.0),
                    );
                }

                ui.separator();

                ui.label(
                    egui::RichText::new(&app.machine_name)
                        .color(theme::TEXT)
                        .strong()
                        .size(13.0),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let dot_color = theme::connection_color(&app.connection);
                    let label = app.connection.label();

                    ui.label(egui::RichText::new(label).color(dot_color).size(12.0));
                    ui.add_space(4.0);

                    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, dot_color);
                });
            });
        });
}
