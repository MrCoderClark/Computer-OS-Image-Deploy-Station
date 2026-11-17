use eframe::egui;
use imaging_common::types::DeployStatus;
use crate::app::AgentApp;
use super::theme;

pub fn show(ctx: &egui::Context, app: &mut AgentApp) {
    theme::setup_visuals(ctx);
    super::header::show(ctx, app);

    if app.reconnecting {
        super::reconnect_overlay::show(ctx, app);
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::BG).inner_margin(egui::Margin::symmetric(32, 24)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(16.0);

                theme::card_frame()
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width().min(520.0));

                        phase_heading(ui, app);

                        ui.add_space(16.0);

                        progress_bar(ui, app);

                        ui.add_space(8.0);

                        progress_stats(ui, app);

                        ui.add_space(16.0);

                        task_sequence(ui, app);

                        ui.add_space(16.0);

                        log_section(ui, app);
                    });
            });
        });
}

fn phase_heading(ui: &mut egui::Ui, app: &AgentApp) {
    let heading = match app.deploy_status {
        Some(DeployStatus::Pending) => "Preparing...",
        Some(DeployStatus::Partitioning) => "Partitioning Disk",
        Some(DeployStatus::Applying) => "Applying Image",
        Some(DeployStatus::Injecting) => "Injecting Drivers",
        Some(DeployStatus::Configuring) => "Configuring System",
        Some(DeployStatus::Rebooting) => "Rebooting",
        Some(DeployStatus::Complete) => "Complete",
        Some(DeployStatus::Failed) => "Failed",
        None => "Starting...",
    };
    ui.label(theme::heading(heading));
}

fn progress_bar(ui: &mut egui::Ui, app: &AgentApp) {
    let progress = app.deploy_progress as f32 / 100.0;
    let bar = egui::ProgressBar::new(progress)
        .show_percentage()
        .animate(progress < 1.0);
    ui.add(bar);
}

fn progress_stats(ui: &mut egui::Ui, app: &AgentApp) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{}% complete", app.deploy_progress))
                .size(13.0)
                .color(theme::TEXT_SECONDARY),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let elapsed = app.formatted_elapsed();
            if !elapsed.is_empty() {
                ui.label(
                    egui::RichText::new(&elapsed)
                        .size(12.0)
                        .color(theme::TEXT_TERTIARY),
                );
                ui.label(
                    egui::RichText::new("·")
                        .size(12.0)
                        .color(theme::TEXT_TERTIARY),
                );
            }

            if !app.deploy_message.is_empty() {
                ui.label(
                    egui::RichText::new(&app.deploy_message)
                        .size(12.0)
                        .color(theme::TEXT_TERTIARY),
                );
            }
        });
    });
}

fn task_sequence(ui: &mut egui::Ui, app: &AgentApp) {
    ui.label(theme::label("TASK SEQUENCE"));
    ui.add_space(6.0);

    let steps: &[(&str, DeployStatus)] = &[
        ("Partition", DeployStatus::Partitioning),
        ("Apply Image", DeployStatus::Applying),
        ("Inject Drivers", DeployStatus::Injecting),
        ("Unattend", DeployStatus::Configuring),
        ("Boot Config", DeployStatus::Configuring),
        ("Reboot", DeployStatus::Rebooting),
    ];

    let current = app.deploy_status.unwrap_or(DeployStatus::Pending);

    ui.horizontal_wrapped(|ui| {
        for (i, (name, step_status)) in steps.iter().enumerate() {
            let (prefix, color) = step_indicator(current, *step_status, i, steps);
            ui.label(
                egui::RichText::new(format!("{prefix} {name}"))
                    .size(12.0)
                    .color(color),
            );
        }
    });
}

fn step_indicator(
    current: DeployStatus,
    step: DeployStatus,
    index: usize,
    steps: &[(&str, DeployStatus)],
) -> (&'static str, egui::Color32) {
    let current_ord = status_order(current);
    let step_ord = status_order(step);

    let is_duplicate_status = index > 0
        && steps.get(index.wrapping_sub(1)).map(|(_, s)| *s) == Some(step);

    if current_ord > step_ord {
        ("\u{2713}", theme::SUCCESS)
    } else if current_ord == step_ord && !is_duplicate_status {
        ("\u{25CF}", theme::ACCENT)
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

fn log_section(ui: &mut egui::Ui, app: &mut AgentApp) {
    let toggle_text = if app.log_expanded { "Log ▴" } else { "Log ▾" };
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new(toggle_text)
                    .size(11.0)
                    .color(theme::TEXT_TERTIARY),
            )
            .frame(false),
        )
        .clicked()
    {
        app.log_expanded = !app.log_expanded;
    }

    if app.log_expanded {
        ui.add_space(4.0);
        egui::Frame::NONE
            .fill(theme::BG)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(8, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_max_height(150.0);
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if app.log_lines.is_empty() {
                            ui.label(theme::mono("Waiting to start..."));
                        } else {
                            for line in &app.log_lines {
                                ui.label(theme::mono(line));
                            }
                        }
                    });
            });
    }
}
