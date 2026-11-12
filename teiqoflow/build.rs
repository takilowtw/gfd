fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    // Only when building for Windows on Windows, where the resource compiler is available.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && cfg!(windows) {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "teiqoflow");
        res.set("FileDescription", "teiqoflow");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon resource not embedded: {e}");
        }
    }
}
