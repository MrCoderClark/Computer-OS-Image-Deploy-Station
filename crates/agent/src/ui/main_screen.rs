use eframe::egui;
use crate::app::{AgentApp, ImageSource, Screen};
use super::theme;

pub fn show(ctx: &egui::Context, app: &mut AgentApp) {
    theme::setup_visuals(ctx);
    super::header::show(ctx, app);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::BG).inner_margin(egui::Margin::symmetric(32, 24)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(12.0);

                if app.using_offline_fallback {
                    offline_fallback_banner(ui);
                    ui.add_space(12.0);
                }

                image_panel(ui, app);

                ui.add_space(24.0);

                action_buttons(ui, app);
            });
        });
}

fn offline_fallback_banner(ui: &mut egui::Ui) {
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(0x3A, 0x30, 0x00))
        .stroke(egui::Stroke::new(1.0_f32, theme::WARNING))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(16, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width().min(460.0));
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("⚠")
                        .size(16.0)
                        .color(theme::WARNING),
                );
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Server unreachable — using local image from USB")
                            .size(13.0)
                            .color(theme::WARNING)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("Deploy will use the image stored on this USB stick.")
                            .size(12.0)
                            .color(theme::TEXT_SECONDARY),
                    );
                });
            });
        });
}

fn image_panel(ui: &mut egui::Ui, app: &AgentApp) {
    theme::card_frame()
        .show(ui, |ui| {
            ui.set_width(ui.available_width().min(460.0));

            if let Some(config) = &app.config {
                if let Some(image) = &config.image {
                    ui.horizontal(|ui| {
                        ui.label(theme::heading(&image.name));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let (source_label, source_color) = match app.image_source {
                                ImageSource::UsbFallback => ("From USB", theme::WARNING),
                                ImageSource::Server => ("From server", theme::ACCENT),
                                ImageSource::Config => ("Local", theme::TEXT_TERTIARY),
                            };
                            ui.label(
                                egui::RichText::new(source_label)
                                    .size(11.0)
                                    .color(source_color),
                            );
                        });
                    });
                    ui.add_space(12.0);

                    egui::Grid::new("image_details")
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(theme::label("OS FAMILY"));
                            ui.label(theme::label("BUILD DATE"));
                            ui.end_row();

                            ui.label(theme::body(&image.os_family.to_string()));
                            ui.label(theme::body(
                                image.build_date.as_deref().unwrap_or("Unknown"),
                            ));
                            ui.end_row();

                            ui.add_space(6.0);
                            ui.add_space(6.0);
                            ui.end_row();

                            ui.label(theme::label("SIZE"));
                            ui.label(theme::label("HASH"));
                            ui.end_row();

                            let size = image
                                .size_bytes
                                .map(|b| format!("{:.1} GB", b as f64 / 1_073_741_824.0))
                                .unwrap_or_else(|| "Unknown".into());
                            ui.label(theme::body(&size));

                            if image.sha256.is_some() {
                                ui.label(
                                    egui::RichText::new("SHA-256 verified")
                                        .size(14.0)
                                        .color(theme::SUCCESS),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("Not verified")
                                        .size(14.0)
                                        .color(theme::TEXT_TERTIARY),
                                );
                            }
                            ui.end_row();
                        });
                } else if !app.server_images.is_empty() {
                    server_image_panel(ui, app);
                } else {
                    no_image_panel(ui, app);
                }
            } else {
                ui.label(theme::heading("No Configuration"));
                ui.add_space(8.0);
                ui.label(theme::body("Place config.json on the USB drive to begin."));
            }
        });
}

