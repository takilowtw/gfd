# Release build for Windows. Compiles whisper.cpp with clang-cl + Ninja, which runs
# speech recognition several times faster than the default MSVC build.
# Usage:  powershell -ExecutionPolicy Bypass -File build.ps1 [-Clean]
param([switch]$Clean)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$extra = @("$env:USERPROFILE\.cargo\bin", 'C:\Program Files\CMake\bin', 'C:\Program Files\LLVM\bin', "$env:LOCALAPPDATA\Microsoft\WinGet\Links")
$env:PATH = ($extra -join ';') + ';' + $env:PATH

# Load the Visual C++ environment (headers, libs, linker) that Ninja builds need.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'Visual Studio C++ Build Tools not found' }
cmd /c "`"$vs\VC\Auxiliary\Build\vcvars64.bat`" >nul && set" | ForEach-Object {
    if ($_ -match '^([^=]+)=(.*)$') { Set-Item "env:$($matches[1])" $matches[2] }
}
$env:PATH = ($extra -join ';') + ';' + $env:PATH

$env:CMAKE_GENERATOR = 'Ninja'
$env:CMAKE_C_COMPILER = 'clang-cl'
$env:CMAKE_CXX_COMPILER = 'clang-cl'
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'

$ErrorActionPreference = 'Continue'
if ($Clean) { cargo clean --release -p whisper-rs-sys }
cargo build --release
exit $LASTEXITCODE
