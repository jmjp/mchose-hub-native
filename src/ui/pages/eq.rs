//! Equalizer page: the legacy 10-band gain editor plus the preset library.
//!
//! The connected C-Media device exposes plain integer band gains
//! (`GetEnable_EQ_GFX`, `getEQ_Slider(i)`, `setEQ_Slider(i, value)`), so this page is
//! a curve + slider editor over the semantic [`AudioChange`] messages instead of the
//! vendor's ECharts canvas. The preset list mirrors the shipped `eq.json`: the
//! embedded factory categories plus user presets saved through the Lead's dialog.
//!
//! Import/export deliberately stay simple: a JSON array of ten integer gains in the
//! application data directory ([`crate::config::data_dir`]). The original's cloud
//! sharing has no native equivalent, so a local file keeps the feature useful.

use egui::{CornerRadius, RichText, Stroke};

use crate::app::MHubApp;
use crate::model::audio::{AudioChange, EQ_BANDS, EQ_FREQUENCIES_HZ, EQ_GAIN_MAX, EQ_GAIN_MIN};
use crate::model::presets::{EqPreset, PresetCategory};
use crate::ui::theme::Palette;
use crate::ui::widgets;

/// File name used by the import/export buttons, relative to [`crate::config::data_dir`].
const EQ_EXPORT_FILE: &str = "eq_export.json";

/// Bands per column when the band sliders are split into two columns.
const BANDS_PER_COLUMN: usize = EQ_BANDS / 2;

