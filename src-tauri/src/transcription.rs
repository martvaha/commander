use crate::config::{is_auto_paste_enabled, LanguageConfig, PromptConfig};
use anyhow::{anyhow, Result};
use std::time::Instant;
use tauri::{image::Image, AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Clone, serde::Serialize)]
pub struct TranscriptionEvent {
    pub text: String,
}

pub fn transcribe_and_copy(app: &AppHandle, pcm_mono_16: Vec<i16>, sample_rate_hz: u32) -> Result<()> {
    let t_total = Instant::now();
    let tmp_dir = std::env::temp_dir();
    let wav_path = tmp_dir.join("commander_record.wav");
    let t_wav_start = Instant::now();
    write_wav_mono(&wav_path, &pcm_mono_16, sample_rate_hz)?;
    let wav_write_ms = t_wav_start.elapsed().as_millis();

    let rt = tokio::runtime::Runtime::new()?;
    let (text, mut profile_json) = rt.block_on(async move {
        let client = reqwest::Client::new();
        let t_read_start = Instant::now();
        let bytes = tokio::fs::read(&wav_path).await?;
        let read_file_ms = t_read_start.elapsed().as_millis();
        let mut url = std::env::var("WHISPER_LOCAL_URL").unwrap_or_else(|_| "http://127.0.0.1:9000/transcribe".to_string());

        let mut maybe_lang: Option<String> = None;
        if let Ok(app_dir) = app.path().app_config_dir() {
            let path = app_dir.join("language.json");
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(cfg) = serde_json::from_str::<LanguageConfig>(&content) {
                        maybe_lang = cfg.language;
                    }
                }
            } else {
                maybe_lang = Some("en".to_string());
            }
        } else {
            maybe_lang = Some("en".to_string());
        }
        if let Some(lang) = maybe_lang {
            if !lang.is_empty() {
                if url.contains('?') { url.push('&'); } else { url.push('?'); }
                url.push_str(&format!("lang={}", urlencoding::encode(&lang)));
            }
        }

        let mut maybe_prompt: Option<String> = None;
        if let Ok(app_dir) = app.path().app_config_dir() {
            let path = app_dir.join("prompt.json");
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(cfg) = serde_json::from_str::<PromptConfig>(&content) {
                        maybe_prompt = cfg.prompt;
                    }
                }
            }
        }
        if let Some(prompt) = maybe_prompt {
            if !prompt.is_empty() {
                if url.contains('?') { url.push('&'); } else { url.push('?'); }
                url.push_str(&format!("prompt={}", urlencoding::encode(&prompt)));
            }
        }

        let t_http_start = Instant::now();
        let resp = client
            .post(url)
            .header("Content-Type", "audio/wav")
            .body(bytes)
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        let http_ms = t_http_start.elapsed().as_millis();
        if !status.is_success() {
            return Err(anyhow!(format!("HTTP {}: {}", status, body)));
        }

        let mut profile_json = serde_json::json!({
            "client": { "wav_write_ms": wav_write_ms, "read_file_ms": read_file_ms, "http_ms": http_ms }
        });
        let text = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                if let Some(p) = v.get("profile").cloned() {
                    profile_json["server"] = p;
                }
                t.to_string()
            } else { body }
        } else { body };

        Ok::<(String, serde_json::Value), anyhow::Error>((text, profile_json))
    })?;

    app.clipboard().write_text(text.clone())?;
    app.emit("transcription", TranscriptionEvent { text: text.clone() })?;
    let total_ms = t_total.elapsed().as_millis();
    profile_json["client"]["total_ms"] = serde_json::json!(total_ms);
    app.emit("transcription-profile", profile_json).ok();
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some("Transcribed and copied"));
        let _ = tray.set_icon(Some(Image::from_bytes(include_bytes!("../icons/commander-icon.png"))?));
    }
    let _ = app.emit("transcription-complete", true);

    if is_auto_paste_enabled(app) {
        trigger_auto_paste(text.clone());
    }
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if let Some(tray) = app2.tray_by_id("main-tray") {
            let _ = tray.set_tooltip(Some("Commander"));
        }
    });
    Ok(())
}

fn trigger_auto_paste(_text: String) {
    std::thread::spawn(move || {
        use std::time::Duration;
        std::thread::sleep(Duration::from_millis(150));
        #[cfg(target_os = "macos")]
        {
            use rdev::{simulate, EventType, Key};
            let _ = simulate(&EventType::KeyPress(Key::MetaLeft));
            std::thread::sleep(std::time::Duration::from_millis(5));
            let _ = simulate(&EventType::KeyPress(Key::KeyV));
            let _ = simulate(&EventType::KeyRelease(Key::KeyV));
            std::thread::sleep(std::time::Duration::from_millis(5));
            let _ = simulate(&EventType::KeyRelease(Key::MetaLeft));
        }
    });
}

fn write_wav_mono(path: &std::path::Path, samples: &[i16], sample_rate_hz: u32) -> Result<()> {
    let spec = hound::WavSpec { channels: 1, sample_rate: sample_rate_hz, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for s in samples { writer.write_sample(*s)?; }
    writer.finalize()?;
    Ok(())
}


