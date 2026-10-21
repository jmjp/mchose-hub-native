//! Virtual 7.1 surround page (Xear).
//!
//! The original legacy device uses the C-Media `XearSurround71Control` handler
//! (not an `IPropertyStore` key), so writing it requires the vendor control
//! handler. This page therefore documents the limitation and keeps the speaker
//! test workflow (which is fully implemented through the system mixer + vendor
//! WAV assets).

use egui::{RichText, Vec2};

use crate::app::MHubApp;
use crate::model::audio::{RoomSize, SurroundMode, TestChannel};
use crate::ui::theme::Palette;
use crate::ui::widgets;

pub fn draw(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    ui.label(
        RichText::new(app.i18n.t("surround.title"))
            .strong()
            .size(22.0)
            .color(palette.text),
    );
    ui.add_space(14.0);

    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("surround.title"), None);
        let mut enabled = app.audio.surround.enabled;
        widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("surround.enable"),
            Some(app.i18n.t("surround.note")),
            &mut enabled,
            app.caps.surround,
        );
        widgets::divider(ui, palette);

        let mut mode = app.audio.surround.mode;
        ui.label(
            RichText::new(app.i18n.t("surround.mode"))
                .size(12.5)
                .color(palette.text_weak),
        );
        ui.add_enabled_ui(app.caps.surround, |ui| {
            let labels = [
                (SurroundMode::Music, app.i18n.t("surround.mode.music")),
                (SurroundMode::Movie, app.i18n.t("surround.mode.movie")),
            ];
            if widgets::segmented(ui, palette, &mut mode, &labels) {
                app.apply_audio(crate::model::audio::AudioChange::SurroundMode(mode));
            }
        });

        ui.add_space(8.0);
        let mut room = app.audio.surround.room;
        ui.label(
            RichText::new(app.i18n.t("surround.room"))
                .size(12.5)
                .color(palette.text_weak),
        );
        ui.add_enabled_ui(app.caps.surround, |ui| {
            let labels = [
                (RoomSize::Small, app.i18n.t("surround.room.small")),
                (RoomSize::Medium, app.i18n.t("surround.room.mid")),
                (RoomSize::Large, app.i18n.t("surround.room.big")),
            ];
            if widgets::segmented(ui, palette, &mut room, &labels) {
                app.apply_audio(crate::model::audio::AudioChange::SurroundRoom(room));
            }
        });
        if !app.caps.surround {
            ui.add_space(4.0);
            ui.label(
                RichText::new(app.i18n.t("common.unsupported"))
                    .size(11.0)
                    .color(palette.warning),
            );
        }
    });

    ui.add_space(14.0);
    draw_speaker_test(app, ui, palette);
}

fn draw_speaker_test(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let mut play: Option<TestChannel> = None;
    let mut stop = false;
    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("surround.test"), None);
        let width = ui.available_width();
        let columns = ((width / 150.0).floor() as usize).max(2);
        for chunk in TestChannel::ALL.chunks(columns) {
            ui.horizontal(|ui| {
                for channel in chunk {
                    let selected = app.test_channel == Some(*channel);
                    let label = app.i18n.t(channel.label_key());
                    let fill = if selected {
                        palette.accent
                    } else {
                        palette.card_hover
                    };
                    let color = if selected {
                        palette.accent_text
                    } else {
                        palette.text
                    };
                    let button = egui::Button::new(RichText::new(label).size(12.0).color(color))
                        .fill(fill)
                        .corner_radius(egui::CornerRadius::same(10))
                        .min_size(Vec2::new(130.0, 34.0));
                    if ui.add(button).clicked() {
                        play = Some(*channel);
                    }
                }
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button(app.i18n.t("surround.test.stop")).clicked() {
                stop = true;
            }
            if !app.caps.test_sound {
                ui.label(
                    RichText::new(app.i18n.t("common.unsupported"))
                        .size(11.0)
                        .color(palette.warning),
                );
            }
        });
    });
    if stop {
        app.stop_test();
    }
    if let Some(channel) = play {
        app.play_test(channel);
    }
}
