//! Glues the pipeline together: hotkey -> microphone -> Whisper -> translation -> typing.

use crate::audio::{self, Recording};
use crate::config::HistoryItem;
use crate::hotkey::KeyEvent;
use crate::state::{Phase, Shared, StatusLevel, lock};
use crate::stt::{self, Engine};
use crate::{languages, output, sound, translate};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

const MIN_SAMPLES: usize = audio::TARGET_SR as usize * 35 / 100;
const SILENCE_RMS: f32 = 0.002;

pub enum EngineMsg {
    LoadModel(String),
    Transcribe(Vec<f32>),
}

/// Starts the recording and engine threads. Returns the channel used to (re)load models.
pub fn spawn(shared: Arc<Shared>, keys: Receiver<KeyEvent>) -> Sender<EngineMsg> {
    let (tx, rx) = unbounded::<EngineMsg>();

    let s = shared.clone();
    std::thread::Builder::new().name("engine".into()).spawn(move || engine_loop(s, rx)).unwrap();

    let s = shared.clone();
    let engine_tx = tx.clone();
    // cpal streams aren't Send, so they are created and dropped on this one thread.
    std::thread::Builder::new()
        .name("recorder".into())
        .spawn(move || {
            let mut rec: Option<Recording> = None;
            for ev in keys {
                match ev {
                    KeyEvent::Press => {
                        if rec.is_some() {
                            continue;
                        }
                        let cfg = s.cfg();
                        match Recording::start(cfg.microphone.as_deref(), s.clone()) {
                            Ok(r) => {
                                rec = Some(r);
                                if cfg.sounds {
                                    sound::play_start();
                                }
                                s.set_level(0.0);
                                s.set_phase(Phase::Listening);
                            }
                            Err(e) => s.set_phase(Phase::Error(format!("Microphone: {e}"))),
                        }
                    }
                    KeyEvent::Cancel => {
                        if rec.take().is_some() {
                            s.set_phase(Phase::Idle);
                        }
                    }
                    KeyEvent::Release => {
                        let Some(r) = rec.take() else { continue };
                        let samples = r.finish();
                        if s.cfg().sounds {
                            sound::play_stop();
                        }
                        if samples.len() < MIN_SAMPLES {
                            s.set_phase(Phase::Idle);
                        } else if audio::rms(&samples) < SILENCE_RMS {
                            s.set_phase(Phase::Error("Didn't hear anything — check your microphone".into()));
                        } else if !s.model_ready.load(Ordering::Relaxed) {
                            s.set_phase(Phase::Error("Speech model is still loading…".into()));
                        } else {
                            s.set_phase(Phase::Processing("Transcribing".into()));
                            let _ = engine_tx.send(EngineMsg::Transcribe(samples));
                        }
                    }
                }
            }
        })
        .unwrap();

    tx
}

fn engine_loop(shared: Arc<Shared>, rx: Receiver<EngineMsg>) {
    let mut engine: Option<Engine> = None;
    while let Ok(msg) = rx.recv() {
        match msg {
            EngineMsg::LoadModel(id) => {
                shared.model_ready.store(false, Ordering::Relaxed);
                engine = None;
                shared.set_status(format!("Loading {id} model…"), StatusLevel::Busy);
                let result = stt::ensure_model(&id, &shared).and_then(|path| {
                    shared.set_status(format!("Loading {id} model…"), StatusLevel::Busy);
                    Engine::load(&path)
                });
                match result {
                    Ok(e) => {
                        engine = Some(e);
                        shared.model_ready.store(true, Ordering::Relaxed);
                        shared.set_status(format!("Ready · {} model", stt::model_info(&id).id), StatusLevel::Ok);
                    }
                    Err(e) => shared.set_status(e, StatusLevel::Error),
                }
            }
            EngineMsg::Transcribe(samples) => {
                let Some(engine) = engine.as_ref() else {
                    shared.set_phase(Phase::Error("Speech model is not loaded".into()));
                    continue;
                };
                process(&shared, engine, &samples);
            }
        }
        shared.repaint();
    }
}

fn process(shared: &Shared, engine: &Engine, samples: &[f32]) {
    let cfg = shared.cfg();
    let (text, detected) = match engine.transcribe(samples, &cfg.source_lang) {
        Ok(r) => r,
        Err(e) => return shared.set_phase(Phase::Error(e)),
    };
    if text.is_empty() {
        return shared.set_phase(Phase::Error("Didn't catch that — try again".into()));
    }

    let src = if cfg.source_lang == "auto" { detected } else { cfg.source_lang.clone() };
    let mut result = text.clone();
    let mut dst = src.clone();
    if cfg.translate && !languages::same_language(&src, &cfg.target_lang) {
        shared.set_phase(Phase::Processing("Translating".into()));
        match translate::translate(&text, &src, &cfg.target_lang) {
            Ok(t) => {
                result = t;
                dst = cfg.target_lang.clone();
            }
            Err(e) => return shared.set_phase(Phase::Error(format!("Translation failed: {e}"))),
        }
    } else if cfg.translate {
        dst = cfg.target_lang.clone();
    }

    shared.suppress_hotkey.store(true, Ordering::Relaxed);
    output::insert(&result, cfg.insert_method, cfg.restore_clipboard, cfg.auto_enter);
    std::thread::sleep(Duration::from_millis(60));
    shared.suppress_hotkey.store(false, Ordering::Relaxed);

    shared.set_phase(Phase::Done(result.clone()));
    let mut c = lock(&shared.cfg);
    c.history.insert(0, HistoryItem { time: now_string(), original: text, result, src, dst });
    c.history.truncate(100);
    c.save();
}

fn now_string() -> String {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    let mut t = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut t) };
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{} {} {:02}:{:02}", t.wDay, MONTHS[(t.wMonth.clamp(1, 12) - 1) as usize], t.wHour, t.wMinute)
}
