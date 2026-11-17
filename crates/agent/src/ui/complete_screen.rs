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
                    egui::RichText::new("✓")
                        .size(64.0)
                        .color(theme::SUCCESS)
                        .strong(),
                );
                ui.add_space(16.0);

                ui.label(theme::display("Operation Complete"));
                ui.add_space(12.0);

                ui.label(theme::body("The machine is ready to reboot into Windows."));

                if let Some(elapsed) = app.elapsed_time() {
                    ui.add_space(8.0);
                    let secs = elapsed.as_secs();
                    let mins = secs / 60;
                    let secs = secs % 60;
                    let time_str = if mins > 0 {
                        format!("Completed in {}m {}s", mins, secs)
                    } else {
                        format!("Completed in {}s", secs)
                    };
                    ui.label(
                        egui::RichText::new(time_str)
                            .size(13.0)
                            .color(theme::TEXT_TERTIARY),
                    );
                }

                ui.add_space(32.0);

                let btn = theme::primary_button("Return to Main Screen");
                if ui.add(btn).clicked() {
                    app.deploy_started = None;
                    app.screen = Screen::Main;
                }
            });
        });
}
