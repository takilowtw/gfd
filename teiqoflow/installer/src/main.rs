#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[path = "../../src/icon.rs"]
#[allow(dead_code)]
mod icon;
#[path = "../../src/style.rs"]
#[allow(dead_code)]
mod style;
mod sys;

mod payload {
    include!(concat!(env!("OUT_DIR"), "/payload.rs"));
}

use egui::{Align, Color32, FontId, Layout, Rect, RichText, Sense, Stroke, Ui, ViewportCommand, pos2, vec2};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use style::*;

const TITLE: &str = "teiqoflow setup";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq)]
enum Lang {
    Ru,
    En,
}

/// All installer text, already lowercase to match the app.
fn tr(lang: Lang, key: &'static str) -> &'static str {
    if lang == Lang::En {
        return key;
    }
    match key {
        "setup" => "установка",
        "speak. it types." => "говори.",
        "it types for you." => "текст появится сам.",
        "intro" => "диктовка с мгновенным переводом в любое окно: зажми ctrl + win, скажи и отпусти.",
        "install to" => "папка установки",
        "change" => "изменить",
        "desktop shortcut" => "ярлык на рабочем столе",
        "start with windows" => "запускать вместе с windows",
        "launch when done" => "запустить после установки",
        "install" => "установить",
        "installing…" => "устанавливаю…",
        "closing teiqoflow" => "закрываю teiqoflow",
        "copying files" => "копирую файлы",
        "creating shortcuts" => "создаю ярлыки",
        "registering" => "регистрирую в системе",
        "done." => "готово.",
        "done text" => "teiqoflow установлен. зажми ctrl + win в любом окне и говори.",
        "finish" => "готово",
        "failed." => "не получилось.",
        "close" => "закрыть",
        "remove title" => "удалить teiqoflow?",
        "remove text" => "приложение, ярлыки и автозапуск будут удалены.",
        "wipe data" => "удалить настройки, историю и модели",
        "cancel" => "отмена",
        "remove" => "удалить",
        "removing…" => "удаляю…",
        "removed." => "удалено.",
        "removed text" => "teiqoflow удалён с компьютера.",
        "mb" => "мб",
        "empty payload" => "установщик собран без файлов приложения — запусти build.ps1.",
        _ => key,
    }
}

#[derive(Clone, PartialEq)]
enum Step {
    Welcome,
    Working,
    Done,
    Failed(String),
    Confirm,
    Removing,
    Removed,
}

#[derive(Default)]
struct Progress {
    fraction: f32,
    label: &'static str,
    result: Option<Result<(), String>>,
}

struct Setup {
    lang: Lang,
    step: Step,
    dir: PathBuf,
    desktop: bool,
    autostart: bool,
    launch: bool,
    wipe_data: bool,
    progress: Arc<Mutex<Progress>>,
    logo: egui::TextureHandle,
    payload_mb: u64,
}

impl Setup {
    fn new(cc: &eframe::CreationContext<'_>, uninstall: bool) -> Self {
        setup_style(&cc.egui_ctx);
        disable_system_frame(TITLE);
        let rgba = icon::app_icon_rgba(160);
        let logo = cc.egui_ctx.load_texture("logo", egui::ColorImage::from_rgba_unmultiplied([160, 160], &rgba), egui::TextureOptions::LINEAR);
        let dir = if uninstall {
            std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)).unwrap_or_else(sys::default_dir)
        } else {
            sys::default_dir()
        };
        Self {
            lang: if sys::system_is_russian() { Lang::Ru } else { Lang::En },
            step: if uninstall { Step::Confirm } else { Step::Welcome },
            dir,
            desktop: true,
            autostart: true,
            launch: true,
            wipe_data: false,
            progress: Arc::default(),
            logo,
            payload_mb: payload::FILES.iter().map(|f| f.1).sum::<u64>().div_ceil(1_048_576),
        }
    }

    fn t(&self, key: &'static str) -> &'static str {
        tr(self.lang, key)
    }

    fn start_install(&mut self, ctx: &egui::Context) {
        self.step = Step::Working;
        let (dir, desktop, autostart, progress, ctx) = (self.dir.clone(), self.desktop, self.autostart, self.progress.clone(), ctx.clone());
        let lang = self.lang;
        std::thread::spawn(move || {
            let set = |f: f32, label: &'static str| {
                let mut p = progress.lock().unwrap();
                p.fraction = f;
                p.label = tr(lang, label);
                ctx.request_repaint();
            };
            let result = (|| -> Result<(), String> {
                if payload::FILES.is_empty() {
                    return Err(tr(lang, "empty payload").into());
                }
                set(0.02, "closing teiqoflow");
                sys::close_running_app();
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;

                let total: u64 = payload::FILES.iter().map(|f| f.1).sum();
                let mut done = 0u64;
                for (name, _, data) in payload::FILES {
                    sys::unpack(data, &dir.join(name), |n| {
                        done += n;
                        set(0.05 + 0.80 * done as f32 / total as f32, "copying files");
                    })?;
                }
                if let Ok(me) = std::env::current_exe() {
                    let target = dir.join(sys::UNINSTALLER);
                    if me != target {
                        std::fs::copy(&me, &target).map_err(|e| format!("{}: {e}", target.display()))?;
                    }
                }
                set(0.90, "creating shortcuts");
                sys::write_shortcuts(&dir, desktop)?;
                set(0.96, "registering");
                sys::register_uninstaller(&dir, VERSION, (total / 1024) as u32)?;
                sys::set_autostart(&dir, autostart)?;
                set(1.0, "registering");
                Ok(())
            })();
            progress.lock().unwrap().result = Some(result);
            ctx.request_repaint();
        });
    }

    fn start_uninstall(&mut self, ctx: &egui::Context) {
        self.step = Step::Removing;
        let (dir, wipe, progress, ctx) = (self.dir.clone(), self.wipe_data, self.progress.clone(), ctx.clone());
        std::thread::spawn(move || {
            sys::close_running_app();
            progress.lock().unwrap().fraction = 0.3;
            sys::remove_shortcuts();
            sys::unregister();
            progress.lock().unwrap().fraction = 0.6;
            sys::remove_files(&dir);
            if wipe {
                let _ = std::fs::remove_dir_all(sys::data_dir());
            }
            // Let the bar visibly reach the end before switching pages.
            progress.lock().unwrap().fraction = 1.0;
            ctx.request_repaint();
            std::thread::sleep(Duration::from_millis(500));
            progress.lock().unwrap().result = Some(Ok(()));
            ctx.request_repaint();
        });
    }

    fn finish_uninstall(&self, ctx: &egui::Context) {
        if std::env::current_exe().map(|p| p.starts_with(&self.dir)).unwrap_or(false) {
            sys::schedule_folder_removal(&self.dir);
        }
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
}

