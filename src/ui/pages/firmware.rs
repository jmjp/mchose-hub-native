//! Firmware & power page.
//!
//! Firmware flashing is deliberately out of scope (vendor OTA SDK + per-device
//! images). The page reports versions and exposes the power-management HID
//! commands that could be implemented, clearly marked until verified on hardware.

use egui::RichText;

use crate::app::{ConfirmAction, MHubApp};
use crate::backend::hid::HidCommand;
use crate::ui::theme::Palette;
use crate::ui::widgets;

pub fn draw(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    ui.label(
        RichText::new(app.i18n.t("firmware.title"))
            .strong()
            .size(22.0)
            .color(palette.text),
    );
    ui.label(
        RichText::new(app.i18n.t("firmware.subtitle"))
            .size(13.0)
            .color(palette.text_weak),
    );
    ui.add_space(14.0);

    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("firmware.title"), None);
        widgets::kv_row(
            ui,
            palette,
            app.i18n.t("firmware.headset"),
            &app.status
                .headset_firmware
                .clone()
                .unwrap_or_else(|| app.i18n.t("device.unknown").to_string()),
        );
        widgets::kv_row(
            ui,
            palette,
            app.i18n.t("firmware.dongle"),
            &app.status
                .dongle_firmware
                .clone()
                .unwrap_or_else(|| app.i18n.t("device.unknown").to_string()),
        );
        widgets::kv_row(
            ui,
            palette,
            app.i18n.t("firmware.driver"),
            app.i18n.t("device.unknown"),
        );
        widgets::divider(ui, palette);
        ui.label(
            RichText::new(app.i18n.t("firmware.ota_note"))
                .size(11.5)
                .color(palette.warning),
        );
        if ui.button(app.i18n.t("device.refresh")).clicked() {
            app.refresh_status();
        }
    });

    ui.add_space(12.0);
    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("firmware.power"), None);
        let mut prompt = app.status.prompt_volume.unwrap_or(8) as i32;
        let prompt_changed = widgets::slider_row(
            ui,
            palette,
            app.i18n.t("routine.volume"),
            &mut prompt,
            1..=16,
            "",
            app.caps.hid_commands,
        );
        if prompt_changed {
            app.hid.send(HidCommand::SetPromptVolume(prompt as u8));
        }

        let mut shutdown = app.status.auto_shutdown_minutes.unwrap_or(0) as i32;
        ui.label(
            RichText::new(app.i18n.t("firmware.auto_shutdown"))
                .size(12.5)
                .color(palette.text_weak),
        );
        ui.add_enabled_ui(app.caps.hid_commands, |ui| {
            let minutes = [0i32, 5, 10, 20, 30, 60];
            let labels: Vec<(i32, String)> = minutes
                .iter()
                .map(|m| {
                    let text = if *m == 0 {
                        app.i18n.t("firmware.never").to_string()
                    } else {
                        format!("{m} {}", app.i18n.t("firmware.minutes"))
                    };
                    (*m, text)
                })
                .collect();
            let mut selected = shutdown;
            let options: Vec<(i32, &str)> = labels
                .iter()
                .map(|(v, s)| (*v, s.as_str()))
                .collect();
            if widgets::segmented(ui, palette, &mut selected, &options) {
                shutdown = selected;
                app.hid.send(HidCommand::SetAutoShutdown(shutdown as u16));
            }
        });
        if !app.caps.hid_commands {
            ui.add_space(4.0);
            ui.label(
                RichText::new(app.i18n.t("firmware.not_supported"))
                    .size(11.0)
                    .color(palette.text_weak),
            );
        }
    });

    ui.add_space(12.0);
    widgets::card(ui, palette, |ui| {
        widgets::section_header(
            ui,
            palette,
            app.i18n.t("firmware.factory"),
            Some(app.i18n.t("firmware.factory.desc")),
        );
        ui.add_enabled_ui(app.caps.device_power, |ui| {
            if ui
                .add(
                    egui::Button::new(
                        RichText::new(app.i18n.t("firmware.factory")).color(palette.danger),
                    )
                    .fill(palette.card)
                    .stroke(egui::Stroke::new(1.0_f32, palette.danger)),
                )
                .clicked()
            {
                app.confirm = Some(ConfirmAction::FactoryReset);
            }
        });
        if !app.caps.device_power {
            ui.label(
                RichText::new(app.i18n.t("firmware.not_supported"))
                    .size(11.0)
                    .color(palette.text_weak),
            );
        }
    });
}
