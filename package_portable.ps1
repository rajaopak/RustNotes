# Script Packaging Portable Distribution untuk RustNotes Windows x64
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$version = "0.1.0"
$distDir = "dist"
$targetFolder = "$distDir\rustnotes-v$version-windows-x64"
$zipFile = "$distDir\rustnotes-v$version-windows-x64.zip"

Write-Host "=== Membangun Biner Release RustNotes ===" -ForegroundColor Cyan
$env:Path = "C:\Users\Nafis\.cargo\bin;" + $env:Path
cargo build --release

if (Test-Path $distDir) {
    Remove-Item -Recurse -Force $distDir
}
New-Item -ItemType Directory -Force -Path $targetFolder | Out-Null
New-Item -ItemType Directory -Force -Path "$targetFolder\assets" | Out-Null

Write-Host "=== Menyalin Berkas Paket ===" -ForegroundColor Cyan
Copy-Item "target\release\rustnotes.exe" -Destination $targetFolder
Copy-Item "pdfium.dll" -Destination $targetFolder
Copy-Item "assets\icon.ico" -Destination "$targetFolder\assets"
Copy-Item "assets\icon.png" -Destination "$targetFolder\assets"
Copy-Item "RUSTNOTES.md" -Destination $targetFolder

Write-Host "=== Membuat Berkas ZIP Portable ===" -ForegroundColor Cyan
Compress-Archive -Path "$targetFolder\*" -DestinationPath $zipFile -Force

$zipSize = (Get-Item $zipFile).Length / 1MB
Write-Host "✅ Paket Portable Berhasil Dibuat: $zipFile ($([math]::Round($zipSize, 2)) MB)" -ForegroundColor Green
