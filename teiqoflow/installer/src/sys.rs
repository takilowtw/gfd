//! Windows integration for the installer: files, shortcuts, registry, processes.

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_DWORD, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW,
    RegDeleteTreeW, RegSetValueExW,
};

pub const APP: &str = "teiqoflow";
pub const APP_EXE: &str = "teiqoflow.exe";
pub const UNINSTALLER: &str = "uninstall.exe";
const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\teiqoflow";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn default_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    Path::new(&base).join("Programs").join(APP)
}

pub fn data_dir() -> PathBuf {
    Path::new(&std::env::var("APPDATA").unwrap_or_else(|_| ".".into())).join(APP)
}

pub fn system_is_russian() -> bool {
    let lang = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    lang & 0x3FF == 0x19
}

fn hidden(cmd: &mut Command) -> &mut Command {
    cmd.creation_flags(CREATE_NO_WINDOW)
}

pub fn close_running_app() {
    let _ = hidden(Command::new("taskkill").args(["/F", "/IM", APP_EXE])).output();
    std::thread::sleep(std::time::Duration::from_millis(400));
}

/// Decompresses one payload file to `dest`, reporting uncompressed bytes written.
pub fn unpack(data: &[u8], dest: &Path, mut progress: impl FnMut(u64)) -> Result<(), String> {
    let tmp = dest.with_extension("new");
    let mut out = std::fs::File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let mut dec = flate2::read::DeflateDecoder::new(data);
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = dec.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut out, &buf[..n]).map_err(|e| e.to_string())?;
        progress(n as u64);
    }
    drop(out);
    // A running copy may still hold the old file for a moment after being closed.
    for attempt in 0..10 {
        match std::fs::rename(&tmp, dest) {
            Ok(()) => return Ok(()),
            Err(e) if attempt == 9 => return Err(format!("{}: {e}", dest.display())),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(300)),
        }
    }
    Ok(())
}

fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn powershell(script: &str) -> Result<(), String> {
    let out = hidden(Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]))
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// Start menu shortcut always; desktop shortcut created or removed according to `desktop`.
pub fn write_shortcuts(dir: &Path, desktop: bool) -> Result<(), String> {
    let exe = dir.join(APP_EXE);
    let script = format!(
        "$s = New-Object -ComObject WScript.Shell; $t = {t}; $w = {w};\
         function mk($p) {{ $l = $s.CreateShortcut($p); $l.TargetPath = $t; $l.WorkingDirectory = $w; $l.IconLocation = \"$t,0\"; $l.Description = 'push-to-talk dictation with translation'; $l.Save() }};\
         mk ([Environment]::GetFolderPath('Programs') + '\\{APP}.lnk');\
         $d = [Environment]::GetFolderPath('Desktop') + '\\{APP}.lnk';\
         if (${desktop}) {{ mk $d }} else {{ Remove-Item -LiteralPath $d -ErrorAction SilentlyContinue }}",
        t = ps_quote(&exe.to_string_lossy()),
        w = ps_quote(&dir.to_string_lossy()),
    );
    powershell(&script)
}

pub fn remove_shortcuts() {
    let _ = powershell(&format!(
        "foreach ($f in 'Programs','Desktop') {{ Remove-Item -LiteralPath ([Environment]::GetFolderPath($f) + '\\{APP}.lnk') -ErrorAction SilentlyContinue }}"
    ));
}

fn with_key(path: &str, f: impl FnOnce(HKEY) -> Result<(), String>) -> Result<(), String> {
    let mut key: HKEY = std::ptr::null_mut();
    let rc = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    if rc != ERROR_SUCCESS {
        return Err(format!("registry {path}: error {rc}"));
    }
    let result = f(key);
    unsafe { RegCloseKey(key) };
    result
}

fn set_str(key: HKEY, name: &str, value: &str) -> Result<(), String> {
    let data = wide(value);
    let rc = unsafe { RegSetValueExW(key, wide(name).as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, (data.len() * 2) as u32) };
    if rc == ERROR_SUCCESS { Ok(()) } else { Err(format!("registry {name}: error {rc}")) }
}

fn set_dword(key: HKEY, name: &str, value: u32) -> Result<(), String> {
    let rc = unsafe { RegSetValueExW(key, wide(name).as_ptr(), 0, REG_DWORD, &value as *const u32 as *const u8, 4) };
    if rc == ERROR_SUCCESS { Ok(()) } else { Err(format!("registry {name}: error {rc}")) }
}

/// Entry in Settings → Apps so teiqoflow can be uninstalled like any other program.
pub fn register_uninstaller(dir: &Path, version: &str, size_kb: u32) -> Result<(), String> {
    let exe = dir.join(APP_EXE).to_string_lossy().into_owned();
    let uninstall = format!("\"{}\" --uninstall", dir.join(UNINSTALLER).display());
    with_key(UNINSTALL_KEY, |k| {
        set_str(k, "DisplayName", APP)?;
        set_str(k, "DisplayVersion", version)?;
        set_str(k, "Publisher", APP)?;
        set_str(k, "DisplayIcon", &format!("{exe},0"))?;
        set_str(k, "InstallLocation", &dir.to_string_lossy())?;
        set_str(k, "UninstallString", &uninstall)?;
        set_str(k, "QuietUninstallString", &uninstall)?;
        set_dword(k, "NoModify", 1)?;
        set_dword(k, "NoRepair", 1)?;
        set_dword(k, "EstimatedSize", size_kb)
    })
}

pub fn set_autostart(dir: &Path, enabled: bool) -> Result<(), String> {
    if enabled {
        let cmd = format!("\"{}\"", dir.join(APP_EXE).display());
        with_key(RUN_KEY, |k| set_str(k, APP, &cmd))
    } else {
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), wide(APP).as_ptr()) };
        Ok(())
    }
}

pub fn unregister() {
    unsafe {
        RegDeleteTreeW(HKEY_CURRENT_USER, wide(UNINSTALL_KEY).as_ptr());
        RegDeleteKeyValueW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), wide(APP).as_ptr());
    }
}

pub fn launch(dir: &Path) {
    let _ = Command::new(dir.join(APP_EXE)).current_dir(dir).creation_flags(DETACHED_PROCESS).spawn();
}

/// Removes everything in `dir` except the running uninstaller, which can't delete itself.
pub fn remove_files(dir: &Path) {
    let me = std::env::current_exe().ok();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if Some(&path) == me.as_ref() {
                continue;
            }
            let _ = if path.is_dir() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
        }
    }
}

/// After this process exits, a hidden cmd removes the install folder (including uninstall.exe).
pub fn schedule_folder_removal(dir: &Path) {
    let script = format!("ping -n 3 127.0.0.1 >nul & rmdir /s /q \"{}\"", dir.display());
    let _ = hidden(Command::new("cmd.exe").raw_arg("/c").raw_arg(&script)).spawn();
}
