# Build everything: native host, extension, and the Tauri installer.
#   pwsh scripts/build.ps1            # release installer (NSIS)
#   pwsh scripts/build.ps1 -Dev       # only host + extension, for `npm run tauri dev`
param([switch]$Dev)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$triple = "x86_64-pc-windows-msvc"
$profile = if ($Dev) { "debug" } else { "release" }

# Terminal yang dibuka sebelum Rust dipasang belum punya ~/.cargo/bin di PATH.
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (-not (Get-Command cargo -ErrorAction SilentlyContinue) -and (Test-Path (Join-Path $cargoBin "cargo.exe"))) {
    $env:PATH = "$cargoBin;$env:PATH"
}

Push-Location $root
try {
    Write-Host "==> rahsa-host ($profile)"
    if ($Dev) { cargo build -p rahsa-host } else { cargo build -p rahsa-host --release }
    New-Item -ItemType Directory -Force "app/src-tauri/binaries" | Out-Null
    Copy-Item "target/$profile/rahsa-host.exe" "app/src-tauri/binaries/rahsa-host-$triple.exe" -Force

    Write-Host "==> extension"
    Push-Location extension
    if (-not (Test-Path node_modules)) { npm ci }
    npm run build
    Pop-Location

    Push-Location app
    if (-not (Test-Path node_modules)) { npm ci }
    if (-not $Dev) {
        Write-Host "==> app installer"
        npm run tauri build
        Write-Host "Installer: target/release/bundle/nsis/"
    }
    Pop-Location
    Write-Host "Ekstensi siap dimuat dari: $root\extension\dist"
}
finally {
    Pop-Location
}
