use eframe::egui::{self, Color32, FontFamily, FontId, RichText, Stroke, Vec2};

pub const BG: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1C);
pub const SURFACE: Color32 = Color32::from_rgb(0x2D, 0x2D, 0x2D);
pub const SURFACE_HOVER: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);
pub const SURFACE_BORDER: Color32 = Color32::from_rgb(0x40, 0x40, 0x40);
pub const TEXT: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xAB, 0xAB, 0xAB);
pub const TEXT_TERTIARY: Color32 = Color32::from_rgb(0x76, 0x76, 0x76);

pub const ACCENT: Color32 = Color32::from_rgb(0x60, 0xCD, 0xFF);
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x0A, 0x3A, 0x5C);
pub const TEAL: Color32 = Color32::from_rgb(0x2E, 0xE5, 0xD2);
pub const SUCCESS: Color32 = Color32::from_rgb(0x6C, 0xCB, 0x5F);
pub const WARNING: Color32 = Color32::from_rgb(0xFC, 0xE1, 0x00);
pub const ERROR: Color32 = Color32::from_rgb(0xFF, 0x99, 0xA4);
pub const ERROR_DIM: Color32 = Color32::from_rgb(0x3C, 0x1A, 0x1E);

pub fn display(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(28.0, FontFamily::Proportional))
        .color(TEXT)
        .strong()
}

pub fn heading(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(20.0, FontFamily::Proportional))
        .color(TEXT)
        .strong()
}

#[allow(dead_code)]
pub fn title(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(16.0, FontFamily::Proportional))
        .color(TEXT)
        .strong()
}

pub fn body(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(14.0, FontFamily::Proportional))
        .color(TEXT_SECONDARY)
}

#[allow(dead_code)]
pub fn caption(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(12.0, FontFamily::Proportional))
        .color(TEXT_TERTIARY)
}

pub fn mono(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(12.0, FontFamily::Monospace))
        .color(TEXT_TERTIARY)
}

pub fn label(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(11.0, FontFamily::Proportional))
        .color(TEXT_TERTIARY)
}

pub fn mode_color(mode: imaging_common::config::OperatingMode) -> Color32 {
    use imaging_common::config::OperatingMode;
    match mode {
        OperatingMode::Online => ACCENT,
        OperatingMode::Offline => WARNING,
        OperatingMode::Hybrid => TEAL,
    }
}

pub fn connection_color(status: &crate::app::ConnectionStatus) -> Color32 {
    match status {
        crate::app::ConnectionStatus::Connected => SUCCESS,
        crate::app::ConnectionStatus::Connecting => WARNING,
        crate::app::ConnectionStatus::Unreachable => ERROR,
    }
}

pub fn setup_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.hovered.bg_fill = SURFACE_HOVER;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0_f32, TEXT);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    ctx.set_style(style);
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, SURFACE_BORDER))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(20, 16))
}

pub fn primary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(
        RichText::new(text)
            .size(14.0)
            .color(Color32::from_rgb(0x00, 0x20, 0x40))
            .strong(),
    )
    .fill(ACCENT)
    .corner_radius(4.0)
}

pub fn secondary_button(text: &str) -> egui::Button<'_> {
    egui::Button::new(
        RichText::new(text).size(14.0).color(TEXT),
    )
    .fill(SURFACE)
    .stroke(Stroke::new(1.0_f32, SURFACE_BORDER))
    .corner_radius(4.0)
}

pub fn destructive_button(text: &str, enabled: bool) -> egui::Button<'_> {
    if enabled {
        egui::Button::new(
            RichText::new(text)
                .size(14.0)
                .color(Color32::WHITE)
                .strong(),
        )
        .fill(Color32::from_rgb(0xC4, 0x2B, 0x1C))
        .corner_radius(4.0)
    } else {
        egui::Button::new(
            RichText::new(text)
                .size(14.0)
                .color(TEXT_TERTIARY),
        )
        .fill(SURFACE)
        .corner_radius(4.0)
    }
}