// ------------------------------------------------------------------ widgets

fn headline(ui: &mut Ui, upright: &str, italic: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.label(RichText::new(upright).font(serif(40.0)));
        if !italic.is_empty() {
            ui.label(RichText::new(italic).font(serif_italic(40.0)).color(ACCENT_2));
        }
    });
}

fn muted(ui: &mut Ui, text: &str, size: f32) {
    ui.add(egui::Label::new(RichText::new(text).size(size).color(MUTED)).wrap());
}

fn option_row(ui: &mut Ui, label: &str, value: &mut bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(14.5));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            toggle(ui, value);
        });
    });
}

fn progress_bar(ui: &mut Ui, fraction: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
    ui.painter().rect_filled(rect, 5.0, PANEL_2);
    let shown = ui.ctx().animate_value_with_time(ui.make_persistent_id("progress"), fraction.clamp(0.0, 1.0), 0.25);
    if shown > 0.0 {
        let fill = Rect::from_min_size(rect.min, vec2((rect.width() * shown).max(10.0), rect.height()));
        gradient_rect(ui, fill, 5.0, ACCENT_1, ACCENT_2);
        // A soft highlight sweeping across the filled part.
        let t = (ui.input(|i| i.time) as f32 * 0.8).fract();
        let x = fill.left() + fill.width() * t;
        let glow = Rect::from_center_size(pos2(x, fill.center().y), vec2(60.0f32.min(fill.width()), rect.height()));
        let glow = glow.intersect(fill);
        if glow.width() > 1.0 {
            ui.painter().rect_filled(glow, 5.0, Color32::from_white_alpha(28));
        }
        ui.ctx().request_repaint();
    }
}

