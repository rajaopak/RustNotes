# Portable Windows Distribution Packaging Script for RustNotes (x86_64)
param(
    [switch]$SkipBuild,
    [string]$TargetDir = "target\release"
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$RepoRoot = $PSScriptRoot
Push-Location $RepoRoot
try {
    # Ensure cargo is available if installed in standard user profile path
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $userCargo = Join-Path $env:USERPROFILE ".cargo\bin"
        if (Test-Path $userCargo) {
            $env:Path = "$userCargo;$env:Path"
        }
    }

    # Single source of truth for version from Cargo.toml
    $cargoToml = Get-Content "Cargo.toml" -Raw
    if ($cargoToml -match '(?m)^version\s*=\s*"([^"]+)"') {
        $version = $Matches[1]
    } else {
        throw "Could not determine version from Cargo.toml"
    }

    if (-not $SkipBuild) {
        Write-Host "=== Building RustNotes v$version (Release x86_64) ===" -ForegroundColor Cyan
        cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build --release failed with exit code $LASTEXITCODE"
        }
    }

    $exeSrc = Join-Path $TargetDir "rustnotes.exe"
    $targetDll = Join-Path $TargetDir "pdfium.dll"
    $dllSrc = if (Test-Path $targetDll) { $targetDll } else { "pdfium.dll" }
    $iconIco = "assets\icon.ico"
    $iconPng = "assets\icon.png"
    $licenseSrc = "LICENSE"

    foreach ($req in @($exeSrc, $dllSrc, $iconIco, $iconPng, $licenseSrc)) {
        if (-not (Test-Path $req)) {
            throw "Required distribution file missing: $req"
        }
    }

    # Verify pdfium.dll is x86_64 (PE Machine 0x8664)
    $dllBytes = [System.IO.File]::ReadAllBytes((Resolve-Path $dllSrc))
    $peOffset = [BitConverter]::ToInt32($dllBytes, 0x3C)
    $machine = [BitConverter]::ToUInt16($dllBytes, $peOffset + 4)
    if ($machine -ne 0x8664) {
        throw ("pdfium.dll architecture mismatch: expected 0x8664 (x64), found 0x{0:X4}" -f $machine)
    }

    $distDir = "dist"
    $portableDir = Join-Path $distDir "RustNotes-Portable"
    $zipFile = Join-Path $distDir "RustNotes-Portable.zip"

    if (Test-Path $portableDir) {
        Remove-Item -Recurse -Force $portableDir
    }
    if (Test-Path $zipFile) {
        Remove-Item -Force $zipFile
    }

    New-Item -ItemType Directory -Force -Path $portableDir | Out-Null
    New-Item -ItemType Directory -Force -Path (Join-Path $portableDir "assets") | Out-Null

    Write-Host "=== Assembling Portable Distribution ($portableDir) ===" -ForegroundColor Cyan
    Copy-Item $exeSrc -Destination (Join-Path $portableDir "rustnotes.exe")
    Copy-Item $dllSrc -Destination (Join-Path $portableDir "pdfium.dll")
    Copy-Item $iconIco -Destination (Join-Path $portableDir "assets\icon.ico")
    Copy-Item $iconPng -Destination (Join-Path $portableDir "assets\icon.png")
    Copy-Item $licenseSrc -Destination (Join-Path $portableDir "LICENSE.txt")

    # Portable mode marker: stores user notes, settings, and crash logs in .\data\
    @"
This file enables RustNotes Portable Mode.
User notes, settings, templates, and logs are stored in the .\data\ directory next to rustnotes.exe.
Remove this file (and the .\data\ folder) to use the standard Windows user directory (%LOCALAPPDATA%\RustNotes).
"@ | Set-Content -Path (Join-Path $portableDir "portable.txt") -Encoding UTF8

    $releaseDate = Get-Date -Format "yyyy-MM-dd"
    @"
RustNotes v$version — Portable Windows (x86_64)
Release Date: $releaseDate
Repository: https://github.com/rajaopak/RustNotes

Quick Start:
1. Extract the entire RustNotes-Portable folder to any writable directory (do not run directly from inside a ZIP preview).
2. Double-click rustnotes.exe to launch RustNotes.

Portable Storage Behavior:
- Because portable.txt is present next to rustnotes.exe, all notebooks, settings, custom templates, and crash logs are stored in .\data\ inside this folder.
- When upgrading to a newer portable release, copy your .\data\ folder into the new RustNotes-Portable folder.
- Standard installed versions store data in %LOCALAPPDATA%\RustNotes instead.

Runtime Dependencies & Third-Party Notices:
- pdfium.dll (x86_64) is bundled for PDF import and PDF export.
- PDFium is open-source software licensed under the BSD 3-Clause License (https://pdfium.googlesource.com/pdfium/).
- RustNotes itself is licensed under the MIT License (see LICENSE.txt).
"@ | Set-Content -Path (Join-Path $portableDir "README.txt") -Encoding UTF8

    Write-Host "=== Validating Portable Runtime ===" -ForegroundColor Cyan
    $verifyExe = (Resolve-Path (Join-Path $portableDir "rustnotes.exe")).Path
    $stdoutLog = Join-Path $env:TEMP "rustnotes-verify-stdout.txt"
    $stderrLog = Join-Path $env:TEMP "rustnotes-verify-stderr.txt"
    $proc = Start-Process -FilePath $verifyExe `
        -ArgumentList "--verify-runtime" `
        -WorkingDirectory $portableDir `
        -Wait -PassThru -NoNewWindow `
        -RedirectStandardOutput $stdoutLog `
        -RedirectStandardError $stderrLog
    if (Test-Path $stdoutLog) {
        Get-Content $stdoutLog | Write-Host
        Remove-Item -Force $stdoutLog -ErrorAction SilentlyContinue
    }
    if (Test-Path $stderrLog) {
        $errText = Get-Content $stderrLog -Raw
        Remove-Item -Force $stderrLog -ErrorAction SilentlyContinue
        if (-not [string]::IsNullOrWhiteSpace($errText)) {
            Write-Host $errText -ForegroundColor Red
        }
    }
    if ($proc.ExitCode -ne 0) {
        throw "Portable runtime validation failed with exit code $($proc.ExitCode)!"
    }
    # Clean up the empty data dir created by the write probe so the ZIP starts clean
    $probeDataDir = Join-Path $portableDir "data"
    if (Test-Path $probeDataDir) {
        Remove-Item -Recurse -Force $probeDataDir
    }

    Write-Host "=== Creating Portable ZIP ($zipFile) ===" -ForegroundColor Cyan
    Compress-Archive -Path "$portableDir\*" -DestinationPath $zipFile -Force

    $zipSize = (Get-Item $zipFile).Length / 1MB
    Write-Host ("Portable package created: {0} ({1:N2} MB)" -f $zipFile, $zipSize) -ForegroundColor Green
}
finally {
    Pop-Location
}
