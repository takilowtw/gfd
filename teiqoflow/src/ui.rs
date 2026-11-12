use crate::audio;
use crate::config::{Config, InsertMethod};
use crate::engine::EngineMsg;
use crate::hotkey::combo_name;
use crate::languages::{LANGUAGES, name_for};
use crate::state::{Capture, Shared, StatusLevel, lock};
use crate::stt::{MODELS, model_info, model_path};
use crossbeam_channel::Sender;
use egui::{
    Align, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Layout, Margin, Mesh, Pos2, Rect,
    RichText, Sense, Stroke, Ui, Vec2, ViewportCommand, pos2, vec2,
};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0x0B, 0x0B, 0x10);
pub const PANEL: Color32 = Color32::from_rgb(0x14, 0x14, 0x1C);
pub const PANEL_2: Color32 = Color32::from_rgb(0x1B, 0x1B, 0x26);
pub const BORDER: Color32 = Color32::from_rgb(0x26, 0x26, 0x33);
pub const TEXT: Color32 = Color32::from_rgb(0xED, 0xED, 0xF3);
pub const MUTED: Color32 = Color32::from_rgb(0x8B, 0x8B, 0x9E);
pub const ACCENT_1: Color32 = Color32::from_rgb(0x7C, 0x5C, 0xFF);
pub const ACCENT_2: Color32 = Color32::from_rgb(0xFF, 0x5C, 0xAA);
pub const OK: Color32 = Color32::from_rgb(0x3D, 0xDC, 0x97);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xB5, 0x47);
pub const ERR: Color32 = Color32::from_rgb(0xFF, 0x5C, 0x70);

fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

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

fn setup_style(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()) + "\\Fonts\\";
    let mut fallbacks = Vec::new();
    for (name, file) in [
        ("segoe", "segoeui.ttf"),
        ("segoe-sb", "seguisb.ttf"),
        ("segoe-sym", "seguisym.ttf"),
        ("yahei", "msyh.ttc"),
        ("yugothic", "YuGothM.ttc"),
        ("malgun", "malgun.ttf"),
        ("nirmala", "Nirmala.ttc"),
        ("nirmala2", "Nirmala.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(format!("{dir}{file}")) {
            fonts.font_data.insert(name.into(), Arc::new(FontData::from_owned(bytes)));
            if !matches!(name, "segoe" | "segoe-sb") {
                fallbacks.push(name.to_string());
            }
        }
    }
    let has = |n: &str| fonts.font_data.contains_key(n);
    let regular = if has("segoe") { "segoe" } else { "" };
    let bold = if has("segoe-sb") { "segoe-sb" } else { regular };

    let default_prop = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut prop: Vec<String> = Vec::new();
    if !regular.is_empty() {
        prop.push(regular.into());
    }
    prop.extend(fallbacks.iter().cloned());
    prop.extend(default_prop.iter().cloned());
    let mut sb: Vec<String> = Vec::new();
    if !bold.is_empty() {
        sb.push(bold.into());
    }
    sb.extend(prop.iter().cloned());
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(FontFamily::Name("semibold".into()), sb);
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = vec2(10.0, 10.0);
    style.spacing.button_padding = vec2(14.0, 8.0);
    style.spacing.interact_size.y = 34.0;
    style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style.text_styles.insert(egui::TextStyle::Button, semibold(14.0));
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = PANEL_2;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(10);
    v.extreme_bg_color = PANEL_2;
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT_1;
    v.selection.stroke = Stroke::new(1.0, TEXT);
    v.hyperlink_color = ACCENT_1;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(10);
        w.bg_fill = PANEL_2;
        w.weak_bg_fill = PANEL_2;
        w.bg_stroke = Stroke::new(1.0, BORDER);
        w.fg_stroke = Stroke::new(1.2, TEXT);
    }
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_1);
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_2);
    v.widgets.open.bg_stroke = Stroke::new(1.0, ACCENT_1);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    ctx.set_global_style(style);
}

