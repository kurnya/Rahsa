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
        # One folder to hand out: installer + unpacked extension + guide, then zipped.
        #   release/Rahsa-<versi>-Windows/{Rahsa_<versi>_x64-setup.exe, ekstensi-chrome/, CARA-PASANG.txt}
        #   release/Rahsa-<versi>-Windows.zip
        $version = (Get-Content extension/public/manifest.json -Raw | ConvertFrom-Json).version
        $name = "Rahsa-$version-Windows"
        $out = Join-Path $root "release"
        $pkg = Join-Path $out $name
        if (Test-Path $pkg) { Remove-Item $pkg -Recurse -Force }
        New-Item -ItemType Directory -Force $pkg | Out-Null

        Copy-Item "target/release/bundle/nsis/Rahsa_${version}_x64-setup.exe" $pkg
        Copy-Item "extension/dist" (Join-Path $pkg "ekstensi-chrome") -Recurse
        # Notepad-friendly: Windows line endings, real version number filled in.
        $guide = (Get-Content "docs/CARA-PASANG.txt" -Raw) -replace "<versi>", $version -replace "\r?\n", "`r`n"
        [IO.File]::WriteAllText((Join-Path $pkg "CARA-PASANG.txt"), $guide, [Text.UTF8Encoding]::new($true))

        $zip = Join-Path $out "$name.zip"
        if (Test-Path $zip) { Remove-Item $zip }
        Compress-Archive -Path $pkg -DestinationPath $zip
        Write-Host ""
        Write-Host ("Siap dibagikan: {0}  ({1:N1} MB)" -f $zip, ((Get-Item $zip).Length / 1MB))
    }
}
finally {
    Pop-Location
}
