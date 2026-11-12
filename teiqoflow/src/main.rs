#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod engine;
mod hotkey;
mod icon;
mod languages;
mod output;
mod overlay;
mod sound;
mod state;
mod stt;
mod text;
mod translate;
mod ui;

use crate::state::Shared;
use std::sync::Arc;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow};

const TITLE: &str = "teiqoflow";

fn show_main_window(shared: &Shared) {
    let title: Vec<u16> = TITLE.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), title.as_ptr());
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_SHOW);
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }
    if let Some(ctx) = shared.egui.get() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.request_repaint();
    }
}

fn build_tray(shared: Arc<Shared>) -> Option<tray_icon::TrayIcon> {
    let menu = Menu::new();
    let open = MenuItem::new("Open teiqoflow", true, None);
    let quit = MenuItem::new("Quit", true, None);
    menu.append(&open).ok()?;
    menu.append(&PredefinedMenuItem::separator()).ok()?;
    menu.append(&quit).ok()?;
    let (open_id, quit_id) = (open.id().clone(), quit.id().clone());

    let s = shared.clone();
    MenuEvent::set_event_handler(Some(move |ev: MenuEvent| {
        if ev.id == open_id {
            show_main_window(&s);
        } else if ev.id == quit_id {
            s.cfg().save();
            std::process::exit(0);
        }
    }));
    let s = shared.clone();
    TrayIconEvent::set_event_handler(Some(move |ev: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
            show_main_window(&s);
        }
    }));

    let icon = tray_icon::Icon::from_rgba(icon::app_icon_rgba(64), 64, 64).ok()?;
    TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip(TITLE).with_icon(icon).build().ok()
}

/// `teiqoflow.exe --transcribe <file.wav> [target-lang]`: runs a 16-bit PCM WAV file through
/// recognition and translation and prints the result. Useful for checking the engine without a mic.
fn transcribe_file(path: &str, target: &str) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let (mut channels, mut rate, mut data) = (1usize, 16_000u32, &bytes[0..0]);
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let len = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().unwrap()) as usize;
        let body = &bytes[i + 8..(i + 8 + len).min(bytes.len())];
        if id == b"fmt " {
            channels = u16::from_le_bytes([body[2], body[3]]) as usize;
            rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
        } else if id == b"data" {
            data = body;
        }
        i += 8 + len + (len & 1);
    }
    let mono: Vec<f32> = data
        .chunks_exact(2 * channels)
        .map(|f| f.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0).sum::<f32>() / channels as f32)
        .collect();
    let audio = audio::resample(&mono, rate, audio::TARGET_SR);

    let shared = Shared::new(config::Config::load());
    let model = shared.cfg().model;
    let engine = stt::Engine::load(&stt::ensure_model(&model, &shared)?)?;
    let started = std::time::Instant::now();
    let (text, lang) = engine.transcribe(&audio, "auto")?;
    let stt_ms = started.elapsed().as_millis();
    let translated = translate::translate(&text, target)?;
    println!("model: {model}\ndetected: {lang}\ntranscribed in {stt_ms} ms\ntext: {text}\n{target}: {translated}");
    Ok(())
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--transcribe") {
        let target = args.get(3).map(String::as_str).unwrap_or("en");
        if let Err(e) = transcribe_file(args.get(2).map(String::as_str).unwrap_or(""), target) {
            println!("error: {e}");
        }
        return Ok(());
    }

    let cfg = config::Config::load();
    let model = cfg.model.clone();
    let shared = Arc::new(Shared::new(cfg));

    let (key_tx, key_rx) = crossbeam_channel::unbounded();
    hotkey::spawn(shared.clone(), key_tx);
    let engine_tx = engine::spawn(shared.clone(), key_rx);
    let _ = engine_tx.send(engine::EngineMsg::LoadModel(model));

    let icon = egui::IconData { rgba: icon::app_icon_rgba(64), width: 64, height: 64 };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_app_id(TITLE)
            .with_inner_size(ui::initial_size())
            .with_min_inner_size(egui::vec2(820.0, 560.0))
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_icon(Arc::new(icon)),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        TITLE,
        options,
        Box::new(move |cc| {
            let _ = shared.egui.set(cc.egui_ctx.clone());
            // Started here so the process is already DPI-aware when the overlay measures the screen.
            overlay::spawn(shared.clone());
            let tray = build_tray(shared.clone());
            Ok(Box::new(ui::App::new(cc, shared, engine_tx, tray)))
        }),
    )
}
