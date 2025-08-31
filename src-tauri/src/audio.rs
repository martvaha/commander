use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};
use serde::Serialize;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::SystemTime;
use hound::{WavSpec, WavWriter, SampleFormat};

#[derive(Debug)]
pub struct RecorderState {
    pub is_recording: bool,
    pub buffer: Vec<i16>,
    pub start_instant: Option<std::time::Instant>,
    pub sample_rate_hz: u32,
    pub last_level_emit: Option<Instant>,
}

impl RecorderState {
    pub fn new() -> Self {
        Self {
            is_recording: false,
            buffer: Vec::new(),
            start_instant: None,
            sample_rate_hz: 16_000,
            last_level_emit: None,
        }
    }
}

pub fn build_input_stream(recorder: Arc<Mutex<RecorderState>>, preferred_device_name: Option<String>, app: AppHandle) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = match preferred_device_name {
        Some(name) => match find_input_device_by_name(&host, &name) {
            Some(d) => d,
            None => host
                .default_input_device()
                .ok_or_else(|| anyhow!("No default input device"))?,
        },
        None => host
            .default_input_device()
            .ok_or_else(|| anyhow!("No default input device"))?,
    };
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    if let Ok(mut st) = recorder.lock() {
        st.sample_rate_hz = sample_rate;
    }
    let stream = match config.sample_format() {
        cpal::SampleFormat::I16 => {
            let app_handle = app.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _| on_audio(data, channels, &recorder, &app_handle),
                on_err,
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let app_handle = app.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[u16], _| {
                    let converted: Vec<i16> = data.iter().map(|s| ((*s as i32) - 32768) as i16).collect();
                    on_audio(&converted, channels, &recorder, &app_handle);
                },
                on_err,
                None,
            )?
        }
        cpal::SampleFormat::F32 => {
            let app_handle = app.clone();
            device.build_input_stream(
                &config.into(),
                move |data: &[f32], _| {
                    let converted: Vec<i16> = data.iter().map(|s| (s * i16::MAX as f32) as i16).collect();
                    on_audio(&converted, channels, &recorder, &app_handle);
                },
                on_err,
                None,
            )?
        }
        _ => return Err(anyhow!("Unsupported sample format")),
    };
    Ok(stream)
}

#[derive(Serialize, Clone, Debug)]
struct AudioLevelEvent {
    rms: f32,
    peak: f32,
    db: f32,
    recording: bool,
}

fn on_audio(input: &[i16], channels: usize, recorder: &Arc<Mutex<RecorderState>>, app: &AppHandle) {
    // Compute mono RMS and peak (normalized to [-1,1])
    let mut sum_squares: f32 = 0.0;
    let mut peak_abs: f32 = 0.0;
    let max_i16 = i16::MAX as f32;
    let mut frames_count: usize = 0;
    if channels == 1 {
        for &s in input.iter() {
            let v = (s as f32) / max_i16;
            sum_squares += v * v;
            let a = v.abs();
            if a > peak_abs { peak_abs = a; }
        }
        frames_count = input.len();
    } else if channels > 1 {
        for frame in input.chunks_exact(channels) {
            let sum: i32 = frame.iter().map(|v| *v as i32).sum();
            let avg = (sum as f32) / (channels as f32 * max_i16);
            sum_squares += avg * avg;
            let a = avg.abs();
            if a > peak_abs { peak_abs = a; }
            frames_count += 1;
        }
    }
    if frames_count > 0 {
        let rms = (sum_squares / frames_count as f32).sqrt();
        let db = 20.0 * (rms.max(1e-12)).log10();
        // Update recorder state, append audio if recording, and throttle event emission (~20 Hz)
        let mut should_emit = false;
        let mut is_recording_now = false;
        if let Ok(mut st) = recorder.lock() {
            // Append to buffer only when recording
            if st.is_recording {
                is_recording_now = true;
                if channels == 1 {
                    st.buffer.extend_from_slice(input);
                } else {
                    for frame in input.chunks_exact(channels) {
                        let sum: i32 = frame.iter().map(|v| *v as i32).sum();
                        let avg = (sum / channels as i32) as i16;
                        st.buffer.push(avg);
                    }
                }
            }
            let now = Instant::now();
            let do_emit = match st.last_level_emit {
                Some(t) => now.duration_since(t).as_millis() >= 50,
                None => true,
            };
            if do_emit {
                st.last_level_emit = Some(now);
                should_emit = true;
            }
        }
        if should_emit {
            let _ = app.emit("audio-level", AudioLevelEvent { rms, peak: peak_abs, db, recording: is_recording_now });
        }
    }
}

