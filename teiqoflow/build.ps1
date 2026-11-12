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

# With the Vulkan SDK installed, recognition runs on the GPU (any NVIDIA/AMD/Intel card).
if (-not $env:VULKAN_SDK) { $env:VULKAN_SDK = [Environment]::GetEnvironmentVariable('VULKAN_SDK', 'Machine') }
if ($env:VULKAN_SDK -and (Test-Path $env:VULKAN_SDK)) {
    Write-Host "Building with GPU (Vulkan) support from $env:VULKAN_SDK"
    $env:PATH = "$env:VULKAN_SDK\Bin;$env:PATH"
    cargo build --release --features vulkan
} else {
    Write-Host 'Vulkan SDK not found: building CPU-only (winget install KhronosGroup.VulkanSDK for GPU).'
    cargo build --release
}
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# Installer: bundles the app plus the Visual C++ runtime so it runs on PCs without it.
$payload = Join-Path $PSScriptRoot 'target\payload'
Remove-Item $payload -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $payload | Out-Null
Copy-Item 'target\release\teiqoflow.exe' $payload
$crt = Get-ChildItem "$vs\VC\Redist\MSVC\*\x64\Microsoft.VC*.CRT" -Directory | Sort-Object FullName | Select-Object -Last 1
if ($crt) {
    foreach ($dll in 'vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll', 'msvcp140_1.dll') {
        Copy-Item (Join-Path $crt.FullName $dll) $payload -ErrorAction SilentlyContinue
    }
} else {
    Write-Warning 'Visual C++ runtime not found; the installer will rely on the one on the target PC.'
}
$env:TEIQOFLOW_PAYLOAD = $payload
cargo build --release -p teiqoflow-setup
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
New-Item -ItemType Directory 'dist' -Force | Out-Null
Copy-Item 'target\release\teiqoflow-setup.exe' 'dist\teiqoflow-setup.exe' -Force
Write-Host "Installer: $(Resolve-Path 'dist\teiqoflow-setup.exe')"
exit 0