/// Draw the equalizer page.
pub fn draw(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    page_header(app, ui, palette, "eq.title", "eq.subtitle");
    draw_band_card(app, ui, palette);
    ui.add_space(12.0);
    draw_preset_card(app, ui, palette);
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

/// Card one: enable switch, curve and the ten band sliders.
fn draw_band_card(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let caps_eq = app.caps.eq;
    let db_suffix = format!(" {}", app.i18n.t("common.db"));
    // Frequency labels are plain unit strings ("31 Hz"), computed once per frame.
    let labels: Vec<String> = EQ_FREQUENCIES_HZ
        .iter()
        .map(|hz| format!("{hz} {}", app.i18n.t("common.hz")))
        .collect();
    let lang = app.i18n.lang().to_string();
    let preset_name = app
        .presets
        .selected_preset()
        .map(|preset| preset.display_name(&lang).to_string())
        .unwrap_or_else(|| app.i18n.t("eq.no_preset").to_string());

    let mut eq_enabled = app.audio.eq_enabled;
    let mut bands = app.audio.eq_bands;

    widgets::card(ui, palette, |ui| {
        widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("eq.enabled"),
            None,
            &mut eq_enabled,
            caps_eq,
        );
        if !caps_eq {
            note(ui, palette, app.i18n.t("common.unsupported"));
        }
        ui.add_space(8.0);

        let bands_enabled = caps_eq && eq_enabled;
        widgets::eq_curve(ui, palette, &bands, bands_enabled);
        ui.add_space(10.0);

        ui.add_enabled_ui(bands_enabled, |ui| {
            if ui.available_width() >= 560.0 {
                ui.columns(2, |columns| {
                    for (column_index, column) in columns.iter_mut().enumerate() {
                        for row in 0..BANDS_PER_COLUMN {
                            let band = column_index * BANDS_PER_COLUMN + row;
                            let Some(band_label) = labels.get(band) else {
                                continue;
                            };
                            widgets::slider_row(
                                column,
                                palette,
                                band_label,
                                &mut bands[band],
                                EQ_GAIN_MIN..=EQ_GAIN_MAX,
                                &db_suffix,
                                true,
                            );
                        }
                    }
                });
            } else {
                #[allow(clippy::needless_range_loop)]
                for band in 0..EQ_BANDS {
                    let Some(band_label) = labels.get(band) else {
                        continue;
                    };
                    widgets::slider_row(
                        ui,
                        palette,
                        band_label,
                        &mut bands[band],
                        EQ_GAIN_MIN..=EQ_GAIN_MAX,
                        &db_suffix,
                        true,
                    );
                }
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(bands_enabled, |ui| {
                if ui.button(app.i18n.t("eq.flat")).clicked() {
                    bands = [0; EQ_BANDS];
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(&preset_name)
                        .size(11.0)
                        .color(palette.text_weak),
                );
            });
        });
    });

    if eq_enabled != app.audio.eq_enabled {
        app.apply_audio(AudioChange::EqEnabled(eq_enabled));
    }
    #[allow(clippy::needless_range_loop)]
    for band in 0..EQ_BANDS {
        if bands[band] != app.audio.eq_bands[band] {
            app.apply_audio(AudioChange::EqBand {
                band,
                value: bands[band],
            });
        }
    }
}

/// Card two: factory categories, user presets and the import/export/save actions.
fn draw_preset_card(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    let lang = app.i18n.lang().to_string();
    let selected = app.presets.selected.clone();
    let has_selected_user = selected
        .as_deref()
        .map(|id| app.presets.user.iter().any(|preset| preset.id == id))
        .unwrap_or(false);

    // Actions are collected here and applied after the card closure so the closure
    // only needs shared access to the application state.
    let mut clicked: Option<String> = None;
    let mut save_dialog = false;
    let mut delete_selected = false;
    let mut export = false;
    let mut import = false;

    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("eq.preset"), None);

        for category in &app.presets.factory {
            let title = category_title(app, category);
            ui.add_space(6.0);
            ui.label(RichText::new(title).strong().size(13.5).color(palette.text));
            preset_runs(
                ui,
                palette,
                &lang,
                selected.as_deref(),
                &category.presets,
                &mut clicked,
            );
        }

        ui.add_space(10.0);
        ui.label(
            RichText::new(app.i18n.t("eq.my"))
                .strong()
                .size(13.5)
                .color(palette.text),
        );
        if app.presets.user.is_empty() {
            // No user preset yet; the save-as button below is the next step.
            ui.label(RichText::new("—").size(11.5).color(palette.text_weak));
        } else {
            ui.horizontal_wrapped(|ui| {
                for preset in &app.presets.user {
                    preset_button(
                        ui,
                        palette,
                        &lang,
                        selected.as_deref(),
                        preset,
                        &mut clicked,
                    );
                }
            });
        }

        ui.add_space(10.0);
        widgets::divider(ui, palette);
        ui.horizontal_wrapped(|ui| {
            if ui.button(app.i18n.t("eq.save_as")).clicked() {
                save_dialog = true;
            }
            ui.add_enabled_ui(has_selected_user, |ui| {
                if ui.button(app.i18n.t("eq.delete")).clicked() {
                    delete_selected = true;
                }
            });
            if ui.button(app.i18n.t("eq.export")).clicked() {
                export = true;
            }
            ui.add_enabled_ui(app.caps.eq, |ui| {
                if ui.button(app.i18n.t("eq.import")).clicked() {
                    import = true;
                }
            });
        });
    });

    if let Some(id) = clicked {
        app.apply_preset(&id);
    }
    if save_dialog {
        app.eq_editor.save_dialog_open = true;
    }
    if delete_selected {
        if let Some(id) = app.presets.selected.clone() {
            if app.presets.delete_user(&id) {
                app.config.selected_preset = None;
                app.schedule_save();
            }
        }
    }
    if export {
        export_bands(app);
    }
    if import {
        import_bands(app);
    }
}

/// Localized heading for a factory category (`eq.game` / `eq.music`), falling back to
/// the category's own embedded name for unknown future categories.
fn category_title(app: &MHubApp, category: &PresetCategory) -> String {
    match category.id.as_str() {
        "game" => app.i18n.t("eq.game").to_string(),
        "music" => app.i18n.t("eq.music").to_string(),
        _ => category.name.for_lang(app.i18n.lang()).to_string(),
    }
}

