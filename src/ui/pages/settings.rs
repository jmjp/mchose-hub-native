//! Settings, about and log page.

use egui::RichText;

use crate::app::{ConfirmAction, MHubApp};
use crate::config::{self, ThemeMode};
use crate::ui::theme::Palette;
use crate::ui::widgets;

pub fn draw(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    ui.label(
        RichText::new(app.i18n.t("settings.title"))
            .strong()
            .size(22.0)
            .color(palette.text),
    );
    ui.add_space(14.0);

    draw_appearance(app, ui, palette);
    ui.add_space(12.0);
    draw_startup(app, ui, palette);
    ui.add_space(12.0);
    draw_data(app, ui, palette);
    ui.add_space(12.0);
    draw_about(app, ui, palette);
    ui.add_space(12.0);
    draw_log(app, ui, palette);
}

fn draw_appearance(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("settings.appearance"), None);
        ui.label(
            RichText::new(app.i18n.t("settings.theme"))
                .size(12.5)
                .color(palette.text_weak),
        );
        let mut theme = app.config.theme;
        let labels = [
            (ThemeMode::Light, app.i18n.t("settings.theme.light")),
            (ThemeMode::Dark, app.i18n.t("settings.theme.dark")),
            (ThemeMode::System, app.i18n.t("settings.theme.system")),
        ];
        if widgets::segmented(ui, palette, &mut theme, &labels) {
            app.set_theme(theme);
        }
        widgets::divider(ui, palette);
        ui.label(
            RichText::new(app.i18n.t("settings.language"))
                .size(12.5)
                .color(palette.text_weak),
        );
        let current_lang = app.i18n.lang().to_string();
        let mut selected = current_lang.clone();
        let languages: Vec<(String, String)> = crate::i18n::I18n::available()
            .map(|(code, label)| (code.to_string(), label.to_string()))
            .collect();
        egui::ComboBox::from_id_salt("language")
            .selected_text(
                languages
                    .iter()
                    .find(|(code, _)| *code == current_lang)
                    .map(|(_, label)| label.clone())
                    .unwrap_or(current_lang.clone()),
            )
            .show_ui(ui, |ui| {
                for (code, label) in &languages {
                    if ui
                        .selectable_value(&mut selected, code.clone(), label)
                        .clicked()
                    {
                        app.set_language(code);
                    }
                }
            });
    });
}

fn draw_startup(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    widgets::card(ui, palette, |ui| {
        #[cfg(windows)]
    {
    widgets::section_header(ui, palette, app.i18n.t("settings.audio"), None);
    if let Some(device) = app.selected_device.as_ref() {
        if crate::backend::driverinstall::driver_dir_label(device.pid).is_some() {
            ui.label(
                RichText::new(app.i18n.t("settings.driver_hint"))
                    .size(11.5)
                    .color(palette.text_weak),
            );
            ui.add_space(4.0);
            if ui.button(app.i18n.t("settings.install_driver")).clicked() {
                match crate::backend::driverinstall::install_driver(device.pid) {
                    Ok(message) => app.toast(message, false),
                    Err(err) => app.toast(
                        format!("{}: {err}", app.i18n.t("settings.driver_failed")),
                        true,
                    ),
                }
            }
        }
    } else {
        ui.label(
            RichText::new(app.i18n.t("device.no_device"))
                .size(11.5)
                .color(palette.text_weak),
        );
    }
    ui.add_space(10.0);
    }

    #[cfg(not(windows))]
    {
        ui.label(
            RichText::new(app.i18n.t("settings.platform_note"))
                .size(11.5)
                .color(palette.warning),
        );
        ui.add_space(8.0);
    }

    widgets::section_header(ui, palette, app.i18n.t("settings.startup"), None);
        let mut auto_start = app.config.auto_start;
        if widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("settings.auto_start"),
            None,
            &mut auto_start,
            true,
        )
        .clicked()
        {
            app.config.auto_start = auto_start;
            if let Err(err) = crate::autostart::set(auto_start) {
                log::warn!("cannot update auto-start entry: {err}");
                app.toast(err, true);
            }
            app.schedule_save();
        }
        let mut minimized = app.config.start_minimized;
        if widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("settings.start_minimized"),
            None,
            &mut minimized,
            true,
        )
        .clicked()
        {
            app.config.start_minimized = minimized;
            app.schedule_save();
        }
        let mut close_to_tray = app.config.close_to_tray;
        if widgets::toggle_row(
            ui,
            palette,
            app.i18n.t("settings.close_to_tray"),
            Some(app.i18n.t("settings.close_to_tray_hint")),
            &mut close_to_tray,
            app.tray_active,
        )
        .clicked()
        {
            app.config.close_to_tray = close_to_tray;
            app.schedule_save();
        }
        if !app.tray_active {
            ui.label(
                RichText::new(app.i18n.t("common.unsupported"))
                    .size(11.0)
                    .color(palette.text_weak),
            );
        }
    });
}

fn draw_data(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("settings.data"), None);
        widgets::kv_row(ui, palette, app.i18n.t("settings.backend"), &app.backend_status());
        widgets::kv_row(ui, palette, app.i18n.t("settings.data"), &config::data_dir_display());
        ui.horizontal(|ui| {
            if ui.button(app.i18n.t("settings.open_data")).clicked() {
                open_path(&config::data_dir());
            }
            if ui.button(app.i18n.t("settings.open_log")).clicked() {
                if let Some(path) = crate::logging::log_file() {
                    open_path(&path);
                }
            }
            if ui.button(app.i18n.t("settings.reset")).clicked() {
                app.confirm = Some(ConfirmAction::ResetSettings);
            }
        });
    });
}

fn draw_about(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("settings.about"), None);
        widgets::kv_row(ui, palette, app.i18n.t("settings.version"), env!("CARGO_PKG_VERSION"));
        ui.label(
            RichText::new(app.i18n.t("settings.about_text"))
                .size(12.0)
                .color(palette.text),
        );
        ui.add_space(6.0);
        widgets::section_header(ui, palette, app.i18n.t("settings.compat"), None);
        ui.label(
            RichText::new(app.i18n.t("settings.compat_text"))
                .size(11.5)
                .color(palette.text_weak),
        );
    });
}

fn draw_log(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    widgets::card(ui, palette, |ui| {
        ui.horizontal(|ui| {
            widgets::section_header(ui, palette, app.i18n.t("log.title"), None);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |_ui| {
                let _ = app.log_dirty;
            });
        });
        let entries = crate::logging::entries();
        if entries.is_empty() {
            ui.label(
                RichText::new(app.i18n.t("log.empty"))
                    .size(11.5)
                    .color(palette.text_weak),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for entry in entries.iter().rev().take(120) {
                    let color = match entry.level {
                        log::Level::Error => palette.danger,
                        log::Level::Warn => palette.warning,
                        _ => palette.text_weak,
                    };
                    ui.label(
                        RichText::new(format!(
                            "[{}] {}: {}",
                            entry.level, entry.target, entry.message
                        ))
                        .size(10.5)
                        .color(color)
                        .monospace(),
                    );
                }
            });
    });
}

fn open_path(path: &std::path::Path) {
    #[cfg(windows)]
    let command = "explorer";
    #[cfg(target_os = "linux")]
    let command = "xdg-open";
    #[cfg(target_os = "macos")]
    let command = "open";
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    let command = "true";
    let result = std::process::Command::new(command).arg(path.as_os_str()).spawn();
    if let Err(err) = result {
        log::warn!("cannot open {}: {err}", path.display());
    }
}

