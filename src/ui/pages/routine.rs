//! Sound / "Regular parameters" page, laid out like the vendor client:
//! a tab strip (Regular parameters · Equalizer · Virtual 7.1 Surround), a product
//! stage with the headset photo, and a three-column parameter panel
//! (Volume + Microphone | Audio brightening + Dynamic Low Frequency |
//!  Intelligent volume + Vocal Clarity).

use egui::{Color32, CornerRadius, RichText, Stroke, Vec2};

use crate::app::MHubApp;
use crate::model::audio::{AudioChange, EffectKind};
use crate::ui::pages::Page;
use crate::ui::theme::Palette;
use crate::ui::widgets;

pub fn draw(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    draw_device_header(app, ui, palette);
    ui.add_space(8.0);
    draw_tabs(app, ui, palette);
    ui.add_space(10.0);
    draw_stage(app, ui, palette);
    ui.add_space(10.0);
    draw_parameters(app, ui, palette);
}

fn draw_device_header(app: &MHubApp, ui: &mut egui::Ui, palette: Palette) {
    ui.horizontal(|ui| {
        let name = app
            .selected_device
            .as_ref()
            .map(|device| {
                crate::model::device::family_for(device.vid, device.pid)
                    .map(|family| family.name)
                    .filter(|name| !name.is_empty())
                    .unwrap_or(device.product_name.as_str())
                    .to_string()
            })
            .unwrap_or_else(|| app.i18n.t("device.no_device").to_string());
        ui.label(RichText::new(name).strong().size(21.0).color(palette.text));
        ui.add_space(10.0);
        let connection = if app.connected() {
            app.i18n.t("connect.wireless24g")
        } else {
            app.i18n.t("device.status.disconnected")
        };
        widgets::chip(ui, palette, connection);
        if let Some(serial) = app
            .selected_device
            .as_ref()
            .and_then(|device| device.serial.as_ref())
            .filter(|serial| !serial.trim().is_empty())
        {
            widgets::chip(ui, palette, &format!("SN {serial}"));
        }
        if let Some(firmware) = &app.status.headset_firmware {
            widgets::chip(ui, palette, &format!("FW {firmware}"));
        }
    });
}

fn draw_tabs(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let tabs = [
        (Page::Routine, "routine.tab.regular"),
        (Page::Eq, "routine.tab.equalizer"),
        (Page::Surround, "routine.tab.surround"),
    ];
    ui.horizontal(|ui| {
        for (page, label) in tabs {
            let active = app.page == page;
            let text = RichText::new(app.i18n.t(label))
                .size(13.5)
                .color(if active {
                    palette.accent_text
                } else {
                    palette.text_weak
                });
            let button = egui::Button::new(text)
                .fill(if active { palette.accent } else { palette.card })
                .stroke(Stroke::new(
                    1.0_f32,
                    if active { palette.accent } else { palette.border },
                ))
                .corner_radius(CornerRadius::same(10))
                .min_size(Vec2::new(0.0, 30.0));
            if ui.add(button).clicked() && !active {
                app.set_page(page);
            }
            ui.add_space(6.0);
        }
    });
}

/// Product photo stage (light surface in both themes, like a product shot).
fn draw_stage(app: &MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let stage = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    let frame = egui::Frame::new()
        .fill(stage)
        .stroke(Stroke::new(1.0_f32, palette.border))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::symmetric(18, 10));
    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.vertical_centered(|ui| {
            if let Some(texture) = &app.product_image {
                ui.add(egui::Image::new(texture).fit_to_exact_size(Vec2::new(188.0, 180.0)));
            } else {
                ui.add_space(50.0);
                ui.label(
                    RichText::new("◑")
                        .size(56.0)
                        .color(Color32::from_rgb(0x9A, 0xA0, 0xAB)),
                );
                ui.add_space(50.0);
            }
        });
    });
}

fn draw_parameters(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    widgets::card(ui, palette, |ui| {
        ui.columns(3, |columns| {
            draw_volume_block(app, &mut columns[0], palette);
            draw_microphone_block(app, &mut columns[0], palette);
            draw_effect_block(app, &mut columns[1], palette, EffectKind::Brilliance);
            draw_effect_block(app, &mut columns[1], palette, EffectKind::VirtualBass);
            draw_effect_block(app, &mut columns[2], palette, EffectKind::AdaptiveVolume);
            draw_effect_block(app, &mut columns[2], palette, EffectKind::VoiceClarity);
        });
    });
}

fn block_title(ui: &mut egui::Ui, palette: Palette, title: &str) {
    ui.label(RichText::new(title).strong().size(14.0).color(palette.text));
}