fn server_image_panel(ui: &mut egui::Ui, app: &AgentApp) {
    let img = &app.server_images[0];

    ui.horizontal(|ui| {
        ui.label(theme::heading(&img.name));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new("From server")
                    .size(11.0)
                    .color(theme::ACCENT),
            );
        });
    });
    ui.add_space(12.0);

    egui::Grid::new("server_image_details")
        .num_columns(2)
        .spacing([40.0, 4.0])
        .show(ui, |ui| {
            ui.label(theme::label("OS FAMILY"));
            ui.label(theme::label("BUILD DATE"));
            ui.end_row();

            ui.label(theme::body(&img.os_family.to_string()));
            ui.label(theme::body(
                img.build_date.as_deref().unwrap_or("Unknown"),
            ));
            ui.end_row();

            ui.add_space(6.0);
            ui.add_space(6.0);
            ui.end_row();

            ui.label(theme::label("SIZE"));
            ui.label(theme::label("IMAGES AVAILABLE"));
            ui.end_row();

            let size = format!("{:.1} GB", img.size_bytes as f64 / 1_073_741_824.0);
            ui.label(theme::body(&size));
            ui.label(theme::body(&format!("{}", app.server_images.len())));
            ui.end_row();
        });
}

fn no_image_panel(ui: &mut egui::Ui, app: &AgentApp) {
    let is_connecting = app.connection == crate::app::ConnectionStatus::Connecting;
    if is_connecting {
        ui.label(theme::heading("Connecting to Server..."));
        ui.add_space(8.0);
        ui.label(theme::body("Waiting for image list from the server."));
        ui.add_space(8.0);
        ui.spinner();
    } else {
        ui.label(theme::heading("No Image Available"));
        ui.add_space(8.0);
        ui.label(theme::body(
            "Could not load an image. Check the server connection or USB configuration.",
        ));
    }
}

fn action_buttons(ui: &mut egui::Ui, app: &mut AgentApp) {
    let max_width = ui.available_width().min(460.0);
    let half = (max_width - 16.0) / 2.0;

    ui.horizontal(|ui| {
        let deploy_clicked = action_card(
            ui,
            half,
            "↓",
            "Image This Computer",
            "Apply image to this machine's disk",
            theme::ACCENT,
            theme::ACCENT_DIM,
            egui::Stroke::new(2.0_f32, theme::ACCENT),
        );

        if deploy_clicked {
            app.screen = Screen::ConfirmDeploy;
        }

        ui.add_space(16.0);

        let capture_clicked = action_card(
            ui,
            half,
            "↑",
            "Create Image",
            "Capture this machine to the server",
            theme::TEXT_SECONDARY,
            theme::SURFACE,
            egui::Stroke::new(1.0_f32, theme::SURFACE_BORDER),
        );

        if capture_clicked {
            app.screen = Screen::ConfirmCapture;
        }
    });
}

fn action_card(
    ui: &mut egui::Ui,
    width: f32,
    icon: &str,
    label: &str,
    description: &str,
    icon_color: egui::Color32,
    fill: egui::Color32,
    stroke: egui::Stroke,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, 110.0),
        egui::Sense::click(),
    );

    let hovered = response.hovered();
    let bg = if hovered {
        lighten(fill, 0.08)
    } else {
        fill
    };

    if ui.is_rect_visible(rect) {
        ui.painter().rect(rect, 6.0, bg, stroke, egui::StrokeKind::Outside);

        let center_x = rect.center().x;

        ui.painter().text(
            egui::pos2(center_x, rect.top() + 28.0),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::new(26.0, egui::FontFamily::Proportional),
            icon_color,
        );

        ui.painter().text(
            egui::pos2(center_x, rect.top() + 58.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::new(14.0, egui::FontFamily::Proportional),
            theme::TEXT,
        );

        ui.painter().text(
            egui::pos2(center_x, rect.top() + 80.0),
            egui::Align2::CENTER_CENTER,
            description,
            egui::FontId::new(11.0, egui::FontFamily::Proportional),
            theme::TEXT_TERTIARY,
        );
    }

    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response.clicked()
}

fn lighten(color: egui::Color32, amount: f32) -> egui::Color32 {
    let r = (color.r() as f32 + 255.0 * amount).min(255.0) as u8;
    let g = (color.g() as f32 + 255.0 * amount).min(255.0) as u8;
    let b = (color.b() as f32 + 255.0 * amount).min(255.0) as u8;
    egui::Color32::from_rgb(r, g, b)
}
