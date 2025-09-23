// use anyhow::Context;
use log::{error, info, warn};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::path::BaseDirectory;
use tauri::{Emitter, Manager};
use tauri::image::Image;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
// use cpal::traits::StreamTrait;
mod http_server;
mod whisper_service;
mod audio;
mod config;
mod tray;
mod transcription;
mod platform;
mod commands;
use crate::audio::{start_audio_controller, RecorderState};
use crate::config::{is_hold_to_record_enabled, ShortcutConfig};
use crate::tray::{build_tray, make_recording_icon, make_transcribing_icon};
use crate::transcription::transcribe_and_copy;
use crate::config::{read_model_config, read_audio_input_config};
use crate::http_server::{start_whisper_server, load_model};

#[allow(dead_code)]
// commands moved to `commands` module

// commands moved to `commands` module

// commands moved to `commands` module

// commands moved to `commands` module

// commands moved to `commands` module

// commands moved to `commands` module

// commands moved to `commands` module

// moved to `config`

// commands moved to `commands` module

// commands moved to `commands` module

// moved to `config`

// commands moved to `commands` module

// commands moved to `commands` module

// moved to `transcription`

// moved to `audio`

// Tray helpers removed (moved to `tray`)

// Audio stream helpers removed (moved to `audio`)

// moved to `transcription`

// moved to `transcription`

// moved to `transcription`

