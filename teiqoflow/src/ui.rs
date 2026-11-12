use crate::audio;
use crate::config::{Config, InsertMethod};
use crate::engine::EngineMsg;
use crate::hotkey::combo_name;
use crate::languages::{LANGUAGES, name_for};
use crate::state::{Capture, Shared, StatusLevel, lock};
use crate::i18n::{t, tf};
use crate::stt::{Kind, MODELS, is_downloaded, model_info};
use crossbeam_channel::Sender;
use egui::{
    Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, Rect,
    RichText, Sense, Stroke, Ui, Vec2, ViewportCommand, pos2, vec2,
};
use std::sync::Arc;

use crate::style::*;


#[derive(PartialEq, Clone, Copy)]
enum Page {
    Home,
    History,
    Settings,
}

pub struct App {
    shared: Arc<Shared>,
    engine: Sender<EngineMsg>,
    page: Page,
    mics: Vec<String>,
    logo: egui::TextureHandle,
    _tray: Option<tray_icon::TrayIcon>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, shared: Arc<Shared>, engine: Sender<EngineMsg>, tray: Option<tray_icon::TrayIcon>) -> Self {
        setup_style(&cc.egui_ctx);
        let rgba = crate::icon::app_icon_rgba(96);
        let logo = cc.egui_ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([96, 96], &rgba), egui::TextureOptions::LINEAR);
        Self { shared, engine, page: Page::Home, mics: audio::list_microphones(), logo, _tray: tray }
    }
}



fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(20))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text.to_lowercase()).font(semibold(11.0)).color(MUTED).extra_letter_spacing(1.0));
}

fn muted(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(13.0).color(MUTED)).wrap());
}

fn keycap(ui: &mut Ui, text: &str, size: f32) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), semibold(size), TEXT);
    let pad = vec2(14.0, 7.0);
    let (rect, _) = ui.allocate_exact_size(galley.size() + pad * 2.0 + vec2(0.0, 3.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 9.0, Color32::from_rgb(0x3A, 0x3A, 0x4D));
    let top = Rect::from_min_max(rect.min, rect.max - vec2(0.0, 3.0));
    p.rect_filled(top, 9.0, PANEL_2);
    p.rect_stroke(top, 9.0, Stroke::new(1.0, Color32::from_rgb(0x3A, 0x3A, 0x4D)), egui::StrokeKind::Inside);
    p.galley(top.min + pad, galley, TEXT);
}


fn setting_row(ui: &mut Ui, title: &str, subtitle: &str, control: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let right_w = 300.0f32.min(ui.available_width() * 0.5);
        ui.allocate_ui_with_layout(vec2(ui.available_width() - right_w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(title).font(semibold(15.0)));
            if !subtitle.is_empty() {
                muted(ui, subtitle);
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), control);
    });
}

fn combo<T: PartialEq + Clone>(ui: &mut Ui, id: &str, width: f32, value: &mut T, options: &[(T, String)]) {
    let current = options.iter().find(|(v, _)| v == value).map(|(_, l)| l.clone()).unwrap_or_default();
    egui::ComboBox::from_id_salt(id)
        .width(width)
        .height(360.0)
        // Long labels would otherwise widen the box over the row's description.
        .truncate()
        .selected_text(RichText::new(current).size(14.0))
        .show_ui(ui, |ui| {
            for (v, label) in options {
                ui.selectable_value(value, v.clone(), RichText::new(label).size(14.0));
            }
        });
}

