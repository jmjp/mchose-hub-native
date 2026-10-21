//! Microphone page: capture volume, DSP toggles, monitoring and recovery hints.
//!
//! The microphone DSP values are written through the same C-Media ConfLib engine as
//! the speaker effects; endpoint volume/mute use Core Audio. Microphone monitoring is
//! bridged to the vendor `MicListen.dll` sidecar when it is installed.

use egui::RichText;

use crate::app::MHubApp;
use crate::model::audio::AudioChange;
use crate::ui::theme::Palette;
use crate::ui::widgets;

/// Draw the microphone page.
pub fn draw(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    page_header(app, ui, palette, "mic.title", "mic.subtitle");
    draw_volume_card(app, ui, palette);
    ui.add_space(12.0);
    draw_dsp_card(app, ui, palette);
    ui.add_space(12.0);
    draw_listen_card(app, ui, palette);
    ui.add_space(12.0);
    draw_recovery_card(app, ui, palette);
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

/// Single-line warning/unsupported note in the palette's warning colour.
fn note(ui: &mut egui::Ui, palette: Palette, text: &str) {
    ui.label(RichText::new(text).size(11.0).color(palette.warning));
}

/// Capture volume and mute (Core Audio endpoint values).
fn draw_volume_card(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let caps = app.caps.mic_volume;
    let mut volume = app.audio.volume.mic;
    let mut muted = app.audio.volume.mic_muted;

    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("mic.volume"), None);
        widgets::slider_row_f32(ui, palette, "", &mut volume, "", caps);
        widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("routine.mute"),
            None,
            &mut muted,
            caps,
        );
        if !caps {
            note(ui, palette, app.i18n.t("common.unsupported"));
        }
    });

    if volume != app.audio.volume.mic {
        app.apply_audio(AudioChange::MicVolume(volume));
    }
    if muted != app.audio.volume.mic_muted {
        app.apply_audio(AudioChange::MicMute(muted));
    }
}

/// AI noise reduction, steady noise suppression and magic voice (with its type).
fn draw_dsp_card(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let caps = app.caps.mic_dsp;
    let mut ai_noise_reduction = app.audio.mic.ai_noise_reduction;
    let mut noise_suppression = app.audio.mic.noise_suppression;
    let mut magic_voice = app.audio.mic.magic_voice;
    let mut magic_voice_type = app.audio.mic.magic_voice_type;

    // Owned labels keep the card closure free of translator borrows.
    let type_labels: [(u8, String); 4] = [
        (0, app.i18n.t("routine.magic_voice.monster").to_string()),
        (1, app.i18n.t("routine.magic_voice.cartoon").to_string()),
        (2, app.i18n.t("routine.magic_voice.male").to_string()),
        (3, app.i18n.t("routine.magic_voice.female").to_string()),
    ];

    widgets::card(ui, palette, |ui| {
        widgets::section_header(
            ui,
            palette,
            app.i18n.t("routine.effects"),
            Some(app.i18n.t("routine.effects.hint")),
        );
        widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("mic.ai_nr"),
            Some(app.i18n.t("routine.ai_nr.desc")),
            &mut ai_noise_reduction,
            caps,
        );
        widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("mic.nr"),
            Some(app.i18n.t("routine.nr.desc")),
            &mut noise_suppression,
            caps,
        );
        widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("mic.magic_voice"),
            Some(app.i18n.t("routine.magic_voice.desc")),
            &mut magic_voice,
            caps,
        );

        if magic_voice {
            ui.add_space(4.0);
            ui.label(
                RichText::new(app.i18n.t("routine.magic_voice.type"))
                    .size(12.5)
                    .color(palette.text_weak),
            );
            let options: Vec<(u8, &str)> = type_labels
                .iter()
                .map(|(value, label)| (*value, label.as_str()))
                .collect();
            ui.add_enabled_ui(caps, |ui| {
                widgets::segmented(ui, palette, &mut magic_voice_type, &options);
            });
        }

        if !caps {
            note(ui, palette, app.i18n.t("common.unsupported"));
        }
    });

    if ai_noise_reduction != app.audio.mic.ai_noise_reduction {
        app.apply_audio(AudioChange::AiNoiseReduction(ai_noise_reduction));
    }
    if noise_suppression != app.audio.mic.noise_suppression {
        app.apply_audio(AudioChange::NoiseSuppression(noise_suppression));
    }
    if magic_voice != app.audio.mic.magic_voice {
        app.apply_audio(AudioChange::MagicVoiceEnabled(magic_voice));
    }
    if magic_voice_type != app.audio.mic.magic_voice_type {
        app.apply_audio(AudioChange::MagicVoiceType(magic_voice_type));
    }
}

/// Microphone monitoring card, permanently disabled in the native build.
fn draw_listen_card(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let listen_enabled = app.caps.mic_listen;
    let mut listen = app.audio.mic.mic_listen;

    widgets::card(ui, palette, |ui| {
        widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("mic.listen"),
            Some(app.i18n.t("routine.mic_listen.desc")),
            &mut listen,
            listen_enabled,
        );
        if !listen_enabled {
            note(ui, palette, app.i18n.t("routine.mic_listen.unsupported"));
        }
    });

    if listen != app.audio.mic.mic_listen {
        app.apply_audio(AudioChange::MicListen(listen));
    }
}

/// Recovery hint when Windows disables the capture endpoint.
fn draw_recovery_card(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let mut open_sound = false;
    let mut refresh = false;

    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("routine.windows_sound"), None);
        ui.label(
            RichText::new(app.i18n.t("mic.disabled"))
                .size(12.0)
                .color(palette.warning),
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button(app.i18n.t("mic.open_sound")).clicked() {
                open_sound = true;
            }
            if ui.button(app.i18n.t("common.refresh")).clicked() {
                refresh = true;
            }
        });
    });

    if open_sound {
        open_windows_sound(app);
    }
    if refresh {
        app.refresh_status();
    }
}

/// Open the Windows "Sound" settings page; failures are logged and toasted.
fn open_windows_sound(app: &mut MHubApp) {
    #[cfg(windows)]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "ms-settings:sound"])
        .spawn();
    #[cfg(not(windows))]
    let result: std::io::Result<std::process::Child> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Windows sound settings are only available on Windows",
    ));

    if let Err(err) = result {
        log::warn!("cannot open Windows sound settings: {err}");
        let label = app.i18n.t("common.error").to_string();
        app.toast(format!("{label}: {err}"), true);
    }
}
