use eframe::egui::{self, Color32, FontFamily, FontId, RichText};

pub const BG: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1C);
pub const SURFACE: Color32 = Color32::from_rgb(0x2D, 0x2D, 0x2D);
pub const SURFACE_BORDER: Color32 = Color32::from_rgb(0x40, 0x40, 0x40);
pub const TEXT: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xAB, 0xAB, 0xAB);
pub const TEXT_TERTIARY: Color32 = Color32::from_rgb(0x76, 0x76, 0x76);

pub const ACCENT: Color32 = Color32::from_rgb(0x60, 0xCD, 0xFF);
pub const TEAL: Color32 = Color32::from_rgb(0x2E, 0xE5, 0xD2);
pub const SUCCESS: Color32 = Color32::from_rgb(0x6C, 0xCB, 0x5F);
pub const WARNING: Color32 = Color32::from_rgb(0xFC, 0xE1, 0x00);
pub const ERROR: Color32 = Color32::from_rgb(0xFF, 0x99, 0xA4);

pub fn heading(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(18.0, FontFamily::Proportional))
        .color(TEXT)
        .strong()
}

#[allow(dead_code)]
pub fn title(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(15.0, FontFamily::Proportional))
        .color(TEXT)
        .strong()
}

pub fn body(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(13.0, FontFamily::Proportional))
        .color(TEXT_SECONDARY)
}

pub fn caption(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(11.0, FontFamily::Proportional))
        .color(TEXT_TERTIARY)
}

pub fn label(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(10.0, FontFamily::Proportional))
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

pub fn setup_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.widgets.noninteractive.bg_fill = SURFACE;
    visuals.widgets.inactive.bg_fill = SURFACE;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x38, 0x38, 0x38);
    visuals.widgets.active.bg_fill = ACCENT;
    visuals.override_text_color = Some(TEXT);
    ctx.set_visuals(visuals);
}
