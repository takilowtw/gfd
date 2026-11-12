//! Push-to-talk detection by polling the global key state. Polling (instead of a hook)
//! never delays other apps' input and needs no special permissions.

use crate::state::{Capture, Shared, lock};
use crossbeam_channel::Sender;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

pub enum KeyEvent {
    Press,
    Release,
    Cancel,
}

const VK_ESCAPE: u32 = 0x1B;

/// Unassigned key code sent while Win is held so that releasing Win doesn't open the Start menu.
const VK_DUMMY: u16 = 0xE8;

/// Left/right/middle mouse, the generic Shift/Ctrl/Alt codes (which alias their L/R variants)
/// and our own dummy key.
fn ignored(vk: u32) -> bool {
    matches!(vk, 0x01 | 0x02 | 0x04 | 0x10 | 0x11 | 0x12 | 0xE8)
}

fn suppress_start_menu() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput};
    let key = |flags| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_DUMMY, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    };
    let inputs = [key(0), key(KEYEVENTF_KEYUP)];
    unsafe { SendInput(2, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
}

fn is_mouse(vk: u32) -> bool {
    matches!(vk, 0x01..=0x06)
}

fn is_modifier(vk: u32) -> bool {
    matches!(vk, 0xA0..=0xA5 | 0x5B | 0x5C)
}

fn snapshot() -> [bool; 256] {
    let mut down = [false; 256];
    for (vk, d) in down.iter_mut().enumerate().skip(1) {
        *d = unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000 != 0;
    }
    down
}

pub fn spawn(shared: Arc<Shared>, tx: Sender<KeyEvent>) {
    std::thread::Builder::new()
        .name("hotkey".into())
        .spawn(move || {
            let mut prev = [false; 256];
            let mut active = false;
            let mut cancelled = false;
            let mut captured: Vec<u32> = Vec::new();
            loop {
                std::thread::sleep(Duration::from_millis(8));
                let down = snapshot();

                // Recording a new shortcut from the settings page.
                let capturing = *lock(&shared.capture) == Capture::Waiting;
                if capturing {
                    if active {
                        active = false;
                        let _ = tx.send(KeyEvent::Cancel);
                    }
                    for vk in 1..256u32 {
                        if down[vk as usize] && !prev[vk as usize] && !ignored(vk) && !captured.contains(&vk) {
                            captured.push(vk);
                        }
                    }
                    if !captured.is_empty() && captured.iter().all(|&vk| !down[vk as usize]) {
                        let mut keys = std::mem::take(&mut captured);
                        if keys == [VK_ESCAPE] {
                            keys.clear();
                        }
                        keys.sort_by_key(|&vk| (!is_modifier(vk), vk));
                        *lock(&shared.capture) = Capture::Finished(keys);
                        cancelled = true; // don't fire until the new combo is released
                        shared.repaint();
                    }
                    prev = down;
                    continue;
                }
                captured.clear();

                // Our own synthetic Ctrl+V / typing must not look like the shortcut.
                if shared.suppress_hotkey.load(Ordering::Relaxed) {
                    active = false;
                    cancelled = true;
                    prev = down;
                    continue;
                }

                let combo = lock(&shared.cfg).hotkey.clone();
                if combo.is_empty() {
                    prev = down;
                    continue;
                }
                let held = combo.iter().all(|&vk| down[vk as usize]);

                if active {
                    let other_pressed = (1..256u32).any(|vk| {
                        down[vk as usize] && !prev[vk as usize] && !ignored(vk) && !is_mouse(vk) && !combo.contains(&vk)
                    });
                    if other_pressed {
                        // Something like Ctrl+C: the user is using a shortcut, not dictating.
                        active = false;
                        cancelled = true;
                        let _ = tx.send(KeyEvent::Cancel);
                    } else if !held {
                        active = false;
                        let _ = tx.send(KeyEvent::Release);
                    }
                } else if cancelled {
                    if combo.iter().all(|&vk| !down[vk as usize]) {
                        cancelled = false;
                    }
                } else if held {
                    // Only start when the last key of the combo goes down now, so holding Ctrl and
                    // later pressing another key doesn't start a recording.
                    let fresh = combo.iter().any(|&vk| !prev[vk as usize]);
                    let extra = (1..256u32).any(|vk| down[vk as usize] && !ignored(vk) && !is_mouse(vk) && !combo.contains(&vk));
                    if fresh && !extra {
                        if combo.iter().any(|&vk| vk == 0x5B || vk == 0x5C) {
                            suppress_start_menu();
                        }
                        active = true;
                        let _ = tx.send(KeyEvent::Press);
                    }
                }
                prev = down;
            }
        })
        .expect("spawn hotkey thread");
}

pub fn vk_name(vk: u32) -> String {
    let s = match vk {
        0x05 => "Mouse 4",
        0x06 => "Mouse 5",
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0D => "Enter",
        0x13 => "Pause",
        0x14 => "Caps Lock",
        0x1B => "Esc",
        0x20 => "Space",
        0x21 => "Page Up",
        0x22 => "Page Down",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "←",
        0x26 => "↑",
        0x27 => "→",
        0x28 => "↓",
        0x2C => "Print Screen",
        0x2D => "Insert",
        0x2E => "Delete",
        0x30..=0x39 | 0x41..=0x5A => return char::from_u32(vk).unwrap().to_string(),
        0x5B => "Left Win",
        0x5C => "Right Win",
        0x5D => "Menu",
        0x60..=0x69 => return format!("Num {}", vk - 0x60),
        0x6A => "Num *",
        0x6B => "Num +",
        0x6D => "Num -",
        0x6E => "Num .",
        0x6F => "Num /",
        0x70..=0x87 => return format!("F{}", vk - 0x6F),
        0x90 => "Num Lock",
        0x91 => "Scroll Lock",
        0xA0 => "Left Shift",
        0xA1 => "Right Shift",
        0xA2 => "Left Ctrl",
        0xA3 => "Right Ctrl",
        0xA4 => "Left Alt",
        0xA5 => "Right Alt",
        0xBA => ";",
        0xBB => "=",
        0xBC => ",",
        0xBD => "-",
        0xBE => ".",
        0xBF => "/",
        0xC0 => "`",
        0xDB => "[",
        0xDC => "\\",
        0xDD => "]",
        0xDE => "'",
        _ => return format!("Key {vk:#04X}"),
    };
    s.to_string()
}

pub fn combo_name(keys: &[u32]) -> String {
    if keys.is_empty() {
        return "Not set".into();
    }
    keys.iter().map(|&k| vk_name(k)).collect::<Vec<_>>().join(" + ")
}
