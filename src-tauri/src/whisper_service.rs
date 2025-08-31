use anyhow::{anyhow, Context, Result};
use hound::WavReader;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Instant;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
use log::{debug, error, info, warn};
use std::ffi::CStr;
use whisper_rs_sys as sys;

#[derive(serde::Serialize, Clone, Debug)]
pub struct BackendInfo {
    pub target_os: String,
    pub ggml_metal_path_resources: Option<String>,
    pub metallib_present: bool,
    pub likely_using_metal: bool,
    pub model_path: String,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ServiceTimings {
    pub sample_to_mono_ms: u128,
    pub resample_ms: u128,
    pub create_state_ms: u128,
    pub inference_ms: u128,
    pub collect_segments_ms: u128,
    pub total_ms: u128,
}

pub struct WhisperService {
    ctx: Arc<WhisperContext>,
    model_path: String,
}

impl WhisperService {
    pub fn from_model_path(model_path: &str) -> Result<Self> {
        let t0 = Instant::now();

        unsafe extern "C" fn ggml_logger(
            level: sys::ggml_log_level,
            text: *const ::std::os::raw::c_char,
            _userdata: *mut ::std::os::raw::c_void,
        ) {
            if text.is_null() { return; }
            let cstr = unsafe { CStr::from_ptr(text) };
            let msg = cstr.to_string_lossy();
            match level {
                x if x == sys::ggml_log_level_GGML_LOG_LEVEL_ERROR => error!("{}", msg),
                x if x == sys::ggml_log_level_GGML_LOG_LEVEL_WARN => warn!("{}", msg),
                x if x == sys::ggml_log_level_GGML_LOG_LEVEL_INFO => info!("{}", msg),
                x if x == sys::ggml_log_level_GGML_LOG_LEVEL_DEBUG => debug!("{}", msg),
                _ => info!("{}", msg),
            }
        }

        // Capture all internal ggml/whisper logs (Metal init, model load, buffers, etc.)
        unsafe { sys::whisper_log_set(Some(ggml_logger), std::ptr::null_mut()); }

        info!("Loading whisper model: {}", model_path);
        let ctx = WhisperContext::new_with_params(
            model_path,
            WhisperContextParameters::default(),
        )
        .with_context(|| format!("failed to load whisper model at {}", model_path))?;
        info!("whisper model loaded in {} ms", t0.elapsed().as_millis());
        Ok(Self { ctx: Arc::new(ctx), model_path: model_path.to_string() })
    }

    pub fn backend_info(&self) -> BackendInfo {
        let target_os = if cfg!(target_os = "macos") { "macos" } else { "other" }.to_string();
        let ggml_metal_path_resources = std::env::var("GGML_METAL_PATH_RESOURCES").ok();
        let metallib_present = ggml_metal_path_resources
            .as_ref()
            .map(|p| std::path::Path::new(p).join("default.metallib").exists())
            .unwrap_or(false);
        // Heuristic: on macOS, with metallib present, whisper-rs built with metal feature,
        // and env configured by app, we are likely using Metal. This doesn't guarantee it,
        // but is a strong indicator for our UI log.
        let likely_using_metal = cfg!(target_os = "macos") && metallib_present;
        BackendInfo {
            target_os,
            ggml_metal_path_resources,
            metallib_present,
            likely_using_metal,
            model_path: self.model_path.clone(),
        }
    }

