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
                ui.add_space(16.0);

                let border_color = if is_deploy { theme::ERROR } else { theme::ACCENT };
                egui::Frame::NONE
                    .fill(theme::SURFACE)
                    .stroke(egui::Stroke::new(2.0_f32, border_color))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(24, 24))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width().min(460.0));

                        ui.vertical_centered(|ui| {
                            let icon_color = if is_deploy { theme::ERROR } else { theme::ACCENT };
                            ui.label(
                                egui::RichText::new(if is_deploy { "!" } else { "↑" })
                                    .size(32.0)
                                    .color(icon_color)
                                    .strong(),
                            );
                            ui.add_space(8.0);

                            let title = if is_deploy {
                                "Erase disk and apply image?"
                            } else {
                                "Capture image from this computer?"
                            };
                            ui.label(theme::heading(title));
                        });

                        ui.add_space(16.0);

                        if is_deploy {
                            deploy_details(ui, app);
                        } else {
                            capture_details(ui, app);
                        }

                        ui.add_space(20.0);

                        let confirm_word = if is_deploy { "DEPLOY" } else { "CAPTURE" };
                        ui.label(
                            egui::RichText::new(format!("Type {} to confirm", confirm_word))
                                .size(12.0)
                                .color(theme::TEXT_TERTIARY),
                        );
                        ui.add_space(4.0);

                        ui.add(
                            egui::TextEdit::singleline(&mut app.confirm_text)
                                .desired_width(220.0)
                                .horizontal_align(egui::Align::Center)
                                .font(egui::FontId::new(16.0, egui::FontFamily::Monospace)),
                        );

                        let confirmed = app.confirm_text.trim().eq_ignore_ascii_case(confirm_word);

                        ui.add_space(16.0);

                        ui.horizontal(|ui| {
                            let cancel = theme::secondary_button("Cancel");
                            if ui.add(cancel).clicked() {
                                app.confirm_text.clear();
                                app.screen = Screen::Main;
                            }

                            ui.add_space(12.0);

                            let action_label = if is_deploy { "Erase & Deploy" } else { "Start Capture" };
                            let action_btn = theme::destructive_button(action_label, confirmed);
                            let resp = ui.add_enabled(confirmed, action_btn);

                            if !confirmed {
                                resp.clone().on_disabled_hover_text(format!("Type {} to enable", confirm_word));
                            }

                            if resp.clicked() {
                                app.confirm_text.clear();
                                if is_deploy {
                                    match app.resolve_deploy_provider() {
                                        Ok((wim_path, index)) => {
                                            app.start_deploy(0, wim_path, index);
                                        }
                                        Err(e) => {
                                            app.error_message = Some(e);
                                            app.screen = Screen::Error;
                                        }
                                    }
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
}

fn deploy_details(ui: &mut egui::Ui, app: &AgentApp) {
    detail_grid(ui, |ui| {
        detail_row(ui, "Target disk", "Disk 0 — Primary drive");
        ui.end_row();

        if let Some(config) = &app.config {
            if let Some(image) = &config.image {
                let size_str = image
                    .size_bytes
                    .map(|b| format!(" ({:.1} GB)", b as f64 / 1_073_741_824.0))
                    .unwrap_or_default();
                detail_row(ui, "Image", &format!("{}{}", image.name, size_str));
                ui.end_row();
            }
        }

        detail_row(ui, "Partitions", "ESP (300 MB) + MSR (16 MB) + Windows");
        ui.end_row();
    });

    ui.add_space(12.0);

    egui::Frame::NONE
        .fill(theme::ERROR_DIM)
        .corner_radius(4.0)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("⚠")
                        .size(14.0)
                        .color(theme::ERROR),
                );
                ui.label(
                    egui::RichText::new("All existing data on Disk 0 will be permanently erased.")
                        .color(theme::ERROR)
                        .size(13.0)
                        .strong(),
                );
            });
        });
}

fn capture_details(ui: &mut egui::Ui, app: &AgentApp) {
    detail_grid(ui, |ui| {
        detail_row(ui, "Source", &app.machine_name);
        ui.end_row();

        if let Some(config) = &app.config {
            if let Some(target) = &config.capture_target {
                detail_row(ui, "Destination", &target.unc_path);
                ui.end_row();
            }

            if let Some(image) = &config.image {
                detail_row(ui, "Image name", &image.name);
                ui.end_row();
            }
        }
    });
}

fn detail_grid(ui: &mut egui::Ui, add_rows: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new("confirm_details")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            add_rows(ui);
        });
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(
        egui::RichText::new(label)
            .size(13.0)
            .color(theme::TEXT_TERTIARY),
    );
    ui.label(
        egui::RichText::new(value)
            .size(14.0)
            .color(theme::TEXT)
            .strong(),
    );
}
