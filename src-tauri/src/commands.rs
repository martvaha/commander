use crate::audio::{RecorderState, list_input_device_names, AudioController, save_recording_wav};
use crate::config::{AutoPasteConfig, HoldToRecordConfig, LanguageConfig, PromptConfig, ShortcutConfig, read_model_config, write_model_config, read_audio_input_config, write_audio_input_config, AudioInputConfig};
use crate::http_server::{load_model, is_model_loaded};
use crate::transcription::transcribe_and_copy;
use crate::tray::{make_recording_icon, make_transcribing_icon};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{image::Image, AppHandle, Manager, Emitter};
use anyhow::Result as AnyResult;
use std::io::Write;

#[tauri::command]
pub fn get_current_shortcut(app_handle: tauri::AppHandle) -> Result<ShortcutConfig, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    let config_path = config_dir.join("shortcut.json");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
    } else {
        Ok(ShortcutConfig::default())
    }
}

#[tauri::command]
pub fn save_custom_shortcut(app_handle: tauri::AppHandle, config: ShortcutConfig) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
    let config_path = config_dir.join("shortcut.json");
    let content = serde_json::to_string_pretty(&config).map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Failed to write config: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_default_language(app_handle: tauri::AppHandle) -> Result<Option<String>, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    let config_path = config_dir.join("language.json");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read language config: {}", e))?;
        let cfg: LanguageConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse language config: {}", e))?;
        Ok(cfg.language)
    } else {
        Ok(Some("en".to_string()))
    }
}