fn on_err(err: cpal::StreamError) {
    eprintln!("Audio stream error: {err}");
}


pub fn list_input_device_names() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices = host.input_devices()?;
    let mut names = Vec::new();
    for d in devices {
        if let Ok(n) = d.name() { names.push(n); }
    }
    Ok(names)
}

fn find_input_device_by_name(host: &cpal::Host, name: &str) -> Option<cpal::Device> {
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            if let Ok(n) = d.name() {
                if n == name { return Some(d); }
            }
        }
    }
    None
}


#[derive(Debug)]
pub enum AudioCommand {
    Rebuild { device: Option<String> },
}

#[derive(Clone)]
pub struct AudioController {
    tx: Arc<Mutex<Sender<AudioCommand>>>,
}

impl AudioController {
    pub fn set_device(&self, name: Option<String>) -> Result<()> {
        let tx = self.tx.lock().map_err(|_| anyhow!("controller unavailable"))?;
        tx.send(AudioCommand::Rebuild { device: name })
            .map_err(|e| anyhow!(format!("failed to send audio command: {}", e)))
    }
}

pub fn start_audio_controller(
    recorder: Arc<Mutex<RecorderState>>,
    app: AppHandle,
    initial_device: Option<String>,
) -> AudioController {
    let (tx, rx) = mpsc::channel::<AudioCommand>();
    let controller = AudioController { tx: Arc::new(Mutex::new(tx)) };
    let recorder_cloned = recorder.clone();
    let app_cloned = app.clone();
    thread::spawn(move || {
        let mut current_device = initial_device;
        let mut stream: Option<cpal::Stream> = match build_input_stream(recorder_cloned.clone(), current_device.clone(), app_cloned.clone()) {
            Ok(s) => {
                if let Err(e) = s.play() { eprintln!("Failed to play input stream: {}", e); }
                Some(s)
            }
            Err(e) => {
                eprintln!("Failed to build input stream initially: {}", e);
                None
            }
        };
        while let Ok(cmd) = rx.recv() {
            match cmd {
                AudioCommand::Rebuild { device } => {
                    if let Some(s) = stream.take() { drop(s); }
                    current_device = device;
                    match build_input_stream(recorder_cloned.clone(), current_device.clone(), app_cloned.clone()) {
                        Ok(s) => {
                            if let Err(e) = s.play() { eprintln!("Failed to play rebuilt input stream: {}", e); }
                            stream = Some(s);
                        }
                        Err(e) => {
                            eprintln!("Failed to rebuild input stream: {}", e);
                            stream = None;
                        }
                    }
                }
            }
        }
    });
    controller
}


pub fn save_recording_wav(app: &AppHandle, samples: &[i16], sample_rate_hz: u32) -> Result<std::path::PathBuf> {
    if samples.is_empty() { return Err(anyhow!("no samples to save")); }
    let base = app.path().app_data_dir().map_err(|e| anyhow!(format!("failed to get app data dir: {}", e)))?;
    let dir = base.join("recordings");
    std::fs::create_dir_all(&dir).map_err(|e| anyhow!(format!("failed to create recordings dir: {}", e)))?;
    let epoch = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_millis();
    let filename = format!("rec_{}_{}hz.wav", epoch, sample_rate_hz);
    let path = dir.join(filename);
    let spec = WavSpec { channels: 1, sample_rate: sample_rate_hz, bits_per_sample: 16, sample_format: SampleFormat::Int };
    let mut writer = WavWriter::create(&path, spec).map_err(|e| anyhow!(format!("failed to create wav: {}", e)))?;
    for s in samples {
        writer.write_sample(*s).map_err(|e| anyhow!(format!("failed to write sample: {}", e)))?;
    }
    writer.finalize().map_err(|e| anyhow!(format!("failed to finalize wav: {}", e)))?;
    Ok(path)
}


