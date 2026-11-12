//! GigaAM v3 end-to-end CTC (Sber, Russian) running on ONNX Runtime.
//!
//! Feature extraction mirrors onnx-asr's `GigaamPreprocessorV3`: 64 HTK mel bands, 20 ms Hann
//! window, 10 ms hop, no centring, natural log. The window and filterbank are rounded to bfloat16
//! exactly like the reference, because the model was trained on those values.

use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;
use std::f64::consts::PI;
use std::path::Path;

const SR: usize = 16_000;
const N_FFT: usize = 320;
const HOP: usize = 160;
const N_MELS: usize = 64;
const N_FREQS: usize = N_FFT / 2 + 1;
/// Longer clips are split so attention cost stays small.
const CHUNK: usize = SR * 20;

pub struct GigaAm {
    session: Session,
    vocab: Vec<String>,
    blank: usize,
    window: Vec<f32>,
    fbanks: Vec<f32>, // [N_FREQS][N_MELS]
    cos: Vec<f32>,    // [N_FREQS][N_FFT]
    sin: Vec<f32>,
}

fn bf16(x: f64) -> f32 {
    let bits = (x as f32).to_bits();
    let rounded = bits.wrapping_add(0x7FFF + ((bits >> 16) & 1)) & 0xFFFF_0000;
    f32::from_bits(rounded)
}

fn hz_to_mel(f: f64) -> f64 {
    2595.0 * (1.0 + f / 700.0).log10()
}

fn mel_to_hz(m: f64) -> f64 {
    700.0 * (10f64.powf(m / 2595.0) - 1.0)
}

fn mel_filterbank() -> Vec<f32> {
    let (m_min, m_max) = (hz_to_mel(0.0), hz_to_mel(8000.0));
    let pts: Vec<f64> = (0..N_MELS + 2).map(|i| mel_to_hz(m_min + (m_max - m_min) * i as f64 / (N_MELS + 1) as f64)).collect();
    let mut fb = vec![0f32; N_FREQS * N_MELS];
    for k in 0..N_FREQS {
        let f = (SR / 2) as f64 * k as f64 / (N_FREQS - 1) as f64;
        for m in 0..N_MELS {
            let up = (f - pts[m]) / (pts[m + 1] - pts[m]);
            let down = (pts[m + 2] - f) / (pts[m + 2] - pts[m + 1]);
            fb[k * N_MELS + m] = bf16(up.min(down).max(0.0));
        }
    }
    fb
}

impl GigaAm {
    pub fn load(model: &Path) -> Result<Self, String> {
        let vocab_path = model.with_file_name("v3_e2e_ctc_vocab.txt");
        let text = std::fs::read_to_string(&vocab_path).map_err(|e| format!("GigaAM vocabulary: {e}"))?;
        let mut vocab: Vec<String> = Vec::new();
        for line in text.lines() {
            let Some((token, id)) = line.rsplit_once(' ') else { continue };
            let id: usize = id.trim().parse().map_err(|_| format!("Bad vocabulary line: {line}"))?;
            if vocab.len() <= id {
                vocab.resize(id + 1, String::new());
            }
            vocab[id] = token.replace('\u{2581}', " ");
        }
        let blank = vocab.iter().position(|t| t == "<blk>").unwrap_or(vocab.len().saturating_sub(1));

        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8) / 2;
        let env = ort::init().with_name("teiqoflow").build().map_err(|e| e.to_string())?;
        let session = Session::builder(&env)
            .and_then(|b| b.with_optimization_level(GraphOptimizationLevel::Level3).map_err(Into::into))
            .and_then(|b| b.with_intra_threads(threads.max(2)).map_err(Into::into))
            .and_then(|mut b| b.commit_from_file(model))
            .map_err(|e| format!("Could not load GigaAM: {e}"))?;