#[tauri::command]
pub fn save_default_language(app_handle: tauri::AppHandle, language: Option<String>) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
    let config_path = config_dir.join("language.json");
    let cfg = LanguageConfig { language };
    let content = serde_json::to_string_pretty(&cfg).map_err(|e| format!("Failed to serialize language config: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Failed to write language config: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_default_prompt(app_handle: tauri::AppHandle) -> Result<Option<String>, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    let config_path = config_dir.join("prompt.json");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read prompt config: {}", e))?;
        let cfg: PromptConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse prompt config: {}", e))?;
        Ok(cfg.prompt)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn save_default_prompt(app_handle: tauri::AppHandle, prompt: Option<String>) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
    let config_path = config_dir.join("prompt.json");
    let cfg = PromptConfig { prompt };
    let content = serde_json::to_string_pretty(&cfg).map_err(|e| format!("Failed to serialize prompt config: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Failed to write prompt config: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_auto_paste_enabled(app_handle: tauri::AppHandle) -> Result<bool, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    let config_path = config_dir.join("auto_paste.json");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read auto-paste config: {}", e))?;
        let cfg: AutoPasteConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse auto-paste config: {}", e))?;
        Ok(cfg.enabled)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn save_auto_paste_enabled(app_handle: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
    let config_path = config_dir.join("auto_paste.json");
    let cfg = AutoPasteConfig { enabled };
    let content = serde_json::to_string_pretty(&cfg).map_err(|e| format!("Failed to serialize auto-paste config: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Failed to write auto-paste config: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_hold_to_record_enabled(app_handle: tauri::AppHandle) -> Result<bool, String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    let config_path = config_dir.join("hold_to_record.json");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| format!("Failed to read hold-to-record config: {}", e))?;
        let cfg: HoldToRecordConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse hold-to-record config: {}", e))?;
        Ok(cfg.enabled)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub fn save_hold_to_record_enabled(app_handle: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let config_dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("Failed to create config dir: {}", e))?;
    let config_path = config_dir.join("hold_to_record.json");
    let cfg = HoldToRecordConfig { enabled };
    let content = serde_json::to_string_pretty(&cfg).map_err(|e| format!("Failed to serialize hold-to-record config: {}", e))?;
    std::fs::write(&config_path, content).map_err(|e| format!("Failed to write hold-to-record config: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn toggle_recording(
    recorder: tauri::State<Arc<Mutex<RecorderState>>>,
    app_handle: AppHandle,
) -> Result<String, String> {
    if !is_model_loaded() {
        let _ = app_handle.emit("no-model-selected", true);
        return Err("No model selected. Please select and download a model first.".to_string());
    }
    let recording_icon = make_recording_icon();
    let transcribing_icon = make_transcribing_icon();
    let maybe_wav = {
        let mut data = Vec::<i16>::new();
        let mut is_recording = false;
        if let Ok(mut st) = recorder.lock() {
            if st.is_recording {
                st.is_recording = false;
                std::mem::swap(&mut data, &mut st.buffer);
                st.start_instant = None;
            } else {
                st.is_recording = true;
                st.buffer.clear();
                st.start_instant = Some(Instant::now());
                is_recording = true;
            }
        }
        (data, is_recording)
    };

    if maybe_wav.0.is_empty() {
        if let Some(tray) = app_handle.tray_by_id("main-tray") {
            let _ = tray.set_tooltip(Some("Recording…"));
            let _ = tray.set_icon(Some(recording_icon));
        }
        let _ = app_handle.emit("recording-start", true);
        Ok("Recording started".to_string())
    } else {
        if let Some(tray) = app_handle.tray_by_id("main-tray") {
            let _ = tray.set_tooltip(Some("Transcribing…"));
            let _ = tray.set_icon(Some(transcribing_icon));
        }
        let _ = app_handle.emit("transcription-start", true);
        let app2 = app_handle.clone();
        let sample_rate = recorder
            .lock()
            .ok()
            .map(|s| s.sample_rate_hz)
            .unwrap_or(16_000);
        // Save WAV for debugging
        let _ = save_recording_wav(&app_handle, &maybe_wav.0, sample_rate)
            .map(|p| println!("Saved recording: {}", p.display()));
        let _ = app_handle.emit("recording-stop", true);
        std::thread::spawn(move || {
            if let Err(err) = transcribe_and_copy(&app2, maybe_wav.0, sample_rate) {
                eprintln!("transcription error: {err:?}");
                if let Some(tray) = app2.tray_by_id("main-tray") {
                    let _ = tray.set_tooltip(Some("Transcription failed"));
                    let default_icon = Image::from_bytes(include_bytes!("../icons/commander-icon.png")).ok();
                    if let Some(icon) = default_icon { let _ = tray.set_icon(Some(icon)); }
                }
                let _ = app2.emit("transcription-failed", true);
                let app3 = app2.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    if let Some(tray) = app3.tray_by_id("main-tray") {
                        let _ = tray.set_tooltip(Some("Commander"));
                    }
                });
            }
        });
        Ok("Recording stopped, transcribing...".to_string())
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub url: String,
    pub filename: String,
    pub approx_size_mb: u64,
}

fn model_catalog() -> Vec<ModelEntry> {
    vec![
        ModelEntry {
            id: "large-v3-turbo".to_string(),
            name: "Large v3 Turbo".to_string(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin?download=true".to_string(),
            filename: "ggml-large-v3-turbo.bin".to_string(),
            approx_size_mb: 6000,
        },
        ModelEntry {
            id: "large-v3-turbo-q5_0".to_string(),
            name: "Large v3 Turbo (Q5_0)".to_string(),
            url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin?download=true".to_string(),
            filename: "ggml-large-v3-turbo-q5_0.bin".to_string(),
            approx_size_mb: 3500,
        },
    ]
}

fn models_dir(app: &AppHandle) -> AnyResult<std::path::PathBuf> {
    let dir = app.path().app_data_dir()?;
    let dir = dir.join("models");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ModelStatusItem {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub installed: bool,
    pub size_bytes: Option<u64>,
    pub approx_size_mb: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ModelsStatus {
    pub available: Vec<ModelStatusItem>,
    pub selected_id: Option<String>,
}

#[tauri::command]
pub fn get_models_status(app_handle: tauri::AppHandle) -> Result<ModelsStatus, String> {
    let dir = models_dir(&app_handle).map_err(|e| e.to_string())?;
    let selected = read_model_config(&app_handle).and_then(|c| c.selected_id);
    let mut out = Vec::new();
    for m in model_catalog() {
        let path = dir.join(&m.filename);
        let (installed, size_bytes) = if path.exists() {
            match std::fs::metadata(&path) {
                Ok(md) => (true, Some(md.len())),
                Err(_) => (true, None),
            }
        } else { (false, None) };
        out.push(ModelStatusItem {
            id: m.id,
            name: m.name,
            filename: m.filename,
            installed,
            size_bytes,
            approx_size_mb: m.approx_size_mb,
        });
    }
    Ok(ModelsStatus { available: out, selected_id: selected })
}

#[tauri::command]
pub fn select_model(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let dir = models_dir(&app_handle).map_err(|e| e.to_string())?;
    let m = model_catalog().into_iter().find(|x| x.id == id).ok_or_else(|| "unknown model id".to_string())?;
    let path = dir.join(&m.filename);
    if !path.exists() {
        return Err("Model not installed".to_string());
    }
    load_model(path.to_string_lossy().to_string()).map_err(|e| format!("Failed to load model: {}", e))?;
    let mut cfg = read_model_config(&app_handle).unwrap_or_default();
    cfg.selected_id = Some(m.id);
    write_model_config(&app_handle, &cfg).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DownloadProgress {
    pub id: String,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
}

#[tauri::command]
pub fn download_model(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let m = model_catalog().into_iter().find(|x| x.id == id).ok_or_else(|| "unknown model id".to_string())?;
    let dir = models_dir(&app_handle).map_err(|e| e.to_string())?;
    let final_path = dir.join(&m.filename);
    if final_path.exists() {
        return Ok(());
    }
    let partial_path = dir.join(format!("{}.partial", &m.filename));
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() { Ok(rt) => rt, Err(e) => { let _ = app_handle.emit("model-download-error", format!("{}", e)); return; } };
        let id_clone = id.clone();
        rt.block_on(async move {
            let client = reqwest::Client::new();
            let resp = match client.get(&m.url).send().await { Ok(r) => r, Err(e) => { let _ = app_handle.emit("model-download-error", format!("{}", e)); return; } };
            let total = resp.content_length();
            let _ = app_handle.emit("model-download-start", serde_json::json!({"id": id_clone, "total_bytes": total}));
            let mut stream = resp.bytes_stream();
            let mut file = match std::fs::File::create(&partial_path) { Ok(f) => f, Err(e) => { let _ = app_handle.emit("model-download-error", format!("{}", e)); return; } };
            let mut received: u64 = 0;
            use futures_util::StreamExt;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        if let Err(e) = file.write_all(&bytes) { let _ = app_handle.emit("model-download-error", format!("{}", e)); return; }
                        received += bytes.len() as u64;
                        let _ = app_handle.emit("model-download-progress", serde_json::json!({"id": id_clone, "received_bytes": received, "total_bytes": total}));
                    }
                    Err(e) => { let _ = app_handle.emit("model-download-error", format!("{}", e)); return; }
                }
            }
            if let Err(e) = file.flush() { let _ = app_handle.emit("model-download-error", format!("{}", e)); return; }
            if let Err(e) = std::fs::rename(&partial_path, &final_path) { let _ = app_handle.emit("model-download-error", format!("{}", e)); return; }
            // Auto-select and load
            if let Err(e) = load_model(final_path.to_string_lossy().to_string()) { let _ = app_handle.emit("model-download-error", format!("{}", e)); return; }
            let mut cfg = read_model_config(&app_handle).unwrap_or_default();
            cfg.selected_id = Some(id_clone.clone());
            if let Err(e) = write_model_config(&app_handle, &cfg) { let _ = app_handle.emit("model-download-error", format!("{}", e)); return; }
            let _ = app_handle.emit("model-download-complete", serde_json::json!({"id": id_clone, "selected": true}));
        });
    });
    Ok(())
}

#[tauri::command]
pub fn list_audio_input_devices() -> Result<Vec<String>, String> {
    list_input_device_names().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_selected_audio_input_device(app_handle: tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(read_audio_input_config(&app_handle).and_then(|c| c.selected_device_name))
}

#[tauri::command]
pub fn save_selected_audio_input_device(app_handle: tauri::AppHandle, name: Option<String>) -> Result<(), String> {
    let cfg = AudioInputConfig { selected_device_name: name };
    write_audio_input_config(&app_handle, &cfg).map_err(|e| e.to_string())
}


#[tauri::command]
pub fn apply_selected_audio_input_device(app_handle: tauri::AppHandle, controller: tauri::State<AudioController>) -> Result<(), String> {
    let selected = read_audio_input_config(&app_handle).and_then(|c| c.selected_device_name);
    controller.set_device(selected).map_err(|e| e.to_string())
}


