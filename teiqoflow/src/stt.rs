use crate::config::data_dir;
use crate::state::{Shared, StatusLevel};
use std::io::{Read, Write};
use std::path::PathBuf;

pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo { id: "tiny", label: "Tiny — fastest", file: "ggml-tiny.bin", size_mb: 75 },
    ModelInfo { id: "base", label: "Base — fast", file: "ggml-base.bin", size_mb: 142 },
    ModelInfo { id: "small", label: "Small — balanced", file: "ggml-small-q5_1.bin", size_mb: 181 },
    ModelInfo { id: "medium", label: "Medium — accurate", file: "ggml-medium-q5_0.bin", size_mb: 514 },
    ModelInfo { id: "large-v3-turbo", label: "Large v3 Turbo — best", file: "ggml-large-v3-turbo-q5_0.bin", size_mb: 547 },
];

pub fn model_info(id: &str) -> &'static ModelInfo {
    MODELS.iter().find(|m| m.id == id).unwrap_or(&MODELS[2])
}

pub fn model_path(id: &str) -> PathBuf {
    data_dir().join("models").join(model_info(id).file)
}

pub fn ensure_model(id: &str, shared: &Shared) -> Result<PathBuf, String> {
    let path = model_path(id);
    if path.exists() {
        return Ok(path);
    }
    let info = model_info(id);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}", info.file);
    let mut resp = crate::translate::agent().get(&url).call().map_err(|e| format!("Download failed: {e}"))?;
    let total = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(info.size_mb as u64 * 1_048_576);
    let part = path.with_extension("part");
    let mut file = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut reader = resp.body_mut().with_config().limit(u64::MAX).reader();
    let mut buf = vec![0u8; 1 << 16];
    let mut done = 0u64;
    let mut last_pct = u64::MAX;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Download failed: {e}"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        let pct = done * 100 / total.max(1);
        if pct != last_pct {
            last_pct = pct;
            shared.set_status(format!("Downloading {} model… {pct}%", info.id), StatusLevel::Busy);
        }
    }
    drop(file);
    std::fs::rename(&part, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(feature = "whisper")]
pub struct Engine {
    ctx: whisper_rs::WhisperContext,
}

#[cfg(feature = "whisper")]
impl Engine {
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        use whisper_rs::{WhisperContext, WhisperContextParameters};
        whisper_rs::install_logging_hooks();
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| format!("Could not load model: {e}"))?;
        Ok(Self { ctx })
    }

    /// Returns (text, detected language code).
    pub fn transcribe(&self, audio: &[f32], language: &str) -> Result<(String, String), String> {
        use whisper_rs::{FullParams, SamplingStrategy};
        let mut state = self.ctx.create_state().map_err(|e| e.to_string())?;
        let mut params = FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
        params.set_n_threads(threads as i32);
        let lang = if language == "auto" { "auto" } else { crate::languages::whisper_code(language) };
        params.set_language(Some(lang));
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);

        // Very short clips make Whisper hallucinate; pad to one second of silence.
        let mut padded = audio.to_vec();
        if padded.len() < 16_000 {
            padded.resize(16_000, 0.0);
        }
        state.full(params, &padded).map_err(|e| e.to_string())?;

        let mut text = String::new();
        for i in 0..state.full_n_segments() {
            if let Some(seg) = state.get_segment(i) {
                if let Ok(s) = seg.to_str_lossy() {
                    text.push_str(s.trim());
                    text.push(' ');
                }
            }
        }
        let detected = whisper_rs::get_lang_str(state.full_lang_id_from_state()).unwrap_or(lang).to_string();
        Ok((clean(&text), detected))
    }
}

#[cfg(not(feature = "whisper"))]
pub struct Engine;

#[cfg(not(feature = "whisper"))]
impl Engine {
    pub fn load(_: &std::path::Path) -> Result<Self, String> {
        Err("Built without speech recognition".into())
    }
    pub fn transcribe(&self, _: &[f32], _: &str) -> Result<(String, String), String> {
        Err("Built without speech recognition".into())
    }
}

/// Strips Whisper annotations such as "[BLANK_AUDIO]" or "(music)".
fn clean(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