    pub fn transcribe_wav_bytes_profiled(&self, wav_bytes: &[u8], language: Option<&str>, initial_prompt: Option<&str>) -> Result<(String, ServiceTimings)> {
        let t_total = Instant::now();
        // Decode WAV
        let cursor = Cursor::new(wav_bytes);
        let mut reader = WavReader::new(cursor).context("invalid WAV data")?;
        let spec = reader.spec();

        let channels = spec.channels as usize;
        if channels == 0 {
            return Err(anyhow!("WAV has zero channels"));
        }

        // Gather samples as f32 mono
        let t_mono_start = Instant::now();
        let mut mono_f32: Vec<f32> = Vec::new();
        match spec.sample_format {
            hound::SampleFormat::Int => {
                if spec.bits_per_sample <= 16 {
                    let mut tmp: Vec<i16> = Vec::new();
                    for s in reader.samples::<i16>() {
                        let v = s.context("error reading PCM sample")?;
                        tmp.push(v);
                    }
                    if channels == 1 {
                        mono_f32 = tmp
                            .iter()
                            .map(|v| (*v as f32) / (i16::MAX as f32))
                            .collect();
                    } else {
                        for frame in tmp.chunks_exact(channels) {
                            let sum: f32 = frame.iter().map(|v| *v as f32).sum();
                            let avg = sum / (channels as f32);
                            mono_f32.push(avg / (i16::MAX as f32));
                        }
                    }
                } else {
                    let mut tmp: Vec<i32> = Vec::new();
                    for s in reader.samples::<i32>() {
                        let v = s.context("error reading PCM sample")?;
                        tmp.push(v);
                    }
                    if channels == 1 {
                        mono_f32 = tmp
                            .iter()
                            .map(|v| (*v as f32) / (i32::MAX as f32))
                            .collect();
                    } else {
                        for frame in tmp.chunks_exact(channels) {
                            let sum: f32 = frame.iter().map(|v| *v as f32).sum();
                            let avg = sum / (channels as f32);
                            mono_f32.push(avg / (i32::MAX as f32));
                        }
                    }
                }
            }
            hound::SampleFormat::Float => {
                let mut tmp: Vec<f32> = Vec::new();
                for s in reader.samples::<f32>() {
                    let v = s.context("error reading float sample")?;
                    tmp.push(v);
                }
                if channels == 1 {
                    mono_f32 = tmp;
                } else {
                    for frame in tmp.chunks_exact(channels) {
                        let sum: f32 = frame.iter().sum();
                        let avg = sum / (channels as f32);
                        mono_f32.push(avg);
                    }
                }
            }
        }
        let sample_to_mono_ms = t_mono_start.elapsed().as_millis();

        // Resample if needed to 16kHz expected by whisper
        let src_rate = spec.sample_rate;
        let t_resample_start = Instant::now();
        let audio_16k = if src_rate != 16_000 {
            resample_linear(&mono_f32, src_rate, 16_000)
        } else {
            mono_f32
        };
        let resample_ms = t_resample_start.elapsed().as_millis();

        // Run whisper
        let t_state_start = Instant::now();
        let mut state = self
            .ctx
            .create_state()
            .context("failed to create whisper state")?;
        let create_state_ms = t_state_start.elapsed().as_millis();
        let mut params = FullParams::new(SamplingStrategy::BeamSearch {
            beam_size: 5,
            patience: -1.0,
        });
        if let Some(lang) = language {
            params.set_language(Some(lang));
        }
        if let Some(prompt) = initial_prompt {
            // whisper-rs exposes set_initial_prompt in FullParams as of 0.11
            params.set_initial_prompt(prompt);
        }
        let t_infer_start = Instant::now();
        state.full(params, &audio_16k[..]).context("whisper full failed")?;
        let inference_ms = t_infer_start.elapsed().as_millis();

        // Collect segments (API updated in whisper-rs 0.11)
        let t_collect_start = Instant::now();
        let mut out = String::new();
        let num_segments = match state.full_n_segments() {
            Ok(n) => n,
            Err(_) => 0,
        };
        for i in 0..num_segments {
            let seg_text = match state.full_get_segment_text(i as i32) {
                Ok(text) => text,
                Err(_) => String::new(),
            };
            if !seg_text.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(seg_text.trim());
            }
        }
        let collect_segments_ms = t_collect_start.elapsed().as_millis();
        let total_ms = t_total.elapsed().as_millis();

        let timings = ServiceTimings {
            sample_to_mono_ms,
            resample_ms,
            create_state_ms,
            inference_ms,
            collect_segments_ms,
            total_ms,
        };
        Ok((out, timings))
    }
}

fn resample_linear(input: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if input.is_empty() {
        return Vec::new();
    }
    if src_rate == dst_rate {
        return input.to_vec();
    }
    let src_len = input.len() as f32;
    let ratio = dst_rate as f32 / src_rate as f32;
    let out_len = (src_len * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let t = i as f32 / ratio;
        let i0 = t.floor() as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let frac = t - i0 as f32;
        let v = input[i0] * (1.0 - frac) + input[i1] * frac;
        out.push(v);
    }
    out
}