fn status_badge(ui: &mut Ui, ok: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(56.0, 56.0), Sense::hover());
    let c = rect.center();
    let color = if ok { OK } else { ERR };
    ui.painter().circle_filled(c, 28.0, color.gamma_multiply(0.15));
    ui.painter().circle_filled(c, 20.0, color);
    let stroke = Stroke::new(3.0, BG);
    if ok {
        ui.painter().line_segment([c + vec2(-8.0, 0.5), c + vec2(-2.5, 6.0)], stroke);
        ui.painter().line_segment([c + vec2(-2.5, 6.0), c + vec2(8.5, -6.0)], stroke);
    } else {
        ui.painter().line_segment([c + vec2(-6.5, -6.5), c + vec2(6.5, 6.5)], stroke);
        ui.painter().line_segment([c + vec2(6.5, -6.5), c + vec2(-6.5, 6.5)], stroke);
    }
}

impl eframe::App for Setup {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        window_background(ui);
        let full = ctx.content_rect();

        // Soft blue glow in the top-left corner, behind the content.
        for i in 0..14 {
            let r = 40.0 + i as f32 * 14.0;
            ui.painter().circle_filled(full.left_top() + vec2(70.0, 70.0), r, Color32::from_rgba_unmultiplied(0x25, 0x63, 0xEB, 3));
        }