// moved to `platform`

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let recorder = Arc::new(Mutex::new(RecorderState::new()));
    
    // Default shortcuts as fallback when custom shortcut fails
    let shortcuts = if cfg!(target_os = "macos") {
        vec![
            Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::F9),
        ]
    } else {
        vec![
            Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F9),
        ]
    };
    let recorder_for_stream = recorder.clone();

    tauri::Builder::default()
        .manage(recorder.clone())
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .level(log::LevelFilter::Debug)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir { file_name: Some("commander".to_string()) },
                ))
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
                .max_file_size(5_000_000)
                .build(),
        )
        
        .invoke_handler(tauri::generate_handler![
            commands::toggle_recording,
            commands::get_current_shortcut,
            commands::save_custom_shortcut,
            commands::get_default_language,
            commands::save_default_language,
            commands::get_default_prompt,
            commands::save_default_prompt,
            commands::get_auto_paste_enabled,
            commands::save_auto_paste_enabled,
            commands::get_hold_to_record_enabled,
            commands::save_hold_to_record_enabled,
            commands::get_models_status,
            commands::download_model,
            commands::select_model,
            commands::list_audio_input_devices,
            commands::get_selected_audio_input_device,
            commands::save_selected_audio_input_device,
            commands::apply_selected_audio_input_device,
            #[cfg(target_os = "macos")]
            commands::is_accessibility_trusted,
            #[cfg(target_os = "macos")]
            commands::open_accessibility_settings
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing; keep app running in tray
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler({
                    let recorder = recorder.clone();
                    move |app, _shortcut, event| {
                        let hold_mode = is_hold_to_record_enabled(&app);
                        match (hold_mode, event.state()) {
                            (true, ShortcutState::Pressed) => {
                                // Start recording on press (if not already recording)
                                let mut started = false;
                                if let Ok(mut st) = recorder.lock() {
                                    if !st.is_recording {
                                        st.is_recording = true;
                                        st.buffer.clear();
                                        st.start_instant = Some(Instant::now());
                                        started = true;
                                    }
                                }
                                if started {
                                    if let Some(tray) = app.tray_by_id("main-tray") {
                                        let _ = tray.set_tooltip(Some("Recording…"));
                                        let _ = tray.set_icon(Some(make_recording_icon()));
                                    }
                                    let _ = app.emit("recording-start", true);
                                }
                            }
                            (true, ShortcutState::Released) => {
                                // Stop recording on release and transcribe
                                let mut data = Vec::<i16>::new();
                                let mut should_transcribe = false;
                                if let Ok(mut st) = recorder.lock() {
                                    if st.is_recording {
                                        st.is_recording = false;
                                        std::mem::swap(&mut data, &mut st.buffer);
                                        st.start_instant = None;
                                        should_transcribe = true;
                                    }
                                }
                                if should_transcribe {
                                    if let Some(tray) = app.tray_by_id("main-tray") {
                                        let _ = tray.set_tooltip(Some("Transcribing…"));
                                        let _ = tray.set_icon(Some(make_transcribing_icon()));
                                    }
                                    let _ = app.emit("transcription-start", true);
                                    let _ = app.emit("recording-stop", true);
                                    let app2 = app.clone();
                                    let default_icon_bytes = include_bytes!("../icons/commander-icon.png");
                                    let default_icon = Image::from_bytes(default_icon_bytes).ok();
                                    let sample_rate = recorder
                                        .lock()
                                        .ok()
                                        .map(|s| s.sample_rate_hz)
                                        .unwrap_or(16_000);
                                    // Save WAV for debugging
                                    let _ = crate::audio::save_recording_wav(&app2, &data, sample_rate)
                                        .map(|p| println!("Saved recording: {}", p.display()));
                                    std::thread::spawn(move || {
                                        if let Err(err) = transcribe_and_copy(&app2, data, sample_rate) {
                                            error!("transcription error: {err:?}");
                                            if let Some(tray) = app2.tray_by_id("main-tray") {
                                                let _ = tray.set_tooltip(Some("Transcription failed"));
                                                if let Some(icon) = default_icon.clone() { let _ = tray.set_icon(Some(icon)); }
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
                                }
                            }
                            (false, ShortcutState::Pressed) => {
                                // Toggle behavior on press
                                let maybe_wav = {
                                    let mut data = Vec::<i16>::new();
                                    if let Ok(mut st) = recorder.lock() {
                                        if st.is_recording {
                                            st.is_recording = false;
                                            std::mem::swap(&mut data, &mut st.buffer);
                                            st.start_instant = None;
                                        } else {
                                            st.is_recording = true;
                                            st.buffer.clear();
                                            st.start_instant = Some(Instant::now());
                                        }
                                    }
                                    data
                                };
                                if maybe_wav.is_empty() {
                                    if let Some(tray) = app.tray_by_id("main-tray") {
                                        let _ = tray.set_tooltip(Some("Recording…"));
                                        let _ = tray.set_icon(Some(make_recording_icon()));
                                    }
                                    let _ = app.emit("recording-start", true);
                                } else {
                                    if let Some(tray) = app.tray_by_id("main-tray") {
                                        let _ = tray.set_tooltip(Some("Transcribing…"));
                                        let _ = tray.set_icon(Some(make_transcribing_icon()));
                                    }
                                    let _ = app.emit("transcription-start", true);
                                    let app2 = app.clone();
                                    let default_icon_bytes = include_bytes!("../icons/commander-icon.png");
                                    let default_icon = Image::from_bytes(default_icon_bytes).ok();
                                    let sample_rate = recorder
                                        .lock()
                                        .ok()
                                        .map(|s| s.sample_rate_hz)
                                        .unwrap_or(16_000);
                                    // Save WAV for debugging
                                    let _ = crate::audio::save_recording_wav(&app2, &maybe_wav, sample_rate)
                                        .map(|p| println!("Saved recording: {}", p.display()));
                                    std::thread::spawn(move || {
                                        if let Err(err) = transcribe_and_copy(&app2, maybe_wav, sample_rate) {
                                            error!("transcription error: {err:?}");
                                            if let Some(tray) = app2.tray_by_id("main-tray") {
                                                let _ = tray.set_tooltip(Some("Transcription failed"));
                                                if let Some(icon) = default_icon.clone() { let _ = tray.set_icon(Some(icon)); }
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
                                    let _ = app.emit("recording-stop", true);
                                }
                            }
                            (false, ShortcutState::Released) => {}
                        }
                    }
                })
                .build(),
        )
        .setup(move |app| {
            // On macOS, hide from Dock by switching activation policy to Accessory
            #[cfg(target_os = "macos")]
            {
                use tauri::ActivationPolicy;
                let _ = app.set_activation_policy(ActivationPolicy::Accessory);
            }
            // Emit current accessibility trust status to UI instead of auto-opening settings
            #[cfg(target_os = "macos")]
            {
                let trusted = platform::is_accessibility_trusted();
                let _ = app.emit("accessibility-status", serde_json::json!({"trusted": trusted}));
            }
            
            // Load saved shortcut configuration
            let config_dir = app.path().app_config_dir().ok();
            let saved_config = config_dir
                .and_then(|dir| std::fs::read_to_string(dir.join("shortcut.json")).ok())
                .and_then(|content| serde_json::from_str::<ShortcutConfig>(&content).ok())
                .unwrap_or_default();
            
            info!("Loading shortcut configuration: {:?} + {}", saved_config.modifiers, saved_config.key);
            
            // Force ggml to load metallib from our app resources dir to avoid mismatches
            // between crate-generated paths (e.g. target/debug) and the bundled metallib.
            let resources_dir = if cfg!(debug_assertions) {
                // In dev, use target/{debug|release} where build.rs copies default.metallib
                let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                let target_debug = base.join("target").join("debug");
                let target_release = base.join("target").join("release");
                if target_debug.join("default.metallib").exists() { target_debug }
                else if target_release.join("default.metallib").exists() { target_release }
                else { base.join("resources") }
            } else {
                // Bundle: metallib is packaged at the Resources/ root
                let root = match app.path().resolve("", BaseDirectory::Resource) {
                    Ok(p) => p,
                    Err(_) => std::path::PathBuf::from("."),
                };
                root
            };
            let resources_dir_str = resources_dir.to_string_lossy().to_string();
            std::env::set_var("GGML_METAL_PATH_RESOURCES", &resources_dir_str);
            info!("GGML_METAL_PATH_RESOURCES set to: {} (default.metallib present? {})",
                resources_dir_str,
                std::path::Path::new(&resources_dir).join("default.metallib").exists()
            );
            // Extra logging to see which kernel function (if any) is missing
            std::env::set_var("GGML_METAL_LOG_INFO", "1");

            // For bundled builds, provide sane defaults for ggml threading on Apple Silicon
            // if the environment hasn't set them already. These tend to give higher GPU
            // utilization while keeping CPU overhead low.
            if !cfg!(debug_assertions) {
                if std::env::var_os("GGML_METAL_N_THREADS").is_none() {
                    std::env::set_var("GGML_METAL_N_THREADS", "2");
                }
                if std::env::var_os("GGML_METAL_NCOMMAND_BUFFERS").is_none() {
                    std::env::set_var("GGML_METAL_NCOMMAND_BUFFERS", "4");
                }
                if std::env::var_os("GGML_N_THREADS").is_none() {
                    // Keep CPU threads low so GPU is the bottleneck.
                    std::env::set_var("GGML_N_THREADS", "1");
                }
                eprintln!(
                    "GGML tuning (release): GGML_METAL_N_THREADS={} GGML_METAL_NCOMMAND_BUFFERS={} GGML_N_THREADS={}",
                    std::env::var("GGML_METAL_N_THREADS").unwrap_or_default(),
                    std::env::var("GGML_METAL_NCOMMAND_BUFFERS").unwrap_or_default(),
                    std::env::var("GGML_N_THREADS").unwrap_or_default(),
                );
            }

            // Emit backend status to UI on startup
            {
                // Build minimal backend info without instantiating the model
                let metallib_present = std::path::Path::new(&resources_dir).join("default.metallib").exists();
                let backend = serde_json::json!({
                    "target_os": if cfg!(target_os = "macos") { "macos" } else { "other" },
                    "ggml_metal_path_resources": resources_dir_str,
                    "metallib_present": metallib_present,
                    "likely_using_metal": cfg!(target_os = "macos") && metallib_present
                });
                let _ = app.emit("backend-status", backend);
            }
            build_tray(app, recorder.clone())?;
            // Start local whisper server without model; load selected if present
            let bind_addr =
                std::env::var("WHISPER_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:9000".to_string());
            if let Err(e) = start_whisper_server(bind_addr) {
                error!("failed to start whisper server: {}", e);
            }
            // Attempt to load previously selected model from user data directory
            let app_handle = app.app_handle();
            if let Some(cfg) = read_model_config(&app_handle) {
                if let Some(id) = cfg.selected_id {
                    // Map id -> filename
                    if let Ok(models_dir) = app_handle.path().app_data_dir() {
                        let path = match id.as_str() {
                            "large-v3-turbo" => models_dir.join("models").join("ggml-large-v3-turbo.bin"),
                            "large-v3-turbo-q5_0" => models_dir.join("models").join("ggml-large-v3-turbo-q5_0.bin"),
                            _ => std::path::PathBuf::new(),
                        };
                        if path.exists() {
                            if let Err(e) = load_model(path.to_string_lossy().to_string()) {
                                warn!("Failed to load previously selected model: {}", e);
                            }
                        }
                    }
                }
            }
            // Start audio controller thread that owns the CPAL stream
            let preferred_device_name = read_audio_input_config(&app.app_handle()).and_then(|c| c.selected_device_name);
            let controller = start_audio_controller(recorder_for_stream, app.app_handle().clone(), preferred_device_name);
            app.manage(controller);
            // Ensure only our desired shortcuts are registered
            app.global_shortcut().unregister_all().ok();
            
            // Try to register the user's saved shortcut
            match saved_config.to_shortcut() {
                Ok(user_shortcut) => {
                    match app.global_shortcut().register(user_shortcut.clone()) {
                        Ok(_) => {
                            info!("✅ User's custom shortcut registered successfully!");
                            // Store the shortcut in app state for the handler
                            app.manage(vec![user_shortcut]);
                        }
                        Err(e) => {
                            error!("❌ Failed to register custom shortcut: {}", e);
                            warn!("Falling back to default shortcuts...");
                            
                            // Fallback to default shortcuts
                            let mut registered = false;
                            for (idx, shortcut) in shortcuts.iter().enumerate() {
                                if let Ok(_) = app.global_shortcut().register(shortcut.clone()) {
                                    registered = true;
                                    info!("✅ Fallback shortcut {} registered", idx + 1);
                                    break;
                                }
                            }
                            
                            if !registered {
                                error!("❌ Failed to register any shortcuts!");
                                warn!("On macOS, ensure accessibility permissions are granted.");
                            }
                            app.manage(shortcuts);
                        }
                    }
                }
                Err(e) => {
                    error!("❌ Invalid shortcut configuration: {}", e);
                    warn!("Using default shortcuts...");
                    
                    // Use default shortcuts
                    let mut registered_count = 0;
                    for (idx, shortcut) in shortcuts.iter().enumerate() {
                        if let Ok(_) = app.global_shortcut().register(shortcut.clone()) {
                            registered_count += 1;
                            info!("✅ Default shortcut {} registered", idx + 1);
                        }
                    }
                    
                    if registered_count == 0 {
                        error!("❌ Failed to register any shortcuts!");
                    }
                    app.manage(shortcuts);
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
