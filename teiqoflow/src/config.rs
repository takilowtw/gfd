use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const VK_RCONTROL: u32 = 0xA3;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryItem {
    pub time: String,
    pub original: String,
    pub result: String,
    pub src: String,
    pub dst: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Virtual-key codes that must all be held.
    pub hotkey: Vec<u32>,
    pub source_lang: String,
    pub target_lang: String,
    pub translate: bool,
    pub model: String,
    pub microphone: Option<String>,
    pub insert_method: InsertMethod,
    pub auto_enter: bool,
    pub restore_clipboard: bool,
    pub sounds: bool,
    pub history: Vec<HistoryItem>,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum InsertMethod {
    Paste,
    Type,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: vec![VK_RCONTROL],
            source_lang: "auto".into(),
            target_lang: "en".into(),
            translate: true,
            model: "small".into(),
            microphone: None,
            insert_method: InsertMethod::Paste,
            auto_enter: false,
            restore_clipboard: true,
            sounds: true,
            history: Vec::new(),
        }
    }
}

pub fn data_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("teiqoflow")
}

fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(data_dir());
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let tmp = config_path().with_extension("tmp");
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(tmp, config_path());
            }
        }
    }
}