// ------------------------------------------------------------------ drawing helpers

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// Rounded rectangle filled with a diagonal two-colour gradient.
fn gradient_rect(ui: &Ui, rect: Rect, radius: f32, c0: Color32, c1: Color32) {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let mut pts: Vec<Pos2> = Vec::new();
    let corners = [
        (pos2(rect.right() - r, rect.top() + r), -90.0f32),
        (pos2(rect.right() - r, rect.bottom() - r), 0.0),
        (pos2(rect.left() + r, rect.bottom() - r), 90.0),
        (pos2(rect.left() + r, rect.top() + r), 180.0),
    ];
    for (c, start) in corners {
        for i in 0..=8 {
            let a = (start + i as f32 * 90.0 / 8.0).to_radians();
            pts.push(c + vec2(a.cos(), a.sin()) * r);
        }
    }
    let diag = rect.width() + rect.height();
    let color_at = |p: Pos2| lerp_color(c0, c1, ((p.x - rect.left()) + (p.y - rect.top())) / diag);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.center(), color_at(rect.center()));
    for p in &pts {
        mesh.colored_vertex(*p, color_at(*p));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
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
    ui.label(RichText::new(text.to_uppercase()).font(semibold(11.0)).color(MUTED).extra_letter_spacing(1.0));
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

fn primary_button(ui: &mut Ui, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), semibold(14.0), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(36.0, 18.0), Sense::click());
    let k = if resp.hovered() { 1.0 } else { 0.0 };
    let c0 = lerp_color(ACCENT_1, Color32::from_rgb(0x8D, 0x71, 0xFF), k);
    let c1 = lerp_color(ACCENT_2, Color32::from_rgb(0xFF, 0x74, 0xB8), k);
    gradient_rect(ui, rect, 10.0, c0, c1);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn ghost_button(ui: &mut Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).font(semibold(13.0))).fill(PANEL_2).corner_radius(10))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn toggle(ui: &mut Ui, on: &mut bool) -> egui::Response {
    let size = vec2(46.0, 26.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let p = ui.painter();
    p.rect_filled(rect, 13.0, PANEL_2);
    p.rect_stroke(rect, 13.0, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
    if t > 0.01 {
        let a = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (t * 255.0) as u8);
        gradient_rect(ui, rect, 13.0, a(ACCENT_1), a(ACCENT_2));
    }
    let x = rect.left() + 13.0 + t * (rect.width() - 26.0);
    ui.painter().circle_filled(pos2(x, rect.center().y), 10.0, Color32::WHITE);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
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

        // Rounded window background.
        let full = ctx.content_rect();
        ui.painter().rect(full.shrink(0.5), 16.0, BG, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);

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
        for (page, icon, label) in [(Page::Home, "⌂", "Home"), (Page::History, "☰", "History"), (Page::Settings, "⚙", "Settings")] {
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
                caption(ui, "Engine");
                ui.horizontal_top(|ui| {
                    let color = match level {
                        StatusLevel::Ok => OK,
                        StatusLevel::Busy => WARN,
                        StatusLevel::Error => ERR,
                    };
                    ui.label(RichText::new("●").color(color).size(11.0));
                    ui.add(egui::Label::new(RichText::new(text).size(12.5).color(MUTED)).wrap());
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
        gradient_rect(ui, rect, 20.0, Color32::from_rgb(0x24, 0x1A, 0x4D), Color32::from_rgb(0x3A, 0x16, 0x31));
        ui.painter().rect_stroke(rect, 20.0, Stroke::new(1.0, Color32::from_rgb(0x3A, 0x2F, 0x6B)), egui::StrokeKind::Inside);
        let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(28.0, 24.0))).layout(Layout::top_down(Align::Min)));
        caption(&mut inner, "Push to talk");
        inner.horizontal(|ui| {
            ui.label(RichText::new("Hold").font(semibold(26.0)));
            keycap(ui, &combo_name(&cfg.hotkey), 16.0);
            ui.label(RichText::new("and speak").font(semibold(26.0)));
        });
        muted(&mut inner, "Release the key — your words are transcribed, translated and typed into whatever chat or text field your cursor is in.");
        ui.add_space(6.0);

        card(ui, |ui| {
            caption(ui, "Languages");
            let mut src_opts: Vec<(String, String)> = vec![("auto".into(), "✨  Auto-detect".into())];
            src_opts.extend(LANGUAGES.iter().map(|(n, c)| (c.to_string(), n.to_string())));
            let dst_opts: Vec<(String, String)> = LANGUAGES.iter().map(|(n, c)| (c.to_string(), n.to_string())).collect();
            let col_w = (ui.available_width() - 70.0) / 2.0;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    muted(ui, "I speak");
                    combo(ui, "src", col_w, &mut cfg.source_lang, &src_opts);
                });
                ui.vertical(|ui| {
                    ui.add_space(26.0);
                    let swap = ui
                        .add_enabled(cfg.source_lang != "auto", egui::Button::new(RichText::new("⇄").size(16.0)).corner_radius(19).min_size(vec2(40.0, 34.0)))
                        .on_hover_text("Swap languages");
                    if swap.clicked() {
                        std::mem::swap(&mut cfg.source_lang, &mut cfg.target_lang);
                    }
                });
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    muted(ui, "Type it in");
                    combo(ui, "dst", col_w, &mut cfg.target_lang, &dst_opts);
                });
            });
            ui.add_space(4.0);
            setting_row(ui, "Translate", "Off = type exactly what you said", |ui| {
                toggle(ui, &mut cfg.translate);
            });
        });
        ui.add_space(6.0);

        let words: usize = cfg.history.iter().map(|h| h.result.split_whitespace().count()).sum();
        ui.columns(2, |cols| {
            for (col, (title, value)) in cols.iter_mut().zip([("Words dictated", words), ("Dictations", cfg.history.len())]) {
                card(col, |ui| {
                    caption(ui, title);
                    ui.label(RichText::new(value.to_string()).font(semibold(30.0)));
                });
            }
        });
        ui.add_space(6.0);

        card(ui, |ui| {
            caption(ui, "Last dictation");
            match cfg.history.first() {
                None => muted(ui, "Nothing yet — hold the key and say something."),
                Some(h) => {
                    if h.original != h.result {
                        muted(ui, &h.original);
                    }
                    ui.add(egui::Label::new(RichText::new(&h.result).font(semibold(15.0))).wrap().selectable(true));
                }
            }
        });
    }

    fn history(&mut self, ui: &mut Ui, cfg: &Config) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("History").font(semibold(26.0)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !cfg.history.is_empty() && ghost_button(ui, "Clear").clicked() {
                    let mut g = lock(&self.shared.cfg);
                    g.history.clear();
                    g.save();
                }
            });
        });
        ui.add_space(4.0);
        if cfg.history.is_empty() {
            muted(ui, "Your dictations will appear here.");
        }
        for item in &cfg.history {
            card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.horizontal(|ui| {
                    caption(ui, &format!("{}  ·  {} → {}", item.time, name_for(&item.src), name_for(&item.dst)));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ghost_button(ui, "Copy").clicked() {
                            ui.ctx().copy_text(item.result.clone());
                        }
                    });
                });
                if item.original != item.result {
                    muted(ui, &item.original);
                }
                ui.add(egui::Label::new(RichText::new(&item.result).font(semibold(15.0))).wrap().selectable(true));
            });
            ui.add_space(2.0);
        }
    }

    fn settings(&mut self, ui: &mut Ui, cfg: &mut Config) {
        ui.label(RichText::new("Settings").font(semibold(26.0)));
        ui.add_space(4.0);

        card(ui, |ui| {
            caption(ui, "Shortcut & input");
            let capture = lock(&self.shared.capture).clone();
            if let Capture::Finished(keys) = &capture {
                if !keys.is_empty() {
                    cfg.hotkey = keys.clone();
                }
                *lock(&self.shared.capture) = Capture::Off;
            }
            let waiting = capture == Capture::Waiting;
            setting_row(ui, "Push-to-talk key", "Hold to record, release to type. Esc cancels.", |ui| {
                if waiting {
                    ui.label(RichText::new("Press keys…").font(semibold(14.0)).color(ACCENT_2));
                } else {
                    if primary_button(ui, "Change").clicked() {
                        *lock(&self.shared.capture) = Capture::Waiting;
                    }
                    keycap(ui, &combo_name(&cfg.hotkey), 14.0);
                }
            });
            ui.add_space(4.0);
            setting_row(ui, "Microphone", "", |ui| {
                if ghost_button(ui, "↻").on_hover_text("Refresh list").clicked() {
                    self.mics = audio::list_microphones();
                }
                let mut opts: Vec<(Option<String>, String)> = vec![(None, "System default".into())];
                opts.extend(self.mics.iter().map(|m| (Some(m.clone()), m.clone())));
                combo(ui, "mic", 240.0, &mut cfg.microphone, &opts);
            });
        });
        ui.add_space(6.0);

        card(ui, |ui| {
            caption(ui, "Recognition");
            let downloaded = model_path(&cfg.model).exists();
            let info = model_info(&cfg.model);
            let sub = if downloaded {
                "Larger = more accurate but slower.".to_string()
            } else {
                format!("Larger = more accurate but slower. Downloads {} MB on first use.", info.size_mb)
            };
            setting_row(ui, "Speech model", &sub, |ui| {
                let opts: Vec<(String, String)> = MODELS
                    .iter()
                    .map(|m| (m.id.to_string(), if model_path(m.id).exists() { m.label.to_string() } else { format!("{}  ·  {} MB", m.label, m.size_mb) }))
                    .collect();
                combo(ui, "model", 280.0, &mut cfg.model, &opts);
            });
        });
        ui.add_space(6.0);

        card(ui, |ui| {
            caption(ui, "Output");
            setting_row(ui, "Insert method", "Use typing if an app blocks pasting", |ui| {
                let opts = vec![(InsertMethod::Paste, "Paste (fast, recommended)".to_string()), (InsertMethod::Type, "Simulate typing".to_string())];
                combo(ui, "insert", 240.0, &mut cfg.insert_method, &opts);
            });
            setting_row(ui, "Send automatically", "Press Enter after inserting the text", |ui| {
                toggle(ui, &mut cfg.auto_enter);
            });
            setting_row(ui, "Keep my clipboard", "Restore the clipboard after pasting", |ui| {
                toggle(ui, &mut cfg.restore_clipboard);
            });
            setting_row(ui, "Sounds", "Soft click when recording starts and stops", |ui| {
                toggle(ui, &mut cfg.sounds);
            });
        });
    }
}

pub fn initial_size() -> Vec2 {
    vec2(980.0, 680.0)
}
