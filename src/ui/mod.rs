//! UI shell: custom titlebar, sidebar navigation, page content, dialogs and toasts.

pub mod pages;
pub mod theme;
pub mod widgets;

use egui::{Align, Color32, CornerRadius, Layout, RichText, Stroke, Vec2};

use crate::app::{ConfirmAction, MHubApp};
use crate::ui::pages::Page;
use crate::ui::theme::Palette;

const SIDEBAR_WIDTH: f32 = 216.0;
const HEADER_HEIGHT: f32 = 52.0;

/// Draw the whole application for one frame.
pub fn draw(app: &mut MHubApp, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    let palette = theme::palette(ctx, app.config.theme);

    draw_titlebar(app, ctx, palette);

    if app.connected() {
        let mut navigate: Option<Page> = None;
        egui::SidePanel::left("sidebar")
            .exact_width(SIDEBAR_WIDTH)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(palette.card)
                    .stroke(Stroke::NONE)
                    .inner_margin(egui::Margin::symmetric(12, 14)),
            )
            .show(ctx, |ui| {
                ui.add_space(4.0);
                for page in Page::all() {
                    let response = widgets::nav_item(
                        ui,
                        palette,
                        page.icon(),
                        app.i18n.t(page.title_key()),
                        app.page == *page,
                    );
                    if response.clicked() {
                        navigate = Some(*page);
                    }
                    ui.add_space(2.0);
                }
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(6.0);
                    widgets::divider(ui, palette);
                    draw_device_footer(app, ui, palette);
                });
            });
        if let Some(page) = navigate {
            app.set_page(page);
        }
    }

    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(palette.bg)
                .inner_margin(egui::Margin::symmetric(26, 18)),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if !app.connected() {
                        pages::connect::draw(app, ui, palette);
                    } else {
                        match app.page {
                            Page::Home => pages::home::draw(app, ui, palette),
                            Page::Routine => pages::routine::draw(app, ui, palette),
                            Page::Eq => pages::eq::draw(app, ui, palette),
                            Page::Effects => pages::effects::draw(app, ui, palette),
                            Page::Surround => pages::surround::draw(app, ui, palette),
                            Page::Mic => pages::mic::draw(app, ui, palette),
                            Page::Firmware => pages::firmware::draw(app, ui, palette),
                            Page::Settings => pages::settings::draw(app, ui, palette),
                        }
                    }
                    ui.add_space(24.0);
                });
        });

    draw_dialogs(app, ctx, palette);
    draw_toasts(app, ctx, palette);
}

fn draw_titlebar(app: &mut MHubApp, ctx: &egui::Context, palette: Palette) {
    egui::TopBottomPanel::top("titlebar")
        .exact_height(HEADER_HEIGHT)
        .frame(
            egui::Frame::new()
                .fill(palette.card)
                .inner_margin(egui::Margin::symmetric(16, 6))
                .stroke(Stroke::new(1.0_f32, palette.border)),
        )
        .show(ctx, |ui| {
            let drag_rect = ui.max_rect();
            let drag = ui.interact(
                drag_rect,
                egui::Id::new("titlebar-drag"),
                egui::Sense::drag(),
            );
            if drag.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }

            ui.horizontal_centered(|ui| {
                // Wordmark.
                let (logo_rect, _) = ui.allocate_exact_size(Vec2::splat(26.0_f32), egui::Sense::hover());
                ui.painter()
                    .rect_filled(logo_rect, CornerRadius::same(8), palette.accent);
                ui.painter().text(
                    logo_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "M",
                    egui::FontId::proportional(15.0),
                    palette.accent_text,
                );
                ui.add_space(8.0);
                ui.label(RichText::new("MCHOSE").strong().size(14.5).color(palette.text));
                ui.label(
                    RichText::new(app.i18n.t("app.name"))
                        .size(12.5)
                        .color(palette.text_weak),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if window_button(ui, palette, "✕").clicked() {
                        app.exit_requested = true;
                    }
                    if window_button(ui, palette, "▢").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                            !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                        ));
                    }
                    if window_button(ui, palette, "—").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    ui.add_space(6.0);

                    // Device status chips.
                    if let Some(status) = app.status.battery_label() {
                        let label = if app.status.battery.map(|b| b.charging).unwrap_or(false) {
                            format!("⚡ {status}")
                        } else {
                            status
                        };
                        widgets::chip(ui, palette, &label);
                    }
                    if let Some(firmware) = &app.status.headset_firmware {
                        let text = format!("FW {firmware}");
                        widgets::chip(ui, palette, &text);
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("⟳").size(14.0).color(palette.text))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        )
                        .on_hover_text(app.i18n.t("device.refresh"))
                        .clicked()
                    {
                        app.refresh_status();
                        app.hid.send(crate::backend::hid::HidCommand::Rescan);
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("⚙").size(14.0).color(palette.text))
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE),
                        )
                        .on_hover_text(app.i18n.t("nav.settings"))
                        .clicked()
                    {
                        app.set_page(Page::Settings);
                    }
                });
            });
        });
}

