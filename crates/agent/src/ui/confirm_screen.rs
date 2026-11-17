use eframe::egui;
use crate::app::{AgentApp, Screen};
use super::theme;

pub fn show(ctx: &egui::Context, app: &mut AgentApp, is_deploy: bool) {
    theme::setup_visuals(ctx);
    super::header::show(ctx, app);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::BG).inner_margin(egui::Margin::symmetric(32, 24)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);

                egui::Frame::NONE
                    .fill(theme::SURFACE)
                    .stroke(egui::Stroke::new(2.0_f32, theme::ERROR))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(20, 20))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width().min(420.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new("!")
                                    .size(28.0)
                                    .color(theme::ERROR)
                                    .strong(),
                            );
                            ui.add_space(8.0);

                            let title = if is_deploy {
                                "Erase disk and apply image?"
                            } else {
                                "Capture image from this computer?"
                            };
                            ui.label(theme::heading(title));
                            ui.add_space(12.0);

                            if is_deploy {
                                ui.label(theme::body("Target disk: Disk 0"));
                                if let Some(config) = &app.config {
                                    if let Some(image) = &config.image {
                                        ui.label(theme::body(&format!("Image: {}", image.name)));
                                    }
                                }
                                ui.label(theme::body("Partitions: ESP (300 MB) + MSR (16 MB) + Windows"));
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("All existing data on Disk 0 will be permanently erased.")
                                        .color(theme::ERROR)
                                        .size(12.0)
                                        .strong(),
                                );
                            } else {
                                ui.label(theme::body(&format!("Source: {}", app.machine_name)));
                                if let Some(config) = &app.config {
                                    if let Some(target) = &config.capture_target {
                                        ui.label(theme::body(&format!("Destination: {}", target.unc_path)));
                                    }
                                }
                            }

                            ui.add_space(16.0);

                            let confirm_word = if is_deploy { "DEPLOY" } else { "CAPTURE" };
                            ui.label(theme::caption(&format!("Type {} to confirm", confirm_word)));

                            ui.add(
                                egui::TextEdit::singleline(&mut app.confirm_text)
                                    .desired_width(200.0)
                                    .horizontal_align(egui::Align::Center),
                            );

                            let confirmed = app.confirm_text.trim().eq_ignore_ascii_case(confirm_word);

                            ui.add_space(12.0);

                            ui.horizontal(|ui| {
                                if ui.button(
                                    egui::RichText::new("Cancel").size(13.0),
                                ).clicked() {
                                    app.confirm_text.clear();
                                    app.screen = Screen::Main;
                                }

                                let btn = egui::Button::new(
                                    egui::RichText::new(if is_deploy { "Erase & Deploy" } else { "Start Capture" })
                                        .size(13.0)
                                        .color(if confirmed { egui::Color32::WHITE } else { theme::TEXT_TERTIARY }),
                                );

                                let btn = if confirmed {
                                    btn.fill(theme::ERROR)
                                } else {
                                    btn.fill(theme::SURFACE)
                                };

                                let resp = ui.add_enabled(confirmed, btn);
                                if resp.clicked() {
                                    app.confirm_text.clear();
                                    if is_deploy {
                                        let wim_path = std::path::PathBuf::from(r"X:\imaging\install.wim");
                                        app.start_deploy(0, wim_path, 1);
                                    } else {
                                        let unc = app.config.as_ref()
                                            .and_then(|c| c.capture_target.as_ref())
                                            .map(|t| t.unc_path.clone())
                                            .unwrap_or_else(|| r"\\server\captures".into());
                                        let output = std::path::PathBuf::from(unc)
                                            .join(format!("{}.wim", app.machine_name));
                                        app.start_capture("C:".into(), output);
                                    }
                                }
                            });
                        });
                    });
            });
        });
}
