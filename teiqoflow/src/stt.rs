use crate::config::data_dir;
use crate::i18n::tf;
use crate::state::{Shared, StatusLevel};
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Whisper,
    GigaAm,
}

pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    /// Base URL; files are downloaded from `{base}/{file}`. The first file is the model itself.
    pub base: &'static str,
    pub files: &'static [&'static str],
    pub size_mb: u32,
}

const WHISPER: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";
const GIGAAM: &str = "https://huggingface.co/istupakov/gigaam-v3-onnx/resolve/main";

pub const MODELS: &[ModelInfo] = &[
    ModelInfo { id: "gigaam-v3", label: "GigaAM v3 — Russian only, best for Russian", kind: Kind::GigaAm, base: GIGAAM, files: &["v3_e2e_ctc.int8.onnx", "v3_e2e_ctc_vocab.txt"], size_mb: 225 },
    ModelInfo { id: "tiny", label: "Tiny — fastest", kind: Kind::Whisper, base: WHISPER, files: &["ggml-tiny.bin"], size_mb: 75 },
    ModelInfo { id: "base", label: "Base — fast", kind: Kind::Whisper, base: WHISPER, files: &["ggml-base.bin"], size_mb: 142 },
    ModelInfo { id: "small", label: "Small — balanced", kind: Kind::Whisper, base: WHISPER, files: &["ggml-small-q5_1.bin"], size_mb: 181 },
    ModelInfo { id: "medium", label: "Medium — accurate", kind: Kind::Whisper, base: WHISPER, files: &["ggml-medium-q5_0.bin"], size_mb: 514 },
    ModelInfo { id: "large-v3-turbo", label: "Large v3 Turbo — best", kind: Kind::Whisper, base: WHISPER, files: &["ggml-large-v3-turbo-q5_0.bin"], size_mb: 547 },
];

pub fn model_info(id: &str) -> &'static ModelInfo {
    MODELS.iter().find(|m| m.id == id).unwrap_or(&MODELS[3])
}

fn models_dir() -> PathBuf {
    data_dir().join("models")
}

pub fn is_downloaded(id: &str) -> bool {
    model_info(id).files.iter().all(|f| models_dir().join(f).exists())
}

/// Downloads any missing files of the model and returns the path of the main file.
pub fn ensure_model(id: &str, shared: &Shared) -> Result<PathBuf, String> {
    let info = model_info(id);
    std::fs::create_dir_all(models_dir()).map_err(|e| e.to_string())?;
    for file in info.files {
        let path = models_dir().join(file);
        if !path.exists() {
            download(&format!("{}/{file}", info.base), &path, info, shared)?;
        }
    }
    Ok(models_dir().join(info.files[0]))
}

fn download(url: &str, path: &std::path::Path, info: &ModelInfo, shared: &Shared) -> Result<(), String> {
    let mut resp = crate::translate::download_agent().get(url).call().map_err(|e| format!("Download failed: {e}"))?;
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
        if pct != last_pct && total > 1_000_000 {
            last_pct = pct;
            shared.set_status(tf("Downloading {} model… {}%", &[&info.id, &pct]), StatusLevel::Busy);
        }
    }
    drop(file);
    std::fs::rename(&part, path).map_err(|e| e.to_string())?;
    Ok(())
}

pub enum Engine {
    #[cfg(feature = "whisper")]
    Whisper(whisper_rs::WhisperContext),
    GigaAm(crate::gigaam::GigaAm),
}

impl Engine {
    pub fn load(id: &str, path: &std::path::Path) -> Result<Self, String> {
        match model_info(id).kind {
            Kind::GigaAm => Ok(Engine::GigaAm(crate::gigaam::GigaAm::load(path)?)),
            #[cfg(feature = "whisper")]
            Kind::Whisper => {
                use whisper_rs::{WhisperContext, WhisperContextParameters};
                whisper_rs::install_logging_hooks();
                let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
                    .map_err(|e| format!("Could not load model: {e}"))?;
                Ok(Engine::Whisper(ctx))
            }
            #[cfg(not(feature = "whisper"))]
            Kind::Whisper => Err("Built without Whisper".into()),
        }
    }

    /// Short name of the hardware doing the work, shown in the status line.
    pub fn device(&self) -> &'static str {
        match self {
            #[cfg(feature = "whisper")]
            Engine::Whisper(_) => {
                if cfg!(any(feature = "vulkan", feature = "cuda")) { "GPU" } else { "CPU" }
            }
            Engine::GigaAm(g) => g.device(),
        }
    }

    /// Returns (text, detected language code). "auto" means unknown; the translator then detects it.
    pub fn transcribe(&mut self, audio: &[f32], language: &str) -> Result<(String, String), String> {
        match self {
            Engine::GigaAm(g) => {
                let text = g.transcribe(audio)?;
                let lang = script_language(&text).to_string();
                Ok((text, lang))
            }
            #[cfg(feature = "whisper")]
            Engine::Whisper(ctx) => whisper_transcribe(ctx, audio, language),
        }
    }
}

#[cfg(feature = "whisper")]
fn whisper_transcribe(ctx: &whisper_rs::WhisperContext, audio: &[f32], language: &str) -> Result<(String, String), String> {
    use whisper_rs::{FullParams, SamplingStrategy};
    let mut state = ctx.create_state().map_err(|e| e.to_string())?;
    // Greedy decoding is several times faster than beam search, which matters for push-to-talk.
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    // One thread per physical core; hyper-threads only add contention for whisper.cpp.
    let threads = (std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8) / 2).clamp(2, 8);
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

/// Strips Whisper annotations such as "[BLANK_AUDIO]" or "(music)".
#[cfg_attr(not(feature = "whisper"), allow(dead_code))]
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

/// GigaAM has no language detection and spells non-Russian words in Latin letters, so the
/// language is taken from the script: mostly Cyrillic is Russian, otherwise let the translator
/// detect it.
fn script_language(text: &str) -> &'static str {
    let cyrillic = text.chars().filter(|c| ('\u{0400}'..='\u{04FF}').contains(c)).count();
    let latin = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if cyrillic >= latin { "ru" } else { "auto" }
}
