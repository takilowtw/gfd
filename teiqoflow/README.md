# teiqoflow

Push-to-talk dictation with instant translation, in the spirit of Wispr Flow. Written in Rust.

Hold your shortcut (default **Right Ctrl**), speak, and release. Your speech is transcribed locally
with Whisper, translated into the language you picked, and pasted into whichever chat or text field
has focus. A small animated pill at the bottom of the screen shows when it is listening and working.

## Build

Requirements on Windows: Rust (MSVC toolchain), Visual Studio C++ Build Tools, CMake and LLVM.

```
powershell -ExecutionPolicy Bypass -File build.ps1
target\release\teiqoflow.exe
```

`build.ps1` compiles whisper.cpp with clang-cl and Ninja (`winget install Ninja-build.Ninja`), which makes
recognition much faster than a plain `cargo build`. The first build takes a few minutes.
To check recognition and translation without a microphone:
`target\release\teiqoflow.exe --transcribe some.wav en | more`

GPU acceleration: install the Vulkan SDK (`winget install KhronosGroup.VulkanSDK`) and run
`build.ps1 -Clean`; it then builds with `--features vulkan` automatically.

## Use

- Closing the window hides it to the tray; quit from the tray icon's menu.
- On first start the selected speech model is downloaded to `%APPDATA%\teiqoflow\models`.
- Pressing any other key while holding the shortcut cancels the recording, so ordinary shortcuts
  such as Ctrl+C keep working.
- Translation uses Google Translate and needs an internet connection. Turn **Translate** off to type
  exactly what you said.
- To dictate into apps running as administrator, run teiqoflow as administrator too.
- Settings and history are stored in `%APPDATA%\teiqoflow\config.json`.