// ------------------------------------------------------------------ app

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }

        let mut cfg = self.shared.cfg();
        let before = cfg.clone();

        // Frameless transparent window: we draw one smooth rounded edge ourselves.
        let full = ctx.content_rect();
        window_background(ui);

        egui::Panel::left("sidebar")
            .exact_size(220.0)
            .resizable(false)
            .frame(Frame::new().inner_margin(Margin { left: 18, right: 14, top: 22, bottom: 18 }))
            .show(ui, |ui| self.sidebar(ui));

        // Divider between sidebar and content.
        let x = full.left() + 220.0;
        ui.painter().vline(x, full.y_range().shrink(1.0), Stroke::new(1.0, BORDER));

        egui::CentralPanel::no_frame().show(ui, |ui| {
            self.titlebar(ui);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                Frame::new().inner_margin(Margin { left: 28, right: 28, top: 0, bottom: 28 }).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    match self.page {
                        Page::Home => self.home(ui, &mut cfg),
                        Page::History => self.history(ui, &cfg),
                        Page::Settings => self.settings(ui, &mut cfg),
                    }
                });
            });
        });

        if cfg != before {
            if cfg.ui_lang != before.ui_lang {
                crate::i18n::set_language(&cfg.ui_lang);
            }
            if cfg.model != before.model {
                let _ = self.engine.send(EngineMsg::LoadModel(cfg.model.clone()));
            }
            let mut g = lock(&self.shared.cfg);
            // History is owned by the engine thread; keep its latest copy.
            let history = std::mem::take(&mut g.history);
            *g = Config { history, ..cfg };
            g.save();
        }

        // Keep the status line live while something is loading.
        if self.shared.status().1 == StatusLevel::Busy || *lock(&self.shared.capture) == Capture::Waiting {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

impl App {
    fn titlebar(&mut self, ui: &mut Ui) {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click_and_drag());
        if resp.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        let btn = |ui: &mut Ui, x: f32, label: &str, hover: Color32| -> bool {
            let r = Rect::from_center_size(pos2(x, rect.center().y), vec2(36.0, 28.0));
            let resp = ui.interact(r, ui.make_persistent_id(label), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 8.0, hover);
            }
            let color = if resp.hovered() { TEXT } else { MUTED };
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, FontId::proportional(14.0), color);
            resp.clicked()
        };
        if btn(ui, rect.right() - 26.0, "✕", ERR) {
            ui.ctx().send_viewport_cmd(ViewportCommand::Visible(false));
        }
        if btn(ui, rect.right() - 66.0, "—", PANEL_2) {
            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
        }
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(vec2(34.0, 34.0)));
            ui.label(RichText::new("teiqoflow").font(semibold(19.0)));
        });
        ui.add_space(26.0);
        for (page, icon, label) in [(Page::Home, "⌂", t("Home")), (Page::History, "☰", t("History")), (Page::Settings, "⚙", t("Settings"))] {
            let selected = self.page == page;
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::click());
            if selected || resp.hovered() {
                ui.painter().rect_filled(rect, 10.0, if selected { PANEL_2 } else { PANEL });
            }
            if selected {
                let bar = Rect::from_min_size(rect.left_top() + vec2(0.0, 10.0), vec2(3.0, 20.0));
                gradient_rect(ui, bar, 1.5, ACCENT_1, ACCENT_2);
            }
            let color = if selected || resp.hovered() { TEXT } else { MUTED };
            ui.painter().text(rect.left_center() + vec2(16.0, 0.0), egui::Align2::LEFT_CENTER, icon, FontId::proportional(16.0), color);
            ui.painter().text(rect.left_center() + vec2(44.0, 0.0), egui::Align2::LEFT_CENTER, label, semibold(14.0), color);
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                self.page = page;
            }
        }

        // Pin the status card to the bottom using its height from the previous frame.
        let height_id = ui.make_persistent_id("engine_card_height");
        let last_h: f32 = ui.data(|d| d.get_temp(height_id)).unwrap_or(78.0);
        ui.add_space((ui.available_height() - last_h).max(0.0));
        let (text, level) = self.shared.status();
        let card = Frame::new()
            .fill(PANEL)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                caption(ui, t("Engine"));
                ui.horizontal_top(|ui| {
                    let color = match level {
                        StatusLevel::Ok => OK,
                        StatusLevel::Busy => WARN,
                        StatusLevel::Error => ERR,
                    };
                    ui.label(RichText::new("●").color(color).size(11.0));
                    ui.add(egui::Label::new(RichText::new(text.to_lowercase()).size(12.5).color(MUTED)).wrap());
                });
            });
        let h = card.response.rect.height();
        if (h - last_h).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(height_id, h));
            ui.ctx().request_repaint();
        }
    }

    fn home(&mut self, ui: &mut Ui, cfg: &mut Config) {
        // Hero card.
        let hero_h = 150.0;
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), hero_h), Sense::hover());
        gradient_rect(ui, rect, 20.0, Color32::from_rgb(0x12, 0x22, 0x4D), Color32::from_rgb(0x0B, 0x2B, 0x45));
        ui.painter().rect_stroke(rect, 20.0, Stroke::new(1.0, Color32::from_rgb(0x23, 0x3E, 0x7A)), egui::StrokeKind::Inside);
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(28.0, 24.0))).layout(Layout::top_down(Align::Min)));
        caption(&mut inner, t("Push to talk"));
        inner.horizontal(|ui| {
            ui.label(RichText::new(t("Hold")).font(serif(32.0)));
            keycap(ui, &combo_name(&cfg.hotkey).to_lowercase(), 16.0);
            ui.label(RichText::new(t("and speak")).font(serif_italic(32.0)));
        });
        muted(&mut inner, t("Release the key — your words are transcribed, translated and typed into whatever chat or text field your cursor is in."));
        ui.add_space(6.0);

        card(ui, |ui| {
            caption(ui, t("Languages"));
            let mut src_opts: Vec<(String, String)> = vec![("auto".into(), format!("✨  {}", t("Auto-detect")))];
            src_opts.extend(LANGUAGES.iter().map(|(n, c)| (c.to_string(), n.to_lowercase())));
            let dst_opts: Vec<(String, String)> = LANGUAGES.iter().map(|(n, c)| (c.to_string(), n.to_lowercase())).collect();
            let col_w = (ui.available_width() - 70.0) / 2.0;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    muted(ui, t("I speak"));
                    combo(ui, "src", col_w, &mut cfg.source_lang, &src_opts);
                });
                ui.vertical(|ui| {
                    ui.add_space(26.0);
                    let swap = ui
                        .add_enabled(cfg.source_lang != "auto", egui::Button::new(RichText::new("⇄").size(16.0)).corner_radius(19).min_size(vec2(40.0, 34.0)))
                        .on_hover_text(t("Swap languages"));
                    if swap.clicked() {
                        std::mem::swap(&mut cfg.source_lang, &mut cfg.target_lang);
                    }
                });
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    muted(ui, t("Type it in"));
                    combo(ui, "dst", col_w, &mut cfg.target_lang, &dst_opts);
                });
            });
            ui.add_space(4.0);
            setting_row(ui, t("Translate"), t("Off = type exactly what you said"), |ui| {
                toggle(ui, &mut cfg.translate);
            });
        });
        ui.add_space(6.0);

        let words: usize = cfg.history.iter().map(|h| h.result.split_whitespace().count()).sum();
        ui.columns(2, |cols| {
            for (col, (title, value)) in cols.iter_mut().zip([(t("Words dictated"), words), (t("Dictations"), cfg.history.len())]) {
                card(col, |ui| {
                    caption(ui, title);
                    ui.label(RichText::new(value.to_string()).font(serif(36.0)));
                });
            }
        });
        ui.add_space(6.0);

        card(ui, |ui| {
            caption(ui, t("Last dictation"));
            match cfg.history.first() {
                None => muted(ui, t("Nothing yet — hold the key and say something.")),
                Some(h) => {
                    if h.original != h.result {
                        muted(ui, &h.original.to_lowercase());
                    }
                    ui.add(egui::Label::new(RichText::new(h.result.to_lowercase()).font(semibold(15.0))).wrap().selectable(true));
                }
            }
        });
    }

    fn history(&mut self, ui: &mut Ui, cfg: &Config) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("History")).font(serif(32.0)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !cfg.history.is_empty() && ghost_button(ui, t("Clear")).clicked() {
                    let mut g = lock(&self.shared.cfg);
                    g.history.clear();
                    g.save();
                }
            });
        });
        ui.add_space(4.0);
        if cfg.history.is_empty() {
            muted(ui, t("Your dictations will appear here."));
        }
        for item in &cfg.history {
            card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.horizontal(|ui| {
                    caption(ui, &format!("{}  ·  {} → {}", item.time, name_for(&item.src).to_lowercase(), name_for(&item.dst).to_lowercase()));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ghost_button(ui, t("Copy")).clicked() {
                            ui.ctx().copy_text(item.result.clone());
                        }
                    });
                });
                if item.original != item.result {
                    muted(ui, &item.original.to_lowercase());
                }
                ui.add(egui::Label::new(RichText::new(item.result.to_lowercase()).font(semibold(15.0))).wrap().selectable(true));
            });
            ui.add_space(2.0);
        }
    }

    fn settings(&mut self, ui: &mut Ui, cfg: &mut Config) {
        ui.label(RichText::new(t("Settings")).font(serif(32.0)));
        ui.add_space(4.0);

        card(ui, |ui| {
            caption(ui, t("Interface"));
            setting_row(ui, t("Interface language"), "", |ui| {
                let opts = vec![("ru".to_string(), "русский".to_string()), ("en".to_string(), "english".to_string())];
                combo(ui, "ui_lang", 200.0, &mut cfg.ui_lang, &opts);
            });
        });
        ui.add_space(6.0);

        card(ui, |ui| {
            caption(ui, t("Shortcut & input"));
            let capture = lock(&self.shared.capture).clone();
            if let Capture::Finished(keys) = &capture {
                if !keys.is_empty() {
                    cfg.hotkey = keys.clone();
                }
                *lock(&self.shared.capture) = Capture::Off;
            }
            let waiting = capture == Capture::Waiting;
            setting_row(ui, t("Push-to-talk key"), t("Hold to record, release to type. Esc cancels."), |ui| {
                if waiting {
                    ui.label(RichText::new(t("Press keys…")).font(semibold(14.0)).color(ACCENT_2));
                } else {
                    if primary_button(ui, t("Change")).clicked() {
                        *lock(&self.shared.capture) = Capture::Waiting;
                    }
                    keycap(ui, &combo_name(&cfg.hotkey).to_lowercase(), 14.0);
                }
            });
            ui.add_space(4.0);
            setting_row(ui, t("Microphone"), "", |ui| {
                if ghost_button(ui, "↻").on_hover_text(t("Refresh list")).clicked() {
                    self.mics = audio::list_microphones();
                }
                let mut opts: Vec<(Option<String>, String)> = vec![(None, t("System default").into())];
                opts.extend(self.mics.iter().map(|m| (Some(m.clone()), m.to_lowercase())));
                combo(ui, "mic", 240.0, &mut cfg.microphone, &opts);
            });
        });
        ui.add_space(6.0);

        card(ui, |ui| {
            caption(ui, t("Recognition"));
            let downloaded = is_downloaded(&cfg.model);
            let info = model_info(&cfg.model);
            let mut sub = if downloaded {
                t("Larger = more accurate but slower.").to_string()
            } else {
                tf("Larger = more accurate but slower. Downloads {} MB on first use.", &[&info.size_mb])
            };
            if info.kind == Kind::GigaAm {
                sub = format!("{} {sub}", t("GigaAM recognises Russian only."));
            }
            setting_row(ui, t("Speech model"), &sub, |ui| {
                let opts: Vec<(String, String)> = MODELS
                    .iter()
                    .map(|m| (m.id.to_string(), if is_downloaded(m.id) { t(m.label).to_string() } else { format!("{}  ·  {} {}", t(m.label), m.size_mb, t("MB")) }))
                    .collect();
                combo(ui, "model", 280.0, &mut cfg.model, &opts);
            });
        });
        ui.add_space(6.0);

        card(ui, |ui| {
            caption(ui, t("Output"));
            setting_row(ui, t("Insert method"), t("Use typing if an app blocks pasting"), |ui| {
                let opts = vec![(InsertMethod::Paste, t("Paste (fast, recommended)").to_string()), (InsertMethod::Type, t("Simulate typing").to_string())];
                combo(ui, "insert", 240.0, &mut cfg.insert_method, &opts);
            });
            setting_row(ui, t("Lowercase"), t("Type everything in lowercase"), |ui| {
                toggle(ui, &mut cfg.lowercase);
            });
            setting_row(ui, t("Send automatically"), t("Press Enter after inserting the text"), |ui| {
                toggle(ui, &mut cfg.auto_enter);
            });
            setting_row(ui, t("Keep my clipboard"), t("Restore the clipboard after pasting"), |ui| {
                toggle(ui, &mut cfg.restore_clipboard);
            });
            setting_row(ui, t("Sounds"), t("Soft click when recording starts and stops"), |ui| {
                toggle(ui, &mut cfg.sounds);
            });
        });
    }
}

pub fn initial_size() -> Vec2 {
    vec2(980.0, 680.0)
}