        // Drag anywhere on the background to move the window.
        let drag = ui.interact(full, ui.make_persistent_id("drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        // Pick up results from the worker thread.
        if let Some(result) = self.progress.lock().unwrap().result.take() {
            self.step = match (&self.step, result) {
                (Step::Removing, _) => Step::Removed,
                (_, Ok(())) => Step::Done,
                (_, Err(e)) => Step::Failed(e),
            };
        }

        let busy = matches!(self.step, Step::Working | Step::Removing);
        let close = Rect::from_center_size(pos2(full.right() - 26.0, full.top() + 24.0), vec2(32.0, 26.0));
        let close_resp = ui.interact(close, ui.make_persistent_id("close"), Sense::click());
        if !busy {
            if close_resp.hovered() {
                ui.painter().rect_filled(close, 8.0, ERR);
            }
            let color = if close_resp.hovered() { TEXT } else { MUTED };
            ui.painter().text(close.center(), egui::Align2::CENTER_CENTER, "✕", FontId::proportional(13.0), color);
            if close_resp.clicked() {
                if self.step == Step::Removed {
                    self.finish_uninstall(&ctx);
                } else {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }

        let content = full.shrink2(vec2(40.0, 34.0));
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(content).layout(Layout::top_down(Align::Min)));
        let ui = &mut ui;

        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(vec2(40.0, 40.0)));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(RichText::new("teiqoflow").font(semibold(18.0)));
                let sub = if matches!(self.step, Step::Confirm | Step::Removing | Step::Removed) { "" } else { self.t("setup") };
                ui.label(RichText::new(format!("{sub} · v{VERSION}").trim_start_matches(" · ").to_string()).size(12.5).color(MUTED));
            });
        });
        ui.add_space(26.0);

        match self.step.clone() {
            Step::Welcome => self.welcome(ui),
            Step::Working | Step::Removing => {
                let label = if self.step == Step::Working { self.t("installing…") } else { self.t("removing…") };
                headline(ui, label, "");
                ui.add_space(26.0);
                let p = self.progress.lock().unwrap();
                let (fraction, step_label) = (p.fraction, p.label);
                drop(p);
                progress_bar(ui, fraction);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    muted(ui, step_label, 13.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| muted(ui, &format!("{}%", (fraction * 100.0) as u32), 13.0));
                });
            }
            Step::Done => {
                status_badge(ui, true);
                ui.add_space(14.0);
                headline(ui, self.t("done."), "");
                ui.add_space(4.0);
                muted(ui, self.t("done text"), 14.5);
                ui.add_space(22.0);
                if self.launch {
                    option_row(ui, self.t("launch when done"), &mut self.launch);
                }
                self.bottom(ui, |s, ui| {
                    if primary_button(ui, s.t("finish")).clicked() {
                        if s.launch {
                            sys::launch(&s.dir);
                        }
                        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            }
            Step::Failed(error) => {
                status_badge(ui, false);
                ui.add_space(14.0);
                headline(ui, self.t("failed."), "");
                ui.add_space(4.0);
                muted(ui, &error.to_lowercase(), 13.5);
                self.bottom(ui, |s, ui| {
                    if ghost_button(ui, s.t("close")).clicked() {
                        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            }
            Step::Confirm => {
                headline(ui, self.t("remove title"), "");
                ui.add_space(4.0);
                muted(ui, self.t("remove text"), 14.5);
                ui.add_space(22.0);
                option_row(ui, self.t("wipe data"), &mut self.wipe_data);
                self.bottom(ui, |s, ui| {
                    if primary_button(ui, s.t("remove")).clicked() {
                        s.start_uninstall(ui.ctx());
                    }
                    if ghost_button(ui, s.t("cancel")).clicked() {
                        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                    }
                });
            }
            Step::Removed => {
                status_badge(ui, true);
                ui.add_space(14.0);
                headline(ui, self.t("removed."), "");
                ui.add_space(4.0);
                muted(ui, self.t("removed text"), 14.5);
                self.bottom(ui, |s, ui| {
                    if primary_button(ui, s.t("close")).clicked() {
                        s.finish_uninstall(ui.ctx());
                    }
                });
            }
        }
    }
}

impl Setup {
    fn welcome(&mut self, ui: &mut Ui) {
        headline(ui, self.t("speak. it types."), self.t("it types for you."));
        ui.add_space(2.0);
        muted(ui, self.t("intro"), 14.5);
        ui.add_space(22.0);

        ui.label(RichText::new(self.t("install to")).size(12.5).color(MUTED));
        ui.horizontal(|ui| {
            let w = ui.available_width() - 110.0;
            let (rect, _) = ui.allocate_exact_size(vec2(w, 36.0), Sense::hover());
            ui.painter().rect(rect, 10.0, PANEL_2, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
            let path = self.dir.display().to_string();
            let galley = ui.painter().layout(path, FontId::proportional(13.5), TEXT, w - 24.0);
            let row = galley.rows.first().map(|r| r.rect().height()).unwrap_or(16.0);
            ui.painter().with_clip_rect(rect.shrink(4.0)).galley(pos2(rect.left() + 12.0, rect.center().y - row / 2.0), galley, TEXT);
            if ghost_button(ui, self.t("change")).clicked() {
                if let Some(picked) = rfd::FileDialog::new().set_directory(&self.dir).pick_folder() {
                    let ends_with_app = picked.file_name().map(|n| n.eq_ignore_ascii_case(sys::APP)).unwrap_or(false);
                    self.dir = if ends_with_app { picked } else { picked.join(sys::APP) };
                }
            }
        });
        ui.add_space(14.0);
        option_row(ui, self.t("desktop shortcut"), &mut self.desktop);
        option_row(ui, self.t("start with windows"), &mut self.autostart);
        option_row(ui, self.t("launch when done"), &mut self.launch);

        let size = format!("≈ {} {}", self.payload_mb, self.t("mb"));
        self.bottom(ui, |s, ui| {
            if primary_button(ui, s.t("install")).clicked() {
                s.start_install(ui.ctx());
            }
            ui.add_space(8.0);
            muted(ui, &size, 13.0);
        });
    }

    /// Lays out buttons right-aligned along the bottom edge.
    fn bottom(&mut self, ui: &mut Ui, add: impl FnOnce(&mut Self, &mut Ui)) {
        let rect = ui.max_rect();
        let bar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 40.0), rect.max);
        let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(Layout::right_to_left(Align::Center)));
        add(self, &mut bar_ui);
    }
}

fn main() -> eframe::Result {
    let uninstall = std::env::args().any(|a| a == "--uninstall");
    let icon = egui::IconData { rgba: icon::app_icon_rgba(64), width: 64, height: 64 };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([640.0, 470.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_icon(Arc::new(icon)),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(TITLE, options, Box::new(move |cc| Ok(Box::new(Setup::new(cc, uninstall)))))
}