/// Compact parameter slider: label on the left, value on the right, slider below.
fn param_slider(
    ui: &mut egui::Ui,
    palette: Palette,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    display: impl Fn(f32) -> String,
    enabled: bool,
) -> bool {
    let mut changed = false;
    ui.add_enabled_ui(enabled, |ui| {
        let width = ui.available_width();
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(12.0).color(palette.text_weak));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(display(*value))
                        .size(12.0)
                        .color(palette.text),
                );
            });
        });
        let response = ui.add_sized(
            [width, 18.0],
            egui::Slider::new(value, range.clone())
                .show_value(false)
                .trailing_fill(true),
        );
        changed = response.changed();
    });
    changed
}

fn draw_volume_block(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let title = app.i18n.t("routine.volume").to_string();
    let mute = app.i18n.t("routine.mute").to_string();
    let mut volume = app.audio.volume.speaker;
    let mut muted = app.audio.volume.speaker_muted;
    let enabled = app.caps.volume && !muted;
    let cap = app.caps.volume;

    block_title(ui, palette, &title);
    let mut percent = volume * 100.0;
    if param_slider(
        ui,
        palette,
        "",
        &mut percent,
        0.0..=100.0,
        |value| format!("{value:.0}%"),
        enabled,
    ) {
        volume = (percent / 100.0).clamp(0.0, 1.0);
        app.apply_audio(AudioChange::SpeakerVolume(volume));
    }
    if widgets::toggle_row(ui, palette, &mute, None, &mut muted, cap).clicked() {
        app.apply_audio(AudioChange::SpeakerMute(muted));
    }
    ui.add_space(6.0);
}

fn draw_microphone_block(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let title = app.i18n.t("mic.title").to_string();
    let mute = app.i18n.t("routine.mute").to_string();
    let mut volume = app.audio.volume.mic;
    let mut muted = app.audio.volume.mic_muted;
    let mut ai = app.audio.mic.ai_noise_reduction;
    let mut listen = app.audio.mic.mic_listen;
    let mut magic = app.audio.mic.magic_voice;
    let mut magic_type = app.audio.mic.magic_voice_type;
    let cap_volume = app.caps.mic_volume && !muted;
    let cap_mute = app.caps.mic_volume;
    let cap_dsp = app.caps.mic_dsp;
    let cap_listen = app.caps.mic_listen;
    let ai_label = app.i18n.t("routine.ai_nr").to_string();
    let ai_hint = app.i18n.t("routine.ai_nr.desc").to_string();
    let listen_label = app.i18n.t("routine.mic_listen").to_string();
    let listen_hint = app.i18n.t("routine.mic_listen.desc").to_string();
    let magic_label = app.i18n.t("routine.magic_voice").to_string();
    let magic_hint = app.i18n.t("routine.magic_voice.desc").to_string();
    let type_label = app.i18n.t("routine.magic_voice.type").to_string();
    let types = [
        app.i18n.t("routine.magic_voice.monster").to_string(),
        app.i18n.t("routine.magic_voice.cartoon").to_string(),
        app.i18n.t("routine.magic_voice.male").to_string(),
        app.i18n.t("routine.magic_voice.female").to_string(),
    ];

    widgets::divider(ui, palette);
    block_title(ui, palette, &title);
    let mut percent = volume * 100.0;
    if param_slider(
        ui,
        palette,
        "",
        &mut percent,
        0.0..=100.0,
        |value| format!("{value:.0}%"),
        cap_volume,
    ) {
        volume = (percent / 100.0).clamp(0.0, 1.0);
        app.apply_audio(AudioChange::MicVolume(volume));
    }
    if widgets::toggle_row(ui, palette, &mute, None, &mut muted, cap_mute).clicked() {
        app.apply_audio(AudioChange::MicMute(muted));
    }
    if widgets::toggle_row(ui, palette, &ai_label, Some(&ai_hint), &mut ai, cap_dsp).clicked() {
        app.apply_audio(AudioChange::AiNoiseReduction(ai));
    }
    if widgets::toggle_row(
        ui,
        palette,
        &listen_label,
        Some(&listen_hint),
        &mut listen,
        cap_listen,
    )
    .clicked()
    {
        app.apply_audio(AudioChange::MicListen(listen));
    }
    if widgets::toggle_row(ui, palette, &magic_label, Some(&magic_hint), &mut magic, cap_dsp)
        .clicked()
    {
        app.apply_audio(AudioChange::MagicVoiceEnabled(magic));
    }
    if magic {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&type_label).size(12.0).color(palette.text_weak));
            egui::ComboBox::from_id_salt("magic_voice_type")
                .selected_text(types[usize::from(magic_type.min(3))].clone())
                .show_ui(ui, |ui| {
                    for (index, label) in types.iter().enumerate() {
                        if ui
                            .selectable_label(magic_type as usize == index, label)
                            .clicked()
                        {
                            magic_type = index as u8;
                        }
                    }
                });
        });
        if magic_type != app.audio.mic.magic_voice_type {
            app.apply_audio(AudioChange::MagicVoiceType(magic_type));
        }
    }
}