fn window_button(ui: &mut egui::Ui, palette: Palette, glyph: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(glyph).size(13.0).color(palette.text_weak))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(26.0, 24.0)),
    )
}

fn draw_device_footer(app: &MHubApp, ui: &mut egui::Ui, palette: Palette) {
    if let Some(device) = &app.selected_device {
        ui.label(
            RichText::new(device.product_name.clone())
                .size(12.0)
                .color(palette.text),
        );
        let connection = crate::model::device::family_for(device.vid, device.pid)
            .map(|f| f.name)
            .unwrap_or("MCHOSE");
        ui.label(RichText::new(connection).size(11.0).color(palette.text_weak));
        ui.label(
            RichText::new(format!("VID {:04X} · PID {:04X}", device.vid, device.pid))
                .size(10.5)
                .color(palette.text_weak),
        );
    } else {
        ui.label(
            RichText::new(app.i18n.t("device.no_device"))
                .size(11.5)
                .color(palette.text_weak),
        );
    }
}

fn draw_dialogs(app: &mut MHubApp, ctx: &egui::Context, palette: Palette) {
    // Save-preset dialog.
    if app.eq_editor.save_dialog_open {
        let mut open = true;
        egui::Window::new(app.i18n.t("eq.save_as"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                let mut name = app.eq_editor.name_buffer.clone();
                let response = ui.text_edit_singleline(&mut name);
                app.eq_editor.name_buffer = name;
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let name = app.eq_editor.name_buffer.clone();
                    app.save_preset(&name);
                    app.eq_editor.save_dialog_open = false;
                }
                ui.horizontal(|ui| {
                    if ui.button(app.i18n.t("eq.save")).clicked() {
                        let name = app.eq_editor.name_buffer.clone();
                        app.save_preset(&name);
                        app.eq_editor.save_dialog_open = false;
                    }
                    if ui.button(app.i18n.t("common.cancel")).clicked() {
                        app.eq_editor.save_dialog_open = false;
                    }
                });
            });
        if !open {
            app.eq_editor.save_dialog_open = false;
        }
    }

    // Confirmation dialog.
    if let Some(action) = app.confirm {
        let mut close = false;
        let (title, body) = match action {
            ConfirmAction::FactoryReset => (
                app.i18n.t("firmware.factory").to_string(),
                app.i18n.t("firmware.factory.confirm").to_string(),
            ),
            ConfirmAction::ResetSettings => (
                app.i18n.t("settings.reset").to_string(),
                app.i18n.t("settings.reset").to_string(),
            ),
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(RichText::new(body).color(palette.text));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(app.i18n.t("common.yes")).clicked() {
                        match action {
                            ConfirmAction::FactoryReset => {
                                app.hid.send(crate::backend::hid::HidCommand::FactoryReset(
                                    crate::backend::protocol::ResetTarget::All,
                                ));
                                let text = app.i18n.t("firmware.factory").to_string();
                                app.toast(text, false);
                            }
                            ConfirmAction::ResetSettings => {
                                app.config = crate::config::AppConfig::default();
                                app.schedule_save();
                            }
                        }
                        close = true;
                    }
                    if ui.button(app.i18n.t("common.no")).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            app.confirm = None;
        }
    }
}

fn draw_toasts(app: &mut MHubApp, ctx: &egui::Context, palette: Palette) {
    if app.toasts.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -18.0))
        .show(ctx, |ui| {
            for toast in &app.toasts {
                let border = if toast.is_error {
                    palette.danger
                } else {
                    palette.border
                };
                egui::Frame::new()
                    .fill(palette.card)
                    .stroke(Stroke::new(1.0_f32, border))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.label(RichText::new(toast.text.clone()).size(12.5).color(palette.text));
                    });
            }
        });
}
