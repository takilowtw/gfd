//! Packs the files in `$TEIQOFLOW_PAYLOAD` (set by build.ps1) into the installer, deflate-compressed.
//! Without it the installer still compiles, with an empty payload, so `cargo check` works anywhere.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=TEIQOFLOW_PAYLOAD");
    println!("cargo:rerun-if-changed=../assets/icon.ico");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let mut code = String::from("pub static FILES: &[(&str, u64, &[u8])] = &[\n");
    if let Ok(dir) = std::env::var("TEIQOFLOW_PAYLOAD") {
        let mut entries: Vec<_> = std::fs::read_dir(&dir).expect("payload dir").flatten().filter(|e| e.path().is_file()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            println!("cargo:rerun-if-changed={}", path.display());
            let name = entry.file_name().to_string_lossy().into_owned();
            let data = std::fs::read(&path).unwrap();
            let packed = out.join(format!("{name}.deflate"));
            let mut enc = flate2::write::DeflateEncoder::new(std::fs::File::create(&packed).unwrap(), flate2::Compression::new(7));
            enc.write_all(&data).unwrap();
            enc.finish().unwrap();
            writeln!(code, "    ({name:?}, {}, include_bytes!({:?})),", data.len(), packed.display().to_string()).unwrap();
        }
    }
    code.push_str("];\n");
    std::fs::write(out.join("payload.rs"), code).unwrap();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && cfg!(windows) {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../assets/icon.ico");
        res.set("ProductName", "teiqoflow");
        res.set("FileDescription", "teiqoflow setup");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon resource not embedded: {e}");
        }
    }
}
