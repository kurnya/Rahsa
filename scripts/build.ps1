# Build everything: native host, extension, and the Tauri installer.
#   pwsh scripts/build.ps1            # release: installer (NSIS) + extension zip in release/
#   pwsh scripts/build.ps1 -Dev       # only host + extension, for `npm run tauri dev`
param([switch]$Dev)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$triple = "x86_64-pc-windows-msvc"
$buildProfile = if ($Dev) { "debug" } else { "release" }

# Terminal yang dibuka sebelum Rust dipasang belum punya ~/.cargo/bin di PATH.
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (-not (Get-Command cargo -ErrorAction SilentlyContinue) -and (Test-Path (Join-Path $cargoBin "cargo.exe"))) {
    $env:PATH = "$cargoBin;$env:PATH"
}

Push-Location $root
try {
    Write-Host "==> rahsa-host ($buildProfile)"
    if ($Dev) { cargo build -p rahsa-host } else { cargo build -p rahsa-host --release }
    if ($LASTEXITCODE) { throw "build rahsa-host gagal" }
    New-Item -ItemType Directory -Force "app/src-tauri/binaries" | Out-Null
    Copy-Item "target/$buildProfile/rahsa-host.exe" "app/src-tauri/binaries/rahsa-host-$triple.exe" -Force

    Write-Host "==> extension"
    Push-Location extension
    if (-not (Test-Path node_modules)) { npm ci }
    npm run build
    if ($LASTEXITCODE) { throw "build ekstensi gagal" }
    Pop-Location

    Push-Location app
    if (-not (Test-Path node_modules)) { npm ci }
    if (-not $Dev) {
        Write-Host "==> app installer"
        npm run tauri build
        if ($LASTEXITCODE) { throw "build installer gagal" }
    }
    Pop-Location

    if ($Dev) {
        Write-Host "Ekstensi siap dimuat dari: $root\extension\dist"
    }
    else {
        # Gather everything to hand out in one folder: installer + extension zip.
        $version = (Get-Content extension/public/manifest.json -Raw | ConvertFrom-Json).version
        $out = Join-Path $root "release"
        New-Item -ItemType Directory -Force $out | Out-Null
        Get-ChildItem "target/release/bundle/nsis/*.exe" | Copy-Item -Destination $out -Force
        $zip = Join-Path $out "rahsa-extension-$version.zip"
        if (Test-Path $zip) { Remove-Item $zip }
        Compress-Archive -Path "extension/dist/*" -DestinationPath $zip
        Copy-Item "docs/CARA-PASANG.md" (Join-Path $out "CARA-PASANG.md") -Force
        Write-Host ""
        Write-Host "Siap dibagikan di: $out"
        Get-ChildItem $out | ForEach-Object { Write-Host ("  {0}  ({1:N1} MB)" -f $_.Name, ($_.Length / 1MB)) }
    }
}
finally {
    Pop-Location
}
