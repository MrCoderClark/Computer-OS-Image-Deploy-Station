use eframe::egui;
use imaging_common::types::DeployStatus;
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

                        let heading = match app.deploy_status {
                            Some(DeployStatus::Complete) => "Complete",
                            Some(DeployStatus::Failed) => "Failed",
                            _ => "Applying Image",
                        };
                        ui.label(theme::heading(heading));
                        ui.add_space(12.0);

                        let progress = app.deploy_progress as f32 / 100.0;
                        let bar = egui::ProgressBar::new(progress)
                            .show_percentage()
                            .animate(progress < 1.0);
                        ui.add(bar);
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label(theme::caption(&format!("{}% complete", app.deploy_progress)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(theme::caption(&app.deploy_message));
                            });
                        });

                        ui.add_space(12.0);

                        ui.label(theme::label("TASK SEQUENCE"));
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            let steps = [
                                ("Partition", DeployStatus::Partitioning),
                                ("Apply Image", DeployStatus::Applying),
                                ("Configure", DeployStatus::Configuring),
                                ("Reboot", DeployStatus::Rebooting),
                            ];
                            let current = app.deploy_status.unwrap_or(DeployStatus::Pending);
                            for (name, step_status) in &steps {
                                let (prefix, color) = step_indicator(current, *step_status);
                                ui.label(
                                    egui::RichText::new(format!("{prefix} {name}"))
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
                                ui.set_max_height(120.0);
                                egui::ScrollArea::vertical()
                                    .stick_to_bottom(true)
                                    .show(ui, |ui| {
                                        if app.log_lines.is_empty() {
                                            ui.label(theme::caption("Waiting to start..."));
                                        } else {
                                            for line in &app.log_lines {
                                                ui.label(theme::caption(line));
                                            }
                                        }
                                    });
                            });
                    });
            });
        });
}

fn step_indicator(current: DeployStatus, step: DeployStatus) -> (&'static str, egui::Color32) {
    let current_ord = status_order(current);
    let step_ord = status_order(step);

    if current_ord > step_ord {
        ("\u{2713}", theme::ACCENT)
    } else if current_ord == step_ord {
        ("\u{25CF}", theme::TEAL)
    } else {
        ("\u{25CB}", theme::TEXT_TERTIARY)
    }
}

fn status_order(s: DeployStatus) -> u8 {
    match s {
        DeployStatus::Pending => 0,
        DeployStatus::Partitioning => 1,
        DeployStatus::Applying => 2,
        DeployStatus::Injecting => 3,
        DeployStatus::Configuring => 4,
        DeployStatus::Rebooting => 5,
        DeployStatus::Complete => 6,
        DeployStatus::Failed => 255,
    }
}
