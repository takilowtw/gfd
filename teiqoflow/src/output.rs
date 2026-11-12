//! Inserting text into whatever window currently has keyboard focus.

use crate::config::InsertMethod;
use std::mem::size_of;
use std::thread::sleep;
use std::time::Duration;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, SendInput, VK_CONTROL, VK_RETURN,
};

const VK_V: u16 = 0x56;

fn key(vk: u16, scan: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn send(inputs: &[INPUT]) {
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
}

fn type_unicode(text: &str) {
    for chunk in text.encode_utf16().collect::<Vec<_>>().chunks(32) {
        let inputs: Vec<INPUT> = chunk
            .iter()
            .flat_map(|&u| [key(0, u, KEYEVENTF_UNICODE), key(0, u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)])
            .collect();
        send(&inputs);
        sleep(Duration::from_millis(4));
    }
}

pub fn insert(text: &str, method: InsertMethod, restore_clipboard: bool, press_enter: bool) {
    match method {
        InsertMethod::Type => type_unicode(text),
        InsertMethod::Paste => {
            let mut clipboard = arboard::Clipboard::new().ok();
            let previous = if restore_clipboard { clipboard.as_mut().and_then(|c| c.get_text().ok()) } else { None };
            let pasted = clipboard.as_mut().map(|c| c.set_text(text.to_string()).is_ok()).unwrap_or(false);
            if !pasted {
                type_unicode(text);
            } else {
                sleep(Duration::from_millis(30));
                send(&[key(VK_CONTROL, 0, 0), key(VK_V, 0, 0), key(VK_V, 0, KEYEVENTF_KEYUP), key(VK_CONTROL, 0, KEYEVENTF_KEYUP)]);
                if let (Some(prev), Some(c)) = (previous, clipboard.as_mut()) {
                    // The target app reads the clipboard asynchronously; give it time first.
                    sleep(Duration::from_millis(350));
                    let _ = c.set_text(prev);
                }
            }
        }
    }
    if press_enter {
        sleep(Duration::from_millis(40));
        send(&[key(VK_RETURN, 0, 0), key(VK_RETURN, 0, KEYEVENTF_KEYUP)]);
    }
}