fn draw_effect_block(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette, kind: EffectKind) {
    let state = *app.audio.effect(kind);
    let mut enabled = state.enabled;
    let mut level = state.level;
    let mut cutoff = state.cutoff_hz;
    let mut mode = state.mode;
    let mut noise = state.noise_suppression;
    let cap = app.caps.effects;
    let title = app.i18n.t(kind.title_key()).to_string();
    let desc = app.i18n.t(kind.description_key()).to_string();
    let reset = app.i18n.t("effect.reset").to_string();
    let mode_music = app.i18n.t("effect.mode.music.short").to_string();
    let mode_voice = app.i18n.t("effect.mode.voice.short").to_string();

    widgets::divider(ui, palette);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong().size(14.0).color(palette.text))
            .on_hover_text(desc);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(cap, |ui| {
                widgets::toggle(ui, palette, &mut enabled);
            });
        });
    });
    ui.add_space(2.0);

    let controls = cap && state.enabled;
    let mut level_value = level as f32;
    if param_slider(
        ui,
        palette,
        app.i18n.t("effect.level"),
        &mut level_value,
        0.0..=10.0,
        |value| format!("{value:.0}"),
        controls,
    ) {
        level = level_value.round() as i32;
    }

    match kind {
        EffectKind::Brilliance => {}
        EffectKind::VirtualBass => {
            let mut cutoff_value = cutoff as f32;
            if param_slider(
                ui,
                palette,
                app.i18n.t("effect.cutoff"),
                &mut cutoff_value,
                100.0..=400.0,
                |value| format!("{value:.0} Hz"),
                controls,
            ) {
                cutoff = (((cutoff_value.round() as i32) + 25) / 50 * 50).clamp(100, 400);
            }
        }
        EffectKind::AdaptiveVolume => {
            ui.add_enabled_ui(controls, |ui| {
                let mut selected = mode as usize;
                let options: [(usize, &str); 2] =
                    [(0, mode_music.as_str()), (1, mode_voice.as_str())];
                ui.horizontal(|ui| {
                    widgets::segmented(ui, palette, &mut selected, &options);
                });
                mode = selected as u8;
            });
        }
        EffectKind::VoiceClarity => {
            let db = format!(" {}", app.i18n.t("common.db"));
            if widgets::slider_row(
                ui,
                palette,
                app.i18n.t("effect.noise"),
                &mut noise,
                0..=20,
                &db,
                controls,
            ) {
                noise = (noise / 5 * 5).clamp(0, 20);
            }
        }
    }

    // The vendor shows a reset arrow for brilliance and vocal clarity only.
    if matches!(kind, EffectKind::Brilliance | EffectKind::VoiceClarity) {
        ui.horizontal(|ui| {
            ui.add_enabled_ui(cap, |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new(format!("↺ {reset}"))
                                .size(11.0)
                                .color(palette.text_weak),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE),
                    )
                    .clicked()
                {
                    let defaults = crate::model::audio::default_effects()[kind as usize];
                    if defaults.level != state.level {
                        app.apply_audio(AudioChange::EffectLevel {
                            kind,
                            level: defaults.level,
                        });
                    }
                    if kind == EffectKind::VoiceClarity
                        && defaults.noise_suppression != state.noise_suppression
                    {
                        app.apply_audio(AudioChange::VoiceNoiseSuppression(
                            defaults.noise_suppression,
                        ));
                    }
                }
            });
        });
    }

    if enabled != state.enabled {
        app.apply_audio(AudioChange::EffectEnabled { kind, enabled });
    }
    if level != state.level {
        app.apply_audio(AudioChange::EffectLevel { kind, level });
    }
    if kind == EffectKind::VirtualBass && cutoff != state.cutoff_hz {
        app.apply_audio(AudioChange::BassCutoffHz(cutoff));
    }
    if kind == EffectKind::AdaptiveVolume && mode != state.mode {
        app.apply_audio(AudioChange::AdaptiveMode(mode));
    }
    if kind == EffectKind::VoiceClarity && noise != state.noise_suppression {
        app.apply_audio(AudioChange::VoiceNoiseSuppression(noise));
    }
}
