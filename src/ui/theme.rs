//! Visual theme (light/dark) matching the MCHOSE HUB design language:
//! soft neutral backgrounds, white rounded cards, near-black accents.
//! Also installs a CJK-capable system font so Chinese labels render.

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, Visuals};

use crate::config::ThemeMode;

/// Colours used by the custom widgets.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub border: Color32,
    pub text: Color32,
    pub text_weak: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub track: Color32,
}

pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xF1, 0xF2, 0xF4),
    card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    card_hover: Color32::from_rgb(0xF6, 0xF7, 0xF9),
    border: Color32::from_rgb(0xE4, 0xE6, 0xEA),
    text: Color32::from_rgb(0x16, 0x17, 0x1A),
    text_weak: Color32::from_rgb(0x83, 0x87, 0x90),
    accent: Color32::from_rgb(0x2F, 0x6B, 0xFF),
    accent_text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    success: Color32::from_rgb(0x2F, 0x9E, 0x62),
    warning: Color32::from_rgb(0xD9, 0x9A, 0x2B),
    danger: Color32::from_rgb(0xD9, 0x4B, 0x4B),
    track: Color32::from_rgb(0xE9, 0xEB, 0xEF),
};

pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x10, 0x11, 0x13),
    card: Color32::from_rgb(0x1A, 0x1C, 0x20),
    card_hover: Color32::from_rgb(0x22, 0x24, 0x29),
    border: Color32::from_rgb(0x2A, 0x2D, 0x33),
    text: Color32::from_rgb(0xF2, 0xF3, 0xF5),
    text_weak: Color32::from_rgb(0x9A, 0xA0, 0xAB),
    accent: Color32::from_rgb(0x4C, 0x82, 0xFF),
    accent_text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    success: Color32::from_rgb(0x45, 0xC2, 0x81),
    warning: Color32::from_rgb(0xE8, 0xB4, 0x4A),
    danger: Color32::from_rgb(0xE6, 0x6A, 0x6A),
    track: Color32::from_rgb(0x2C, 0x2F, 0x36),
};

/// True when the effective theme is dark.
pub fn is_dark(_ctx: &egui::Context, mode: ThemeMode) -> bool {
    match mode {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => crate::config::system_prefers_dark(),
    }
}

/// Resolve the palette for the configured theme.
pub fn palette(ctx: &egui::Context, mode: ThemeMode) -> Palette {
    if is_dark(ctx, mode) {
        DARK
    } else {
        LIGHT
    }
}

/// Install a system CJK font (best effort) and set the default font family order.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    if let Some((name, bytes, index)) = load_system_cjk_font() {
        fonts
            .font_data
            .insert("cjk".to_string(), Arc::new(FontData::from_owned(bytes)));
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("cjk".to_string());
        }
        log::info!("CJK font loaded: {name} (index {index})");
    } else {
        log::warn!("no CJK system font found; Chinese labels may render as boxes");
    }
    ctx.set_fonts(fonts);
}

fn load_system_cjk_font() -> Option<(String, Vec<u8>, u32)> {
    let candidates: [(&str, u32); 9] = [
        (r"C:\Windows\Fonts\msyh.ttc", 0),
        (r"C:\Windows\Fonts\msyhbd.ttc", 0),
        (r"C:\Windows\Fonts\simhei.ttf", 0),
        (r"C:\Windows\Fonts\simsun.ttc", 0),
        (r"C:\Windows\Fonts\NotoSansCJK-Regular.ttc", 0),
        ("/System/Library/Fonts/PingFang.ttc", 0),
        ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
        ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
        ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
    ];
    for (path, index) in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            return Some((path.to_string(), bytes, index));
        }
    }
    None
}

/// Apply the egui visuals for the configured theme.
pub fn apply(ctx: &egui::Context, mode: ThemeMode) {
    let dark = is_dark(ctx, mode);
    let palette = if dark { DARK } else { LIGHT };
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.bg;
    visuals.extreme_bg_color = palette.card;
    visuals.faint_bg_color = palette.card_hover;
    visuals.override_text_color = Some(palette.text);
    visuals.widgets.noninteractive.bg_fill = palette.card;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.text);
    visuals.widgets.inactive.bg_fill = palette.card;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.text);
    visuals.widgets.inactive.weak_bg_fill = palette.track;
    visuals.widgets.hovered.bg_fill = palette.card_hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, palette.text);
    visuals.widgets.active.bg_fill = palette.track;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, palette.text);
    visuals.selection.bg_fill = palette.accent.gamma_multiply(0.85);
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.accent_text);
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.menu_corner_radius = CornerRadius::same(10);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 26.0;
    style.text_styles = [
        (egui::TextStyle::Heading, egui::FontId::proportional(20.0)),
        (egui::TextStyle::Body, egui::FontId::proportional(14.0)),
        (egui::TextStyle::Button, egui::FontId::proportional(14.0)),
        (egui::TextStyle::Small, egui::FontId::proportional(12.0)),
        (egui::TextStyle::Monospace, egui::FontId::monospace(12.0)),
    ]
    .into();
    ctx.set_style(style);
}
