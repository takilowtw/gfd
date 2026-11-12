use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const VK_LCONTROL: u32 = 0xA2;
pub const VK_LWIN: u32 = 0x5B;
/// Bumped when defaults change in a way existing users should receive.
const CONFIG_VERSION: u32 = 2;

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
    /// Missing in files written before versioning, so it deserialises as 0.
    #[serde(default)]
    pub version: u32,
    pub ui_lang: String,
    /// Virtual-key codes that must all be held.
    pub hotkey: Vec<u32>,
    pub source_lang: String,
    pub target_lang: String,
    pub translate: bool,
    pub model: String,
    pub microphone: Option<String>,
    pub insert_method: InsertMethod,
    pub lowercase: bool,
    pub auto_enter: bool,
    pub restore_clipboard: bool,
    pub sounds: bool,
    /// Id from `style::ACCENTS`.
    pub accent: String,
    pub light_theme: bool,
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
            version: CONFIG_VERSION,
            ui_lang: "ru".into(),
            hotkey: vec![VK_LCONTROL, VK_LWIN],
            source_lang: "auto".into(),
            target_lang: "en".into(),
            translate: true,
            model: "small".into(),
            microphone: None,
            insert_method: InsertMethod::Paste,
            lowercase: true,
            auto_enter: false,
            restore_clipboard: true,
            sounds: true,
            accent: "blue".into(),
            light_theme: false,
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
    fn migrate(mut self) -> Self {
        if self.version < 2 {
            // v2: Ctrl+Win push-to-talk and lowercase output became the defaults.
            self.hotkey = vec![VK_LCONTROL, VK_LWIN];
            self.lowercase = true;
            self.version = 2;
        }
        self
    }

    pub fn load() -> Self {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
            .map(Config::migrate)
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
