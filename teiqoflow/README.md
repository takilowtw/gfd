# teiqoflow

Push-to-talk dictation with instant translation, in the spirit of Wispr Flow. Written in Rust.

Hold your shortcut (default **Right Ctrl**), speak, and release. Your speech is transcribed locally
with Whisper, translated into the language you picked, and pasted into whichever chat or text field
has focus. A small animated pill at the bottom of the screen shows when it is listening and working.

## Build

Requirements on Windows: Rust (MSVC toolchain), Visual Studio C++ Build Tools, CMake and LLVM.

```
cargo build --release
target\release\teiqoflow.exe
```

The first build compiles whisper.cpp and takes a few minutes. Optional NVIDIA GPU acceleration needs the
CUDA Toolkit and `cargo build --release --features cuda`.

## Use

- Closing the window hides it to the tray; quit from the tray icon's menu.
- On first start the selected speech model is downloaded to `%APPDATA%\teiqoflow\models`.
- Pressing any other key while holding the shortcut cancels the recording, so ordinary shortcuts
  such as Ctrl+C keep working.
- Translation uses Google Translate and needs an internet connection. Turn **Translate** off to type
  exactly what you said.
- To dictate into apps running as administrator, run teiqoflow as administrator too.
- Settings and history are stored in `%APPDATA%\teiqoflow\config.json`.