/// Render consecutive presets that share a `group` as one wrapped row with a small
/// caption, preserving the order of the shipped preset file.
fn preset_runs(
    ui: &mut egui::Ui,
    palette: Palette,
    lang: &str,
    selected: Option<&str>,
    presets: &[EqPreset],
    clicked: &mut Option<String>,
) {
    let mut start = 0;
    while start < presets.len() {
        let group = presets[start].group.clone();
        let mut end = start + 1;
        while end < presets.len() && presets[end].group == group {
            end += 1;
        }
        if let Some(group) = &group {
            ui.add_space(3.0);
            ui.label(RichText::new(group).size(10.5).color(palette.text_weak));
        }
        ui.horizontal_wrapped(|ui| {
            for preset in &presets[start..end] {
                preset_button(ui, palette, lang, selected, preset, clicked);
            }
        });
        start = end;
    }
}

/// One preset pill; the selected preset is filled with the accent colour.
fn preset_button(
    ui: &mut egui::Ui,
    palette: Palette,
    lang: &str,
    selected: Option<&str>,
    preset: &EqPreset,
    clicked: &mut Option<String>,
) {
    let is_selected = selected == Some(preset.id.as_str());
    let text_color = if is_selected {
        palette.accent_text
    } else {
        palette.text
    };
    let fill = if is_selected {
        palette.accent
    } else {
        palette.card_hover
    };
    let border = if is_selected {
        palette.accent
    } else {
        palette.border
    };
    let button = egui::Button::new(
        RichText::new(preset.display_name(lang))
            .size(12.5)
            .color(text_color),
    )
    .fill(fill)
    .stroke(Stroke::new(1.0_f32, border))
    .corner_radius(CornerRadius::same(9));
    if ui
        .add(button)
        .on_hover_text(preset.display_name(lang))
        .clicked()
    {
        *clicked = Some(preset.id.clone());
    }
}

/// Write the current bands as a pretty-printed JSON array of ten integers.
fn export_bands(app: &mut MHubApp) {
    let path = crate::config::data_dir().join(EQ_EXPORT_FILE);
    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            report_error(app, &err.to_string());
            return;
        }
    }
    let json = match serde_json::to_string_pretty(&app.audio.eq_bands) {
        Ok(json) => json,
        Err(err) => {
            report_error(app, &err.to_string());
            return;
        }
    };
    match std::fs::write(&path, json) {
        Ok(()) => {
            let label = app.i18n.t("eq.export").to_string();
            app.toast(format!("{label}: {}", path.display()), false);
        }
        Err(err) => report_error(app, &err.to_string()),
    }
}

/// Read a previously exported band array and apply it band by band.
///
/// Missing trailing bands keep their current value; extra values are ignored, so a
/// slightly older or newer export file still imports predictably.
fn import_bands(app: &mut MHubApp) {
    let path = crate::config::data_dir().join(EQ_EXPORT_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            report_error(app, &format!("{}: {err}", path.display()));
            return;
        }
    };
    let values = match serde_json::from_str::<Vec<i32>>(&text) {
        Ok(values) => values,
        Err(err) => {
            report_error(app, &format!("{}: {err}", path.display()));
            return;
        }
    };
    if values.is_empty() {
        report_error(app, &format!("{}: empty band list", path.display()));
        return;
    }
    for (band, value) in values.iter().take(EQ_BANDS).enumerate() {
        app.apply_audio(AudioChange::EqBand {
            band,
            value: *value,
        });
    }
    let label = app.i18n.t("eq.import").to_string();
    app.toast(format!("{label}: {}", path.display()), false);
}

/// Log and surface an import/export failure without panicking.
fn report_error(app: &mut MHubApp, detail: &str) {
    log::warn!("EQ preset file error: {detail}");
    let label = app.i18n.t("common.error").to_string();
    app.toast(format!("{label}: {detail}"), true);
}
