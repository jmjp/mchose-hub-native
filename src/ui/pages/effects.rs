//! Sound-effects page: the four legacy C-Media DSP blocks in a 2×2 grid.
//!
//! The vendor page (`/headset/audio_speaker`) shows treble enhancement, dynamic low
//! frequency, volume balance and vocal clarity as cards, each with an enable switch,
//! an intensity slider and up to two extra controls. Ranges and defaults below follow
//! the verified `cmedia` layout from the original renderer.

use egui::{Color32, RichText, Stroke};

use crate::app::MHubApp;
use crate::model::audio::{default_effects, AudioChange, EffectKind};
use crate::ui::theme::Palette;
use crate::ui::widgets;

/// Below this content width the effect cards stack in one column.
const TWO_COLUMN_MIN_WIDTH: f32 = 720.0;

/// Draw the sound-effects page.
pub fn draw(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    page_header(app, ui, palette, "effects.title", "effects.subtitle");

    if !app.caps.effects {
        ui.label(
            RichText::new(app.i18n.t("common.unsupported"))
                .size(11.0)
                .color(palette.warning),
        );
        ui.add_space(6.0);
    }

    if ui.available_width() >= TWO_COLUMN_MIN_WIDTH {
        ui.columns(2, |columns| {
            effect_card(app, &mut columns[0], palette, EffectKind::Brilliance);
            effect_card(app, &mut columns[1], palette, EffectKind::VirtualBass);
        });
        ui.add_space(12.0);
        ui.columns(2, |columns| {
            effect_card(app, &mut columns[0], palette, EffectKind::AdaptiveVolume);
            effect_card(app, &mut columns[1], palette, EffectKind::VoiceClarity);
        });
    } else {
        for kind in EffectKind::ALL {
            effect_card(app, ui, palette, kind);
            ui.add_space(12.0);
        }
    }
}

/// Page title + subtitle shared by the three audio pages' visual language.
fn page_header(app: &MHubApp, ui: &mut egui::Ui, palette: Palette, title: &str, subtitle: &str) {
    ui.label(
        RichText::new(app.i18n.t(title))
            .strong()
            .size(22.0)
            .color(palette.text),
    );
    ui.label(
        RichText::new(app.i18n.t(subtitle))
            .size(13.0)
            .color(palette.text_weak),
    );
    ui.add_space(14.0);
}