        let window: Vec<f32> = (0..N_FFT).map(|n| bf16(0.5 - 0.5 * (2.0 * PI * n as f64 / N_FFT as f64).cos())).collect();
        let mut cos = vec![0f32; N_FREQS * N_FFT];
        let mut sin = vec![0f32; N_FREQS * N_FFT];
        for k in 0..N_FREQS {
            for n in 0..N_FFT {
                let a = 2.0 * PI * (k * n % N_FFT) as f64 / N_FFT as f64;
                cos[k * N_FFT + n] = a.cos() as f32;
                sin[k * N_FFT + n] = a.sin() as f32;
            }
        }
        Ok(Self { session, vocab, blank, window, fbanks: mel_filterbank(), cos, sin })
    }

    pub fn device(&self) -> &'static str {
        "CPU"
    }

    pub fn transcribe(&mut self, audio: &[f32]) -> Result<String, String> {
        let mut parts = Vec::new();
        for chunk in audio.chunks(CHUNK) {
            let text = self.transcribe_chunk(chunk)?;
            if !text.is_empty() {
                parts.push(text);
            }
        }
        Ok(parts.join(" "))
    }

    /// Log-mel features laid out as [N_MELS][frames].
    fn features(&self, audio: &[f32]) -> (Vec<f32>, usize) {
        let mut padded;
        let audio = if audio.len() < N_FFT {
            padded = audio.to_vec();
            padded.resize(N_FFT, 0.0);
            &padded[..]
        } else {
            audio
        };
        let frames = (audio.len() - N_FFT) / HOP + 1;
        let mut feats = vec![0f32; N_MELS * frames];
        let mut frame = [0f32; N_FFT];
        let mut power = [0f32; N_FREQS];
        for t in 0..frames {
            let start = t * HOP;
            for n in 0..N_FFT {
                frame[n] = audio[start + n] * self.window[n];
            }
            for k in 0..N_FREQS {
                let (c, s) = (&self.cos[k * N_FFT..(k + 1) * N_FFT], &self.sin[k * N_FFT..(k + 1) * N_FFT]);
                let (mut re, mut im) = (0f32, 0f32);
                for n in 0..N_FFT {
                    re += frame[n] * c[n];
                    im -= frame[n] * s[n];
                }
                power[k] = re * re + im * im;
            }
            for m in 0..N_MELS {
                let mut e = 0f32;
                for k in 0..N_FREQS {
                    e += power[k] * self.fbanks[k * N_MELS + m];
                }
                feats[m * frames + t] = e.clamp(1e-9, 1e9).ln();
            }
        }
        (feats, frames)
    }

    fn transcribe_chunk(&mut self, audio: &[f32]) -> Result<String, String> {
        let (feats, frames) = self.features(audio);
        let features = Tensor::from_array(([1usize, N_MELS, frames], feats)).map_err(|e| e.to_string())?;
        let lengths = Tensor::from_array(([1usize], vec![frames as i64])).map_err(|e| e.to_string())?;
        let outputs = self
            .session
            .run(ort::inputs!["features" => features, "feature_lengths" => lengths])
            .map_err(|e| format!("GigaAM: {e}"))?;
        let (shape, probs) = outputs["log_probs"].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        let (steps, classes) = (shape[1] as usize, shape[2] as usize);
        let valid = ((frames - 1) / 4 + 1).min(steps);

        // Greedy CTC: best token per step, collapse repeats, drop blanks.
        let mut text = String::new();
        let mut prev = self.blank;
        for t in 0..valid {
            let row = &probs[t * classes..(t + 1) * classes];
            let best = row.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i).unwrap_or(self.blank);
            if best != self.blank && best != prev {
                if let Some(tok) = self.vocab.get(best) {
                    if tok != "<unk>" {
                        text.push_str(tok);
                    }
                }
            }
            prev = best;
        }
        Ok(tidy_spaces(&text))
    }
}

/// Same rule as onnx-asr: keep a single space only where a word follows it.
fn tidy_spaces(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_whitespace() {
            let next_is_word = chars.get(i + 1).is_some_and(|n| n.is_alphanumeric() || *n == '_');
            if !out.is_empty() && next_is_word && !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}
