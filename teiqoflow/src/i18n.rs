//! Interface language. Strings are written in English in the code and looked up here.

use std::sync::atomic::{AtomicBool, Ordering};

static RUSSIAN: AtomicBool = AtomicBool::new(true);

pub fn set_language(code: &str) {
    RUSSIAN.store(code == "ru", Ordering::Relaxed);
}

pub fn is_russian() -> bool {
    RUSSIAN.load(Ordering::Relaxed)
}

/// Translated and lowercased: the interface is styled all-lowercase.
pub fn t(en: &'static str) -> &'static str {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<(bool, &'static str), &'static str>>> = OnceLock::new();
    let ru = is_russian();
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    cache.entry((ru, en)).or_insert_with(|| {
        let s = if ru { RU.iter().find(|(k, _)| *k == en).map(|(_, v)| *v).unwrap_or(en) } else { en };
        // Leaked once per distinct string; the set is small and fixed.
        Box::leak(s.to_lowercase().into_boxed_str())
    })
}

/// `t` for templates containing `{}` placeholders, filled in order.
pub fn tf(en: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::new();
    let mut parts = t(en).split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for (i, part) in parts.enumerate() {
        if let Some(a) = args.get(i) {
            out.push_str(&a.to_string());
        }
        out.push_str(part);
    }
    out
}

const RU: &[(&str, &str)] = &[
    // Navigation
    ("Home", "Главная"),
    ("History", "История"),
    ("Settings", "Настройки"),
    ("Engine", "Движок"),
    // Home
    ("Push to talk", "Зажми и говори"),
    ("Hold", "Зажми"),
    ("and speak", "и говори"),
    (
        "Release the key — your words are transcribed, translated and typed into whatever chat or text field your cursor is in.",
        "Отпусти клавиши — речь распознается, переведётся и напечатается в чат или поле, где стоит курсор.",
    ),
    ("Languages", "Языки"),
    ("I speak", "Я говорю на"),
    ("Type it in", "Печатать на"),
    ("Auto-detect", "Автоопределение"),
    ("Swap languages", "Поменять языки"),
    ("Translate", "Переводить"),
    ("Off = type exactly what you said", "Выкл. — печатать сказанное без перевода"),
    ("Words dictated", "Слов надиктовано"),
    ("Dictations", "Диктовок"),
    ("Last dictation", "Последняя диктовка"),
    ("Nothing yet — hold the key and say something.", "Пока пусто — зажми клавиши и скажи что-нибудь."),
    // History
    ("Clear", "Очистить"),
    ("Copy", "Копировать"),
    ("Your dictations will appear here.", "Здесь появятся твои диктовки."),
    // Settings
    ("Interface", "Интерфейс"),
    ("Interface language", "Язык интерфейса"),
    ("Shortcut & input", "Клавиши и ввод"),
    ("Push-to-talk key", "Клавиши записи"),
    ("Hold to record, release to type. Esc cancels.", "Удерживай для записи, отпусти для ввода. Esc — отмена."),
    ("Change", "Изменить"),
    ("Press keys…", "Нажми клавиши…"),
    ("Microphone", "Микрофон"),
    ("System default", "Системный по умолчанию"),
    ("Refresh list", "Обновить список"),
    ("Recognition", "Распознавание"),
    ("Speech model", "Модель распознавания"),
    ("Larger = more accurate but slower.", "Больше — точнее, но медленнее."),
    ("Larger = more accurate but slower. Downloads {} MB on first use.", "Больше — точнее, но медленнее. При первом запуске скачает {} МБ."),
    ("GigaAM recognises Russian only.", "GigaAM распознаёт только русскую речь."),
    ("Output", "Вывод"),
    ("Insert method", "Способ вставки"),
    ("Use typing if an app blocks pasting", "Набор подходит, если приложение запрещает вставку"),
    ("Paste (fast, recommended)", "Вставка (быстро, рекомендуется)"),
    ("Simulate typing", "Имитация набора"),
    ("Lowercase", "Строчные буквы"),
    ("Type everything in lowercase", "Печатать весь текст маленькими буквами"),
    ("Send automatically", "Отправлять сразу"),
    ("Press Enter after inserting the text", "Нажимать Enter после вставки"),
    ("Keep my clipboard", "Сохранять буфер обмена"),
    ("Restore the clipboard after pasting", "Возвращать прежнее содержимое после вставки"),
    ("Sounds", "Звуки"),
    ("Soft click when recording starts and stops", "Тихий щелчок в начале и конце записи"),
    // Models
    ("Tiny — fastest", "Tiny — самая быстрая"),
    ("Base — fast", "Base — быстрая"),
    ("Small — balanced", "Small — баланс"),
    ("Medium — accurate", "Medium — точная"),
    ("Large v3 Turbo — best", "Large v3 Turbo — лучшая"),
    ("GigaAM v3 — Russian only, best for Russian", "GigaAM v3 — только русский, лучшая для русского"),
    // Status and overlay
    ("Starting…", "Запуск…"),
    ("Loading {} model…", "Загрузка модели {}…"),
    ("Downloading {} model… {}%", "Скачивание модели {}… {}%"),
    ("Ready · {} model · {}", "Готово · модель {} · {}"),
    ("Transcribing", "Распознаю"),
    ("Translating", "Перевожу"),
    ("Didn't catch that — try again", "Не расслышал — попробуй ещё раз"),
    ("Didn't hear anything — check your microphone", "Тишина — проверь микрофон"),
    ("Speech model is still loading…", "Модель ещё загружается…"),
    ("Speech model is not loaded", "Модель не загружена"),
    ("Translation failed: {}", "Ошибка перевода: {}"),
    ("Microphone: {}", "Микрофон: {}"),
    ("No microphone found", "Микрофон не найден"),
    // Tray
    ("Open teiqoflow", "Открыть teiqoflow"),
    ("Quit", "Выход"),
    ("MB", "МБ"),
];
