use crate::config::Config;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

#[derive(Clone, PartialEq)]
pub enum Phase {
    Idle,
    Listening,
    Processing(String),
    Done(String),
    Error(String),
}

#[derive(Clone, Copy, PartialEq)]
pub enum StatusLevel {
    Ok,
    Busy,
    Error,
}

#[derive(Clone, PartialEq)]
pub enum Capture {
    Off,
    Waiting,
    Finished(Vec<u32>),
}

/// State shared between the UI, overlay, hotkey and engine threads.
pub struct Shared {
    pub cfg: Mutex<Config>,
    phase: Mutex<(Phase, Instant)>,
    level: AtomicU32,
    status: Mutex<(String, StatusLevel)>,
    pub capture: Mutex<Capture>,
    pub suppress_hotkey: AtomicBool,
    pub model_ready: AtomicBool,
    pub egui: OnceLock<egui::Context>,
}

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Shared {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg: Mutex::new(cfg),
            phase: Mutex::new((Phase::Idle, Instant::now())),
            level: AtomicU32::new(0),
            status: Mutex::new(("Starting…".into(), StatusLevel::Busy)),
            capture: Mutex::new(Capture::Off),
            suppress_hotkey: AtomicBool::new(false),
            model_ready: AtomicBool::new(false),
            egui: OnceLock::new(),
        }
    }

    pub fn repaint(&self) {
        if let Some(ctx) = self.egui.get() {
            ctx.request_repaint();
        }
    }

    pub fn cfg(&self) -> Config {
        lock(&self.cfg).clone()
    }

    pub fn set_phase(&self, phase: Phase) {
        *lock(&self.phase) = (phase, Instant::now());
    }

    pub fn phase(&self) -> (Phase, Instant) {
        lock(&self.phase).clone()
    }

    pub fn set_level(&self, v: f32) {
        self.level.store(v.to_bits(), Ordering::Relaxed);
    }

    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }

    pub fn set_status(&self, text: impl Into<String>, level: StatusLevel) {
        *lock(&self.status) = (text.into(), level);
        self.repaint();
    }

    pub fn status(&self) -> (String, StatusLevel) {
        lock(&self.status).clone()
    }
}
