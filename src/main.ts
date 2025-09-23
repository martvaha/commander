import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { attachConsole, trace, debug as logDebug, info as logInfo, warn as logWarn, error as logError } from '@tauri-apps/plugin-log';

window.addEventListener('DOMContentLoaded', async () => {
  // Forward console to Tauri log plugin (file + stdout)
  try {
    await attachConsole();
    const forwardConsole = (
      fnName: 'log' | 'debug' | 'info' | 'warn' | 'error',
      logger: (message: string) => Promise<void>
    ) => {
      const original = console[fnName];
      console[fnName] = (message?: any, ...optionalParams: any[]) => {
        try { original(message, ...optionalParams); } catch {}
        try { logger(String(message)); } catch {}
      };
    };
    forwardConsole('log', trace);
    forwardConsole('debug', logDebug);
    forwardConsole('info', logInfo);
    forwardConsole('warn', logWarn);
    forwardConsole('error', logError);
  } catch {}
  // Log shortcut info on startup
  const isMac = navigator.platform.includes('Mac');
  console.log(`🎤 Commander is ready!`);
  console.log(
    'Note: On macOS, ensure Accessibility permissions are granted in System Settings.'
  );

  // Helper to add log messages
  const addLog = (message: string) => {
    const logDiv = document.getElementById('log');
    if (logDiv) {
      const timestamp = new Date().toLocaleTimeString();
      logDiv.innerHTML =
        `<div>[${timestamp}] ${message}</div>` + logDiv.innerHTML;
    }
  };

  // Helper to update status
  const updateStatus = (status: string) => {
    const statusEl = document.getElementById('status');
    if (statusEl) {
      statusEl.textContent = status;
    }
  };

  // Track recording state for button text
  let isRecording = false;
  const axBanner = document.getElementById('ax-banner') as HTMLDivElement | null;
  const openAxBtn = document.getElementById('open-ax-settings-btn') as HTMLButtonElement | null;
  const recheckAxBtn = document.getElementById('recheck-ax-btn') as HTMLButtonElement | null;
  const recordBtn = document.getElementById(
    'test-recording-btn'
  ) as HTMLButtonElement;
  console.log(isRecording);

  // Handle button click
  if (recordBtn) {
    recordBtn.addEventListener('click', async () => {
      try {
        recordBtn.disabled = true;
        const result = await invoke('toggle_recording');
        console.log('Toggle result:', result);
      } catch (error) {
        console.error('Failed to toggle recording:', error);
        addLog(`❌ Error: ${error}`);
        updateStatus('Error - Ready');
      } finally {
        // Re-enable after a short delay to prevent double-clicks
        setTimeout(() => {
          recordBtn.disabled = false;
        }, 500);
      }
    });
  }

  // Handle backend-emitted accessibility status
  await listen('accessibility-status', (e) => {
    try {
      const trusted = !!(e.payload as any)?.trusted;
      if (axBanner) axBanner.style.display = trusted ? 'none' : 'block';
    } catch {}
  });

  // Initial client-side check (only works on macOS build where commands exist)
  if (isMac) {
    try {
      const trusted = (await invoke('is_accessibility_trusted')) as boolean;
      if (axBanner) axBanner.style.display = trusted ? 'none' : 'block';
    } catch {
      // Command may not exist on non-mac builds; ignore
    }
  }

  if (openAxBtn) {
    openAxBtn.addEventListener('click', async () => {
      try {
        await invoke('open_accessibility_settings');
      } catch (e) {
        console.error('Failed to open Accessibility settings:', e);
      }
    });
  }

  if (recheckAxBtn) {
    recheckAxBtn.addEventListener('click', async () => {
      try {
        const trusted = (await invoke('is_accessibility_trusted')) as boolean;
        if (axBanner) axBanner.style.display = trusted ? 'none' : 'block';
      } catch (e) {
        console.error('Failed to recheck Accessibility status:', e);
      }
    });
  }

  await listen('recording-start', () => {
    console.log('🔴 Recording started');
    addLog('🔴 Recording started');
    updateStatus('Recording...');
    document.body.style.borderTop = '5px solid #E52222';
    isRecording = true;
    if (recordBtn) {
      recordBtn.textContent = '⏹️ Stop Recording';
      recordBtn.style.background = '#E52222';
    }
  });

  await listen('recording-stop', () => {
    console.log('⏹️ Recording stopped');
    addLog('⏹️ Recording stopped');
    updateStatus('Processing...');
    document.body.style.borderTop = '5px solid #F2B82F';
    isRecording = false;
    if (recordBtn) {
      recordBtn.textContent = '🎤 Start Recording';
      recordBtn.style.background = '#333';
    }
  });

  // Live audio level visualization
  const levelBar = document.getElementById('audio-level-bar') as HTMLDivElement | null;
  const levelText = document.getElementById('audio-level-text') as HTMLDivElement | null;
  let lastLevelUpdate = 0;
  await listen('audio-level', (e) => {
    try {
      const now = performance.now();
      if (now - lastLevelUpdate < 30) return;
      lastLevelUpdate = now;
      const payload = (e.payload as any) || {};
      const peak = Number(payload.peak) || 0;
      const db = Number(payload.db);
      const percent = Math.max(0, Math.min(100, Math.round(peak * 100)));
      if (levelBar) {
        levelBar.style.width = `${percent}%`;
      }
      if (levelText) {
        const dbText = isFinite(db) ? `${db.toFixed(1)} dB` : '-inf dB';
        levelText.textContent = dbText;
      }
    } catch {}
  });

  await listen('transcription-start', () => {
    console.log('⏳ Transcribing...');
    addLog('⏳ Transcribing audio...');
    updateStatus('Transcribing...');
  });

  await listen('transcription', (e) => {
    const text = (e.payload as any)?.text;
    console.log('📝 Transcription:', text);
    addLog(
      `📝 Transcribed: "${text.substring(0, 50)}${
        text.length > 50 ? '...' : ''
      }"`
    );
    updateStatus('Ready');
    document.body.style.borderTop = '5px solid #22E522';
    // Show notification
    if (text) {
      const notification = document.createElement('div');
      notification.style.cssText = `
				position: fixed;
				top: 20px;
				right: 20px;
				background: #333;
				color: white;
				padding: 15px;
				border-radius: 8px;
				max-width: 300px;
				z-index: 9999;
				box-shadow: 0 4px 12px rgba(0,0,0,0.3);
			`;
      notification.textContent = `Transcribed: ${text.substring(0, 100)}${
        text.length > 100 ? '...' : ''
      }`;
      document.body.appendChild(notification);
      setTimeout(() => notification.remove(), 3000);
    }
  });

  await listen('transcription-profile', (e) => {
    try {
      const profile = (e.payload as any) || {};
      const serverProfile = profile.server || {};
      // Some builds nest server info under server.server
      const innerServer = (serverProfile as any).server || serverProfile;
      const backend = (innerServer as any).backend || {};
      const whisper = (serverProfile as any).whisper || profile.whisper || {};
      if (backend && Object.keys(backend).length > 0) {
        const path = backend.ggml_metal_path_resources || 'unset';
        const metallib = backend.metallib_present ? 'present' : 'missing';
        const mode = backend.likely_using_metal ? 'Metal (GPU)' : 'CPU (fallback?)';
        addLog(`⚙️ Backend: ${mode} | metallib: ${metallib} | resources: ${path}`);
      }
      if (whisper && typeof whisper === 'object') {
        const inf = whisper.inference_ms;
        const total = whisper.total_ms;
        if (typeof inf === 'number' || typeof total === 'number') {
          addLog(`⏱️ Whisper timings: inference=${inf ?? '?'} ms, total=${total ?? '?'} ms`);
        }
      }
    } catch (err) {
      console.error('Failed to display transcription profile:', err);
    }
  });

  // Show backend at startup if backend-status is emitted from Rust setup
  await listen('backend-status', (e) => {
    try {
      const b = (e.payload as any) || {};
      const path = b.ggml_metal_path_resources || 'unset';
      const metallib = b.metallib_present ? 'present' : 'missing';
      const mode = b.likely_using_metal ? 'Metal (GPU)' : 'CPU (fallback?)';
      addLog(`⚙️ Backend: ${mode} | metallib: ${metallib} | resources: ${path}`);
    } catch (err) {
      console.error('Failed to display backend status:', err);
    }
  });

  await listen('transcription-complete', () => {
    console.log('✅ Transcription complete and copied to clipboard!');
    addLog('✅ Text copied to clipboard!');
    updateStatus('Ready');
    setTimeout(() => {
      document.body.style.borderTop = 'none';
    }, 2000);
  });

  await listen('transcription-failed', () => {
    console.error('❌ Transcription failed!');
    addLog('❌ Transcription failed! Check console for details.');
    updateStatus('Error - Ready');
    document.body.style.borderTop = '5px solid #E52222';
    setTimeout(() => {
      document.body.style.borderTop = 'none';
    }, 3000);
  });

  // Initial ready message
  addLog('🎤 Commander initialized');

  // Models UI
  const modelsContainer = document.getElementById('models-container');
  const modelsStatus = document.getElementById('models-status');

  const renderModels = async () => {
    if (!modelsContainer) return;
    try {
      const status = (await invoke('get_models_status')) as any;
      const selectedId = status.selected_id as string | null;
      const items = status.available as Array<any>;
      modelsContainer.innerHTML = '';
      const select = document.createElement('select');
      select.style.padding = '8px';
      select.style.fontSize = '1em';
      const placeholder = document.createElement('option');
      placeholder.value = '';
      placeholder.textContent = 'Select a model…';
      select.appendChild(placeholder);
      items.forEach((m) => {
        const opt = document.createElement('option');
        opt.value = m.id;
        const installedMark = m.installed ? ' ✅' : '';
        opt.textContent = `${m.name}${installedMark}`;
        if (selectedId && m.id === selectedId) opt.selected = true;
        select.appendChild(opt);
      });
      const actionBtn = document.createElement('button');
      actionBtn.textContent = 'Download & Use';
      actionBtn.style.marginLeft = '8px';
      actionBtn.onclick = async () => {
        const id = select.value;
        if (!id) {
          modelsStatus!.textContent = 'Please choose a model';
          return;
        }
        try {
          modelsStatus!.textContent = 'Starting download...';
          await invoke('download_model', { id });
        } catch (e) {
          modelsStatus!.textContent = `Failed to start: ${e}`;
        }
      };
      modelsContainer.appendChild(select);
      modelsContainer.appendChild(actionBtn);
    } catch (e) {
      console.error('Failed to load models status', e);
    }
  };

  await renderModels();

  await listen('model-download-start', (e) => {
    try {
      const p = e.payload as any;
      const total = p?.total_bytes ? ` (${Math.round(p.total_bytes / 1024 / 1024)} MB)` : '';
      modelsStatus!.textContent = `Downloading model...${total}`;
    } catch {}
  });
  await listen('model-download-progress', (e) => {
    try {
      const p = e.payload as any;
      const rec = Math.round((p.received_bytes || 0) / 1024 / 1024);
      const tot = p.total_bytes ? Math.round(p.total_bytes / 1024 / 1024) : null;
      modelsStatus!.textContent = `Downloading: ${rec}${tot ? ` / ${tot}` : ''} MB`;
    } catch {}
  });
  await listen('model-download-complete', async () => {
    try {
      modelsStatus!.textContent = `Download complete. File saved.`;
      await renderModels();
    } catch {}
  });
  await listen('model-download-error', (e) => {
    try {
      modelsStatus!.textContent = `Download failed: ${e.payload}`;
    } catch {}
  });

  // Block recording when no model selected
  await listen('no-model-selected', () => {
    addLog('⚠️ No model selected. Please choose a model in the Models section.');
    const statusEl = document.getElementById('status');
    if (statusEl) statusEl.textContent = 'Please select a model first';
  });

  // Shortcut configuration
  const shortcutInput = document.getElementById(
    'shortcut-input'
  ) as HTMLInputElement;
  const saveShortcutBtn = document.getElementById(
    'save-shortcut-btn'
  ) as HTMLButtonElement;
  const shortcutStatus = document.getElementById(
    'shortcut-status'
  ) as HTMLElement;

  let currentShortcut = { modifiers: [] as string[], key: '' };
  const languageSelect = document.getElementById('language-select') as HTMLSelectElement;
  const saveLanguageBtn = document.getElementById('save-language-btn') as HTMLButtonElement;
  const languageStatus = document.getElementById('language-status') as HTMLElement;
  const audioDeviceSelect = document.getElementById('audio-device-select') as HTMLSelectElement | null;
  const saveAudioDeviceBtn = document.getElementById('save-audio-device-btn') as HTMLButtonElement | null;
  const audioDeviceStatus = document.getElementById('audio-device-status') as HTMLElement | null;
  const promptTextarea = document.getElementById('prompt-textarea') as HTMLTextAreaElement;
  const savePromptBtn = document.getElementById('save-prompt-btn') as HTMLButtonElement;
  const promptStatus = document.getElementById('prompt-status') as HTMLElement;
  const autoPasteCheckbox = document.getElementById('auto-paste-checkbox') as HTMLInputElement | null;
  const holdToRecordCheckbox = document.getElementById('hold-to-record-checkbox') as HTMLInputElement | null;

  // Load current shortcut
  try {
    const saved = (await invoke('get_current_shortcut')) as any;
    if (saved) {
      const modifierText = saved.modifiers
        .map((m: string) =>
          m.toLowerCase() === 'super'
            ? isMac
              ? '⌘'
              : 'Win'
            : m.toLowerCase() === 'control'
            ? 'Ctrl'
            : m.toLowerCase() === 'shift'
            ? '⇧'
            : m.toLowerCase() === 'alt'
            ? isMac
              ? '⌥'
              : 'Alt'
            : m
        )
        .join('+');
      const fullShortcut = modifierText + (modifierText ? '+' : '') + saved.key;
      shortcutInput.value = fullShortcut;
      addLog(`Current shortcut: ${fullShortcut}`);
    }
  } catch (e) {
    console.error('Failed to load shortcut config:', e);
    addLog('Using default shortcut: ⌘⇧F9 (or Cmd+Shift+F9)');
  }
  // Load current language
  try {
    const lang = (await invoke('get_default_language')) as string | null;
    if (languageSelect) {
      languageSelect.value = lang ?? '';
    }
    if (lang) addLog(`Default language: ${lang}`);
    else addLog('Default language: Auto-detect');
  } catch (e) {
    console.error('Failed to load language config:', e);
  }

  // Load audio devices and current selection
  try {
    if (audioDeviceSelect) {
      const devices = (await invoke('list_audio_input_devices')) as string[];
      const selected = (await invoke('get_selected_audio_input_device')) as string | null;
      audioDeviceSelect.innerHTML = '';
      const sys = document.createElement('option');
      sys.value = '';
      sys.textContent = 'System default';
      audioDeviceSelect.appendChild(sys);
      devices.forEach((name) => {
        const opt = document.createElement('option');
        opt.value = name;
        opt.textContent = name;
        if (selected && name === selected) opt.selected = true;
        audioDeviceSelect.appendChild(opt);
      });
    }
  } catch (e) {
    console.error('Failed to load audio devices:', e);
  }

  // Load current initial prompt
  try {
    const prompt = (await invoke('get_default_prompt')) as string | null;
    if (promptTextarea) {
      promptTextarea.value = prompt ?? '';
    }
    if (prompt && prompt.length > 0) addLog('Initial prompt loaded.');
  } catch (e) {
    console.error('Failed to load initial prompt:', e);
  }

  // Load auto-paste setting
  try {
    if (autoPasteCheckbox) {
      const enabled = (await invoke('get_auto_paste_enabled')) as boolean;
      autoPasteCheckbox.checked = !!enabled;
    }
  } catch (e) {
    console.error('Failed to load auto-paste setting:', e);
  }

  // Load hold-to-record setting
  try {
    if (holdToRecordCheckbox) {
      const enabled = (await invoke('get_hold_to_record_enabled')) as boolean;
      holdToRecordCheckbox.checked = !!enabled;
    }
  } catch (e) {
    console.error('Failed to load hold-to-record setting:', e);
  }

  // Capture shortcut
  if (shortcutInput) {
    shortcutInput.addEventListener('focus', () => {
      shortcutInput.value = 'Press keys...';
      currentShortcut = { modifiers: [], key: '' };
    });

    shortcutInput.addEventListener('keydown', (e) => {
      e.preventDefault();
      e.stopPropagation();

      const modifiers = [];
      if (e.metaKey || e.ctrlKey) modifiers.push(isMac ? 'Super' : 'Control');
      if (e.shiftKey) modifiers.push('Shift');
      if (e.altKey) modifiers.push('Alt');

      // Get the key name
      let key = e.key.toUpperCase();
      if (key === ' ') key = 'SPACE';
      else if (key === 'ENTER') key = 'ENTER';
      else if (key === 'TAB') key = 'TAB';
      else if (key === 'ESCAPE') key = 'ESCAPE';
      else if (e.code.startsWith('F') && e.code.length <= 3) key = e.code;
      else if (key.length > 1) {
        // Skip modifier keys alone
        if (['SHIFT', 'CONTROL', 'ALT', 'META'].includes(key)) return;
        key = e.code.replace('Key', '');
      }

      currentShortcut = { modifiers, key };

      // Display the shortcut
      const modifierText = modifiers
        .map((m) =>
          m === 'Super'
            ? isMac
              ? '⌘'
              : 'Win'
            : m === 'Control'
            ? 'Ctrl'
            : m === 'Shift'
            ? '⇧'
            : m === 'Alt'
            ? isMac
              ? '⌥'
              : 'Alt'
            : m
        )
        .join('+');

      shortcutInput.value = modifierText + (modifierText ? '+' : '') + key;
      saveShortcutBtn.disabled = false;
    });

    shortcutInput.addEventListener('blur', () => {
      if (shortcutInput.value === 'Press keys...') {
        shortcutInput.value = '';
      }
    });
  }

  // Save shortcut
  if (saveShortcutBtn) {
    saveShortcutBtn.addEventListener('click', async () => {
      if (!currentShortcut.key) return;

      try {
        await invoke('save_custom_shortcut', { config: currentShortcut });
        shortcutStatus.textContent =
          '✅ Shortcut saved! Restart the app to apply.';
        shortcutStatus.style.color = 'green';
        addLog('💾 Custom shortcut saved. Please restart the app.');
      } catch (e) {
        shortcutStatus.textContent = '❌ Failed to save shortcut: ' + e;
        shortcutStatus.style.color = 'red';
        console.error('Failed to save shortcut:', e);
      }
    });
  }

  // Save language
  if (saveLanguageBtn) {
    saveLanguageBtn.addEventListener('click', async () => {
      try {
        const value = languageSelect ? languageSelect.value : '';
        await invoke('save_default_language', { language: value || null });
        if (languageStatus) {
          languageStatus.textContent = '✅ Language saved!';
          languageStatus.style.color = 'green';
        }
        addLog('💾 Default language saved.');
      } catch (e) {
        if (languageStatus) {
          languageStatus.textContent = '❌ Failed to save language: ' + e;
          languageStatus.style.color = 'red';
        }
        console.error('Failed to save language:', e);
      }
    });
  }

  // Save prompt
  if (savePromptBtn) {
    savePromptBtn.addEventListener('click', async () => {
      try {
        const value = promptTextarea ? promptTextarea.value.trim() : '';
        await invoke('save_default_prompt', { prompt: value || null });
        if (promptStatus) {
          promptStatus.textContent = '✅ Prompt saved!';
          promptStatus.style.color = 'green';
        }
        addLog('💾 Initial prompt saved.');
      } catch (e) {
        if (promptStatus) {
          promptStatus.textContent = '❌ Failed to save prompt: ' + e;
          promptStatus.style.color = 'red';
        }
        console.error('Failed to save prompt:', e);
      }
    });
  }

  // Save audio device and apply immediately
  if (saveAudioDeviceBtn && audioDeviceSelect) {
    saveAudioDeviceBtn.addEventListener('click', async () => {
      try {
        const name = audioDeviceSelect.value || null;
        await invoke('save_selected_audio_input_device', { name });
        await invoke('apply_selected_audio_input_device');
        if (audioDeviceStatus) {
          audioDeviceStatus.textContent = '✅ Device applied!';
          audioDeviceStatus.style.color = 'green';
        }
        addLog('🔄 Audio input device applied.');
      } catch (e) {
        if (audioDeviceStatus) {
          audioDeviceStatus.textContent = '❌ Failed to apply device: ' + e;
          audioDeviceStatus.style.color = 'red';
        }
        console.error('Failed to apply audio device:', e);
      }
    });
  }

  // Toggle auto-paste
  if (autoPasteCheckbox) {
    autoPasteCheckbox.addEventListener('change', async () => {
      try {
        const enabled = !!autoPasteCheckbox.checked;
        await invoke('save_auto_paste_enabled', { enabled });
        addLog(enabled ? '💾 Auto-paste enabled.' : '💾 Auto-paste disabled.');
      } catch (e) {
        console.error('Failed to save auto-paste setting:', e);
      }
    });
  }

  // Toggle hold-to-record
  if (holdToRecordCheckbox) {
    holdToRecordCheckbox.addEventListener('change', async () => {
      try {
        const enabled = !!holdToRecordCheckbox.checked;
        await invoke('save_hold_to_record_enabled', { enabled });
        addLog(
          enabled
            ? '💾 Hold to record enabled.'
            : '💾 Hold to record disabled.'
        );
      } catch (e) {
        console.error('Failed to save hold-to-record setting:', e);
      }
    });
  }
});
