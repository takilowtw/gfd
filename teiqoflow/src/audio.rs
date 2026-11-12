use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample, Stream, StreamConfig};
use std::sync::{Arc, Mutex};

use crate::state::{Shared, lock};

pub const TARGET_SR: u32 = 16_000;

pub fn list_microphones() -> Vec<String> {
    let host = cpal::default_host();
    let mut names = Vec::new();
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(desc) = d.description() {
                let name = desc.name().to_string();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
    }
    names
}

fn find_device(name: Option<&str>) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if let Some(name) = name {
        if let Ok(mut devices) = host.input_devices() {
            if let Some(d) = devices.find(|d| d.description().map(|x| x.name() == name).unwrap_or(false)) {
                return Some(d);
            }
        }
    }
    host.default_input_device()
}

pub struct Recording {
    _stream: Stream,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
}

impl Recording {
    pub fn start(mic: Option<&str>, shared: Arc<Shared>) -> Result<Self, String> {
        let device = find_device(mic).ok_or("No microphone found")?;
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: StreamConfig = supported.config();
        let channels = config.channels as usize;
        let samples = Arc::new(Mutex::new(Vec::with_capacity(48_000 * 30)));

        let stream = match format {
            SampleFormat::F32 => build::<f32>(&device, &config, channels, samples.clone(), shared, |s| s),
            SampleFormat::I16 => build::<i16>(&device, &config, channels, samples.clone(), shared, |s| s as f32 / 32768.0),
            SampleFormat::I32 => build::<i32>(&device, &config, channels, samples.clone(), shared, |s| s as f32 / 2147483648.0),
            SampleFormat::U16 => build::<u16>(&device, &config, channels, samples.clone(), shared, |s| (s as f32 - 32768.0) / 32768.0),
            SampleFormat::U8 => build::<u8>(&device, &config, channels, samples.clone(), shared, |s| (s as f32 - 128.0) / 128.0),
            other => return Err(format!("Unsupported microphone format {other:?}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self { _stream: stream, samples, sample_rate: config.sample_rate })
    }

    /// Stops recording and returns 16 kHz mono audio.
    pub fn finish(self) -> Vec<f32> {
        let rate = self.sample_rate;
        let samples = std::mem::take(&mut *lock(&self.samples));
        drop(self);
        resample(&samples, rate, TARGET_SR)
    }
}

fn build<T: SizedSample + Send + 'static>(
    device: &cpal::Device,
    config: &StreamConfig,
    channels: usize,
    out: Arc<Mutex<Vec<f32>>>,
    shared: Arc<Shared>,
    convert: fn(T) -> f32,
) -> Result<Stream, String> {
    device
        .build_input_stream::<T, _, _>(
            config.clone(),
            move |data: &[T], _| {
                let mut sum = 0.0f32;
                let mut buf = lock(&out);
                for frame in data.chunks(channels.max(1)) {
                    let v = frame.iter().map(|&s| convert(s)).sum::<f32>() / frame.len() as f32;
                    sum += v * v;
                    buf.push(v);
                }
                let frames = (data.len() / channels.max(1)).max(1);
                let rms = (sum / frames as f32).sqrt();
                shared.set_level((rms * 14.0).min(1.0));
            },
            |err| eprintln!("audio stream error: {err}"),
            None,
        )
        .map_err(|e| e.to_string())
}

fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let n = (input.len() as f64 / ratio) as usize;
    (0..n)
        .map(|i| {
            let pos = i as f64 * ratio;
            let idx = pos as usize;
            let frac = (pos - idx as f64) as f32;
            let a = input[idx];
            let b = *input.get(idx + 1).unwrap_or(&a);
            a + (b - a) * frac
        })
        .collect()
}

pub fn rms(audio: &[f32]) -> f32 {
    if audio.is_empty() {
        return 0.0;
    }
    (audio.iter().map(|v| v * v).sum::<f32>() / audio.len() as f32).sqrt()
}
