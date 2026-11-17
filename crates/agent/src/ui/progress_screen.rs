use eframe::egui;
use crate::app::AgentApp;
use super::theme;

pub fn show(ctx: &egui::Context, app: &mut AgentApp) {
    theme::setup_visuals(ctx);
    super::header::show(ctx, app);

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::BG).inner_margin(egui::Margin::symmetric(32, 24)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);

                egui::Frame::NONE
                    .fill(theme::SURFACE)
                    .stroke(egui::Stroke::new(1.0_f32, theme::SURFACE_BORDER))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(20, 20))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width().min(480.0));

                        ui.label(theme::heading("Applying Image"));
                        ui.add_space(12.0);

                        let progress = 0.0_f32;
                        let bar = egui::ProgressBar::new(progress)
                            .show_percentage()
                            .animate(true);
                        ui.add(bar);
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label(theme::caption("0% complete"));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(theme::caption("0.0 / 0.0 GB"));
                            });
                        });

                        ui.add_space(12.0);

                        ui.label(theme::label("TASK SEQUENCE"));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            let steps = ["Partition", "Apply Image", "Inject Drivers", "Unattend", "Boot Config", "Reboot"];
                            for (i, step) in steps.iter().enumerate() {
                                let color = if i == 0 {
                                    theme::TEXT_TERTIARY
                                } else {
                                    theme::TEXT_TERTIARY
                                };
                                let prefix = if i == 0 { "● " } else { "○ " };
                                ui.label(
                                    egui::RichText::new(format!("{prefix}{step}"))
                                        .size(11.0)
                                        .color(color),
                                );
                            }
                        });

                        ui.add_space(12.0);

                        ui.label(theme::label("LOG"));
                        egui::Frame::NONE
                            .fill(theme::BG)
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(8, 8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.set_max_height(80.0);
                                ui.label(theme::caption("Waiting to start..."));
                            });
                    });
            });
        });
}
