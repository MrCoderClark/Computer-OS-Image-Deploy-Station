use eframe::egui;
use crate::app::{AgentApp, Screen};
use super::theme;

pub fn show(ctx: &egui::Context, app: &mut AgentApp) {
    theme::setup_visuals(ctx);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::BG).inner_margin(egui::Margin::symmetric(48, 48)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() * 0.15);

                ui.label(
                    egui::RichText::new("✕")
                        .size(64.0)
                        .color(theme::ERROR)
                        .strong(),
                );
                ui.add_space(16.0);

                ui.label(theme::display("Operation Failed"));
                ui.add_space(12.0);

                let msg = app
                    .error_message
                    .as_deref()
                    .unwrap_or("An unexpected error occurred.");

                let actionable = make_actionable(msg);

                egui::Frame::NONE
                    .fill(theme::ERROR_DIM)
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(20, 14))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width().min(500.0));
                        ui.label(
                            egui::RichText::new(&actionable)
                                .size(14.0)
                                .color(theme::ERROR),
                        );
                    });

                ui.add_space(32.0);

                ui.horizontal(|ui| {
                    let retry = theme::primary_button("Retry");
                    if ui.add(retry).clicked() {
                        app.error_message = None;
                        app.screen = Screen::Main;
                    }

                    ui.add_space(12.0);

                    let log_btn = theme::secondary_button("View Log");
                    if ui.add(log_btn).clicked() {
                        app.log_expanded = true;
                        app.screen = Screen::Progress;
                    }
                });

                if !app.log_lines.is_empty() {
                    ui.add_space(24.0);
                    ui.label(theme::label("LAST LOG ENTRIES"));
                    ui.add_space(4.0);

                    egui::Frame::NONE
                        .fill(theme::SURFACE)
                        .corner_radius(4.0)
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width().min(500.0));
                            let start = app.log_lines.len().saturating_sub(5);
                            for line in &app.log_lines[start..] {
                                ui.label(theme::mono(line));
                            }
                        });
                }
            });
        });
}

fn make_actionable(msg: &str) -> String {
    let lower = msg.to_lowercase();
    if lower.contains("connection") || lower.contains("reach") || lower.contains("network") {
        format!("{} — Check the network cable and verify the server address in your configuration.", msg)
    } else if lower.contains("disk") || lower.contains("locked") || lower.contains("partition") {
        format!("{} — The target disk may be in use. Try a different disk or reboot the machine.", msg)
    } else if lower.contains("wim") || lower.contains("image") || lower.contains("provider") {
        format!("{} — Verify the image file exists and the USB or network share is accessible.", msg)
    } else if lower.contains("auth") || lower.contains("token") || lower.contains("credential") {
        format!("{} — Authentication failed. Check the enrollment key or rebuild the USB media.", msg)
    } else {
        format!("{} — Try again, or check the log for details.", msg)
    }
}