/// One effect block: switch, description, controls and a per-card reset button.
fn effect_card(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette, kind: EffectKind) {
    let caps = app.caps.effects;
    let current = *app.audio.effect(kind);
    let fallback = default_effects()[kind as usize];

    // Unit suffixes and mode labels are owned strings so the closure below only needs
    // shared access to the translator.
    let db_suffix = format!(" {}", app.i18n.t("common.db"));
    let hz_suffix = format!(" {}", app.i18n.t("common.hz"));
    // Vendor level rows are bare numbers; only voice-clarity noise uses dB and only
    // the bass cutoff uses Hz (see findings-renderer.md §7.3).
    let level_suffix = String::new();
    let mode_music = app.i18n.t("effect.mode.music").to_string();
    let mode_voice = app.i18n.t("effect.mode.voice").to_string();

    let mut enabled = current.enabled;
    let mut level = current.level;
    let mut cutoff = current.cutoff_hz;
    let mut mode = current.mode;
    let mut noise = current.noise_suppression;
    let mut reset = false;

    widgets::card(ui, palette, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(app.i18n.t(kind.title_key()))
                    .strong()
                    .size(15.0)
                    .color(palette.text),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(caps, |ui| {
                    widgets::toggle(ui, palette, &mut enabled);
                });
            });
        });
        ui.label(
            RichText::new(app.i18n.t(kind.description_key()))
                .size(11.5)
                .color(palette.text_weak),
        );
        ui.add_space(6.0);

        let controls = caps && enabled;
        match kind {
            EffectKind::Brilliance => {
                widgets::slider_row(
                    ui,
                    palette,
                    app.i18n.t("effect.level"),
                    &mut level,
                    0..=10,
                    &level_suffix,
                    controls,
                );
            }
            EffectKind::VirtualBass => {
                widgets::slider_row(
                    ui,
                    palette,
                    app.i18n.t("effect.level"),
                    &mut level,
                    0..=10,
                    &level_suffix,
                    controls,
                );
                if widgets::slider_row(
                    ui,
                    palette,
                    app.i18n.t("effect.cutoff"),
                    &mut cutoff,
                    100..=400,
                    &hz_suffix,
                    controls,
                ) {
                    // The legacy C-Media slider steps 50 Hz between 100 and 400 Hz.
                    cutoff = ((cutoff + 25) / 50 * 50).clamp(100, 400);
                }
            }
            EffectKind::AdaptiveVolume => {
                widgets::slider_row(
                    ui,
                    palette,
                    app.i18n.t("effect.level"),
                    &mut level,
                    0..=100,
                    &level_suffix,
                    controls,
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(app.i18n.t("effect.mode"))
                        .size(12.5)
                        .color(palette.text_weak),
                );
                let options: [(u8, &str); 2] = [(0, mode_music.as_str()), (1, mode_voice.as_str())];
                ui.add_enabled_ui(controls, |ui| {
                    widgets::segmented(ui, palette, &mut mode, &options);
                });
            }
            EffectKind::VoiceClarity => {
                widgets::slider_row(
                    ui,
                    palette,
                    app.i18n.t("effect.level"),
                    &mut level,
                    0..=10,
                    &level_suffix,
                    controls,
                );
                if widgets::slider_row(
                    ui,
                    palette,
                    app.i18n.t("effect.noise"),
                    &mut noise,
                    0..=20,
                    &db_suffix,
                    controls,
                ) {
                    // The legacy slider marks 0/5/10/15/20 dB only.
                    noise = (noise / 5 * 5).clamp(0, 20);
                }
            }
        }

        // The vendor shows per-card reset only for brilliance (rewrites level) and
        // vocal clarity (rewrites level + noise); bass/volume-balance resets exist
        // only on the THX runtime.
        if matches!(kind, EffectKind::Brilliance | EffectKind::VoiceClarity) {
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(caps, |ui| {
                    let button = egui::Button::new(
                        RichText::new(format!("↺ {}", app.i18n.t("effect.reset")))
                            .size(11.0)
                            .color(palette.text_weak),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::NONE);
                    if ui.add(button).clicked() {
                        reset = true;
                    }
                });
            });
        }
    });

    if reset {
        // Reset restores the block's default values without disabling the block
        // (matches the vendor round-arrow behaviour).
        if current.level != fallback.level {
            app.apply_audio(AudioChange::EffectLevel {
                kind,
                level: fallback.level,
            });
        }
        match kind {
            EffectKind::VirtualBass => {
                if current.cutoff_hz != fallback.cutoff_hz {
                    app.apply_audio(AudioChange::BassCutoffHz(fallback.cutoff_hz));
                }
            }
            EffectKind::AdaptiveVolume => {
                if current.mode != fallback.mode {
                    app.apply_audio(AudioChange::AdaptiveMode(fallback.mode));
                }
            }
            EffectKind::VoiceClarity => {
                if current.noise_suppression != fallback.noise_suppression {
                    app.apply_audio(AudioChange::VoiceNoiseSuppression(
                        fallback.noise_suppression,
                    ));
                }
            }
            EffectKind::Brilliance => {}
        }
        return;
    }

    if enabled != current.enabled {
        app.apply_audio(AudioChange::EffectEnabled { kind, enabled });
    }
    if level != current.level {
        app.apply_audio(AudioChange::EffectLevel { kind, level });
    }
    match kind {
        EffectKind::VirtualBass => {
            if cutoff != current.cutoff_hz {
                app.apply_audio(AudioChange::BassCutoffHz(cutoff));
            }
        }
        EffectKind::AdaptiveVolume => {
            if mode != current.mode {
                app.apply_audio(AudioChange::AdaptiveMode(mode));
            }
        }
        EffectKind::VoiceClarity => {
            if noise != current.noise_suppression {
                app.apply_audio(AudioChange::VoiceNoiseSuppression(noise));
            }
        }
        EffectKind::Brilliance => {}
    }
}
