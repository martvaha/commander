use crate::audio::RecorderState;
use crate::transcription::transcribe_and_copy;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use tauri::{image::Image, Manager, Emitter};

pub fn make_recording_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/commander-icon-recording.png")).expect("failed to load recording tray icon")
}

pub fn make_transcribing_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/commander-icon-transcribing.png")).expect("failed to load transcribing tray icon")
}

pub fn build_tray(app: &tauri::App, recorder: Arc<Mutex<RecorderState>>) -> Result<()> {
    let icon_bytes = include_bytes!("../icons/commander-icon.png");
    let default_icon = Image::from_bytes(icon_bytes)?;
    let recording_icon = make_recording_icon();
    let transcribing_icon = make_transcribing_icon();

    let quit = tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let show = tauri::menu::MenuItem::with_id(app, "show", "Show Commander", true, None::<&str>)?;
    let simulate = tauri::menu::MenuItem::with_id(app, "simulate", "Toggle Recording", true, None::<&str>)?;

    let menu = tauri::menu::Menu::with_items(app, &[&show, &simulate, &quit])?;

    let rec_for_cb = recorder.clone();
    let tray = tauri::tray::TrayIconBuilder::with_id("main-tray")
        .icon(default_icon.clone())
        .menu(&menu)
        .tooltip("Commander")
        .on_menu_event(move |app, event| {
            if event.id.as_ref() == "quit" {
                app.exit(0);
                return;
            }
            if event.id.as_ref() == "show" {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
                return;
            }
            if event.id.as_ref() == "simulate" {
                let maybe_wav = {
                    let mut data = Vec::<i16>::new();
                    if let Ok(mut st) = rec_for_cb.lock() {
                        if st.is_recording {
                            st.is_recording = false;
                            std::mem::swap(&mut data, &mut st.buffer);
                            st.start_instant = None;
                        } else {
                            st.is_recording = true;
                            st.buffer.clear();
                            st.start_instant = Some(std::time::Instant::now());
                        }
                    }
                    data
                };
                if maybe_wav.is_empty() {
                    if let Some(tray) = app.tray_by_id("main-tray") {
                        let _ = tray.set_tooltip(Some("Recording…"));
                        let _ = tray.set_icon(Some(recording_icon.clone()));
                    }
                    let _ = app.emit("recording-start", true);
                } else {
                    if let Some(tray) = app.tray_by_id("main-tray") {
                        let _ = tray.set_tooltip(Some("Transcribing…"));
                        let _ = tray.set_icon(Some(transcribing_icon.clone()));
                    }
                    let _ = app.emit("transcription-start", true);
                    let app2 = app.clone();
                    let default_icon2 = default_icon.clone();
                    let sample_rate = rec_for_cb
                        .lock()
                        .ok()
                        .map(|s| s.sample_rate_hz)
                        .unwrap_or(16_000);
                    std::thread::spawn(move || {
                        if let Err(err) = transcribe_and_copy(&app2, maybe_wav, sample_rate) {
                            eprintln!("transcription error: {err:?}");
                            if let Some(tray) = app2.tray_by_id("main-tray") {
                                let _ = tray.set_tooltip(Some("Transcription failed"));
                                let _ = tray.set_icon(Some(default_icon2.clone()));
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
        })
        .menu(&menu)
        .on_tray_icon_event(|icon, event| match event {
            tauri::tray::TrayIconEvent::DoubleClick { .. } => {
                let app = icon.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            _ => {}
        })
        .build(app)?;

    tray.set_tooltip(Some("Commander"))?;
    tray.set_visible(true)?;
    Ok(())
}


