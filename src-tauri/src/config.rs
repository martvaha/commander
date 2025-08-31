use anyhow::{anyhow, Result};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct LanguageConfig {
    pub language: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct PromptConfig {
    pub prompt: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct AutoPasteConfig {
    pub enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct HoldToRecordConfig {
    pub enabled: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ShortcutConfig {
    pub modifiers: Vec<String>,
    pub key: String,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            modifiers: vec!["Super".to_string(), "Shift".to_string()],
            key: "F9".to_string(),
        }
    }
}

impl ShortcutConfig {
    pub fn to_shortcut(&self) -> Result<Shortcut> {
        let mut modifier_flags = Modifiers::empty();
        for modifier in &self.modifiers {
            match modifier.to_lowercase().as_str() {
                "cmd" | "super" | "meta" => modifier_flags |= Modifiers::SUPER,
                "ctrl" | "control" => modifier_flags |= Modifiers::CONTROL,
                "alt" | "option" => modifier_flags |= Modifiers::ALT,
                "shift" => modifier_flags |= Modifiers::SHIFT,
                _ => {}
            }
        }
        let code = match self.key.to_uppercase().as_str() {
            "F1" => Code::F1,
            "F2" => Code::F2,
            "F3" => Code::F3,
            "F4" => Code::F4,
            "F5" => Code::F5,
            "F6" => Code::F6,
            "F7" => Code::F7,
            "F8" => Code::F8,
            "F9" => Code::F9,
            "F10" => Code::F10,
            "F11" => Code::F11,
            "F12" => Code::F12,
            "F13" => Code::F13,
            "F14" => Code::F14,
            "F15" => Code::F15,
            "F16" => Code::F16,
            "F17" => Code::F17,
            "F18" => Code::F18,
            "F19" => Code::F19,
            "F20" => Code::F20,
            "F21" => Code::F21,
            "F22" => Code::F22,
            "A" => Code::KeyA,
            "B" => Code::KeyB,
            "C" => Code::KeyC,
            "D" => Code::KeyD,
            "E" => Code::KeyE,
            "F" => Code::KeyF,
            "G" => Code::KeyG,
            "H" => Code::KeyH,
            "I" => Code::KeyI,
            "J" => Code::KeyJ,
            "K" => Code::KeyK,
            "L" => Code::KeyL,
            "M" => Code::KeyM,
            "N" => Code::KeyN,
            "O" => Code::KeyO,
            "P" => Code::KeyP,
            "Q" => Code::KeyQ,
            "R" => Code::KeyR,
            "S" => Code::KeyS,
            "T" => Code::KeyT,
            "U" => Code::KeyU,
            "V" => Code::KeyV,
            "W" => Code::KeyW,
            "X" => Code::KeyX,
            "Y" => Code::KeyY,
            "Z" => Code::KeyZ,
            "SPACE" => Code::Space,
            "ENTER" => Code::Enter,
            "TAB" => Code::Tab,
            "ESCAPE" | "ESC" => Code::Escape,
            _ => return Err(anyhow!("Unsupported key: {}", self.key)),
        };
        Ok(Shortcut::new(
            if modifier_flags.is_empty() { None } else { Some(modifier_flags) },
            code,
        ))
    }
}

pub fn is_auto_paste_enabled(app: &AppHandle) -> bool {
    if let Ok(config_dir) = app.path().app_config_dir() {
        let path = config_dir.join("auto_paste.json");
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<AutoPasteConfig>(&content) {
                    return cfg.enabled;
                }
            }
        }
    }
    false
}

pub fn is_hold_to_record_enabled(app: &AppHandle) -> bool {
    if let Ok(config_dir) = app.path().app_config_dir() {
        let path = config_dir.join("hold_to_record.json");
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<HoldToRecordConfig>(&content) {
                    return cfg.enabled;
                }
            }
        }
    }
    false
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct ModelConfig {
    pub selected_id: Option<String>,
}

pub fn read_model_config(app: &AppHandle) -> Option<ModelConfig> {
    if let Ok(dir) = app.path().app_config_dir() {
        let path = dir.join("model.json");
        if path.exists() {
            if let Ok(s) = std::fs::read_to_string(path) {
                return serde_json::from_str::<ModelConfig>(&s).ok();
            }
        }
    }
    None
}

pub fn write_model_config(app: &AppHandle, cfg: &ModelConfig) -> anyhow::Result<()> {
    let dir = app.path().app_config_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("model.json");
    let content = serde_json::to_string_pretty(cfg)?;
    std::fs::write(path, content)?;
    Ok(())
}


#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct AudioInputConfig {
    pub selected_device_name: Option<String>,
}

pub fn read_audio_input_config(app: &AppHandle) -> Option<AudioInputConfig> {
    if let Ok(dir) = app.path().app_config_dir() {
        let path = dir.join("audio_input.json");
        if path.exists() {
            if let Ok(s) = std::fs::read_to_string(path) {
                return serde_json::from_str::<AudioInputConfig>(&s).ok();
            }
        }
    }
    None
}

pub fn write_audio_input_config(app: &AppHandle, cfg: &AudioInputConfig) -> anyhow::Result<()> {
    let dir = app.path().app_config_dir()?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("audio_input.json");
    let content = serde_json::to_string_pretty(cfg)?;
    std::fs::write(path, content)?;
    Ok(())
}


