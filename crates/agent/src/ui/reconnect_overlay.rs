use eframe::egui;
use crate::app::{AgentApp, Screen};
use super::theme;

const MAX_RECONNECT_ATTEMPTS: u8 = 3;

pub fn show(ctx: &egui::Context, app: &mut AgentApp) {
    let screen_rect = ctx.screen_rect();

    egui::Area::new(egui::Id::new("reconnect_dim"))
        .fixed_pos(screen_rect.left_top())
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen_rect.size(), egui::Sense::click());
            ui.painter().rect_filled(
                rect,
                0.0,
                egui::Color32::from_black_alpha(180),
            );
        });

    let exceeded = app.reconnect_attempt >= MAX_RECONNECT_ATTEMPTS;

    egui::Window::new("Reconnecting")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_size(egui::vec2(380.0, if exceeded { 220.0 } else { 170.0 }))
        .frame(
            egui::Frame::NONE
                .fill(theme::SURFACE)
                .stroke(egui::Stroke::new(2.0_f32, theme::WARNING))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(24, 20)),
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("⟳")
                        .size(28.0)
                        .color(theme::WARNING)
                        .strong(),
                );
                ui.add_space(8.0);

                ui.label(
                    egui::RichText::new("Reconnecting...")
                        .size(18.0)
                        .color(theme::WARNING)
                        .strong(),
                );
                ui.add_space(4.0);

                ui.label(
                    egui::RichText::new(format!(
                        "Attempt {} of {}",
                        app.reconnect_attempt, MAX_RECONNECT_ATTEMPTS
                    ))
                    .size(13.0)
                    .color(theme::TEXT_SECONDARY),
                );
                ui.add_space(4.0);

                ui.label(
                    egui::RichText::new("Progress is paused. Transfer will resume on reconnect.")
                        .size(12.0)
                        .color(theme::TEXT_TERTIARY),
                );

                if exceeded {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let retry = theme::primary_button("Retry");
                        if ui.add(retry).clicked() {
                            app.reconnecting = false;
                            app.reconnect_attempt = 0;
                        }

                        ui.add_space(8.0);

                        let cancel = theme::secondary_button("Cancel");
                        if ui.add(cancel).clicked() {
                            app.reconnecting = false;
                            app.reconnect_attempt = 0;
                            app.error_message = Some(
                                "Operation cancelled — network connection lost after 3 reconnection attempts.".into(),
                            );
                            app.screen = Screen::Error;
                        }
                    });
                }
            });
        });
}
