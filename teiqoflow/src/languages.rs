/// (display name, code). Codes are Google Translate codes; see `whisper_code` for Whisper.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("English", "en"),
    ("Русский", "ru"),
    ("Українська", "uk"),
    ("Deutsch", "de"),
    ("Français", "fr"),
    ("Español", "es"),
    ("Italiano", "it"),
    ("Português", "pt"),
    ("Polski", "pl"),
    ("Čeština", "cs"),
    ("Nederlands", "nl"),
    ("Svenska", "sv"),
    ("Suomi", "fi"),
    ("Türkçe", "tr"),
    ("Ελληνικά", "el"),
    ("Қазақша", "kk"),
    ("Беларуская", "be"),
    ("中文 (简体)", "zh-CN"),
    ("日本語", "ja"),
    ("한국어", "ko"),
    ("Tiếng Việt", "vi"),
    ("Bahasa Indonesia", "id"),
    ("हिन्दी", "hi"),
];

pub fn name_for(code: &str) -> &str {
    if code == "auto" {
        return "Auto-detect";
    }
    LANGUAGES.iter().find(|(_, c)| *c == code).map(|(n, _)| *n).unwrap_or(code)
}

pub fn whisper_code(code: &str) -> &str {
    match code {
        "zh-CN" => "zh",
        other => other,
    }
}

pub fn same_language(a: &str, b: &str) -> bool {
    let base = |s: &str| s.split('-').next().unwrap_or(s).to_ascii_lowercase();
    base(a) == base(b)
}
