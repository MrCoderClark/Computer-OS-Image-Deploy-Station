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
                ui.add_space(16.0);

                image_panel(ui, app);

                ui.add_space(24.0);

                action_buttons(ui, app);
            });
        });
}

fn image_panel(ui: &mut egui::Ui, app: &AgentApp) {
    egui::Frame::NONE
        .fill(theme::SURFACE)
        .stroke(egui::Stroke::new(1.0_f32, theme::SURFACE_BORDER))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(20, 16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width().min(420.0));

            if let Some(config) = &app.config {
                if let Some(image) = &config.image {
                    ui.label(theme::heading(&image.name));
                    ui.add_space(8.0);

                    egui::Grid::new("image_details")
                        .num_columns(2)
                        .spacing([40.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(theme::label("OS FAMILY"));
                            ui.label(theme::label("BUILD DATE"));
                            ui.end_row();

                            ui.label(theme::body(&image.os_family.to_string()));
                            ui.label(theme::body(
                                image.build_date.as_deref().unwrap_or("Unknown"),
                            ));
                            ui.end_row();

                            ui.label(theme::label("SIZE"));
                            ui.label(theme::label("HASH"));
                            ui.end_row();

                            let size = image
                                .size_bytes
                                .map(|b| format!("{:.1} GB", b as f64 / 1_073_741_824.0))
                                .unwrap_or_else(|| "Unknown".into());
                            ui.label(theme::body(&size));

                            let hash_status = if image.sha256.is_some() {
                                egui::RichText::new("SHA-256 verified")
                                    .color(theme::SUCCESS)
                                    .size(13.0)
                            } else {
                                egui::RichText::new("Not verified")
                                    .color(theme::TEXT_TERTIARY)
                                    .size(13.0)
                            };
                            ui.label(hash_status);
                            ui.end_row();
                        });
                } else if !app.server_images.is_empty() {
                    let img = &app.server_images[0];
                    ui.label(theme::heading(&img.name));
                    ui.label(
                        egui::RichText::new("From server")
                            .size(10.0)
                            .color(theme::ACCENT),
                    );
                    ui.add_space(8.0);

                    egui::Grid::new("server_image_details")
                        .num_columns(2)
                        .spacing([40.0, 6.0])
                        .show(ui, |ui| {
                            ui.label(theme::label("OS FAMILY"));
                            ui.label(theme::label("BUILD DATE"));
                            ui.end_row();

                            ui.label(theme::body(&img.os_family.to_string()));
                            ui.label(theme::body(
                                img.build_date.as_deref().unwrap_or("Unknown"),
                            ));
                            ui.end_row();

                            ui.label(theme::label("SIZE"));
                            ui.label(theme::label("IMAGES AVAILABLE"));
                            ui.end_row();

                            let size = format!("{:.1} GB", img.size_bytes as f64 / 1_073_741_824.0);
                            ui.label(theme::body(&size));
                            ui.label(theme::body(&format!("{}", app.server_images.len())));
                            ui.end_row();
                        });
                } else {
                    ui.label(theme::heading("No Image Loaded"));
                    ui.add_space(4.0);
                    ui.label(theme::body("Online mode — connecting to server..."));
                }
            } else {
                ui.label(theme::heading("No Configuration"));
                ui.add_space(4.0);
                ui.label(theme::body("Place config.json on the USB drive"));
            }
        });
}

fn action_buttons(ui: &mut egui::Ui, app: &mut AgentApp) {
    let button_width = ui.available_width().min(420.0);

    ui.horizontal(|ui| {
        let half = (button_width - 14.0) / 2.0;

        let deploy_btn = egui::Frame::NONE
            .fill(egui::Color32::from_rgb(0x0A, 0x3A, 0x5C))
            .stroke(egui::Stroke::new(2.0_f32, theme::ACCENT))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(14, 20))
            .show(ui, |ui| {
                ui.set_width(half);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("↓")
                            .size(24.0)
                            .color(theme::ACCENT)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Image This Computer")
                            .size(13.0)
                            .color(theme::TEXT)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("Apply image to this machine's disk")
                            .size(10.0)
                            .color(theme::TEXT_TERTIARY),
                    );
                });
            });

        if deploy_btn.response.interact(egui::Sense::click()).clicked() {
            app.screen = Screen::ConfirmDeploy;
        }

        ui.add_space(14.0);

        let capture_btn = egui::Frame::NONE
            .fill(theme::SURFACE)
            .stroke(egui::Stroke::new(1.0_f32, theme::SURFACE_BORDER))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(14, 20))
            .show(ui, |ui| {
                ui.set_width(half);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("↑")
                            .size(24.0)
                            .color(theme::TEXT_SECONDARY)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Create Image")
                            .size(13.0)
                            .color(theme::TEXT)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("Capture this machine to the server")
                            .size(10.0)
                            .color(theme::TEXT_TERTIARY),
                    );
                });
            });

        if capture_btn.response.interact(egui::Sense::click()).clicked() {
            app.screen = Screen::ConfirmCapture;
        }
    });
}
