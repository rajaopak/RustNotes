# Automated Windows Installer Silent-Install Smoke Test for RustNotes
param(
    [string]$InstallerPath = "dist\RustNotes-Setup.exe"
)

$ErrorActionPreference = "Stop"

function Assert-PeX64 {
    param([string]$FilePath, [string]$Label)
    if (-not (Test-Path $FilePath)) {
        throw "$Label missing at '$FilePath'"
    }
    $bytes = [System.IO.File]::ReadAllBytes((Resolve-Path $FilePath))
    if ($bytes.Length -lt 0x40 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
        throw "$Label at '$FilePath' is not a valid Windows PE binary"
    }
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3C)
    $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
    if ($machine -ne 0x8664) {
        throw ("$Label architecture mismatch at '$FilePath': expected 0x8664 (x64), found 0x{0:X4}" -f $machine)
    }
}

$resolvedInstaller = (Resolve-Path $InstallerPath).Path
$tempInstallDir = Join-Path $env:TEMP ("RustNotes-InstallTest-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))

try {
    Write-Host "=== Running Silent Install Smoke Test ===" -ForegroundColor Cyan
    Write-Host "Installer: $resolvedInstaller"
    Write-Host "Target:    $tempInstallDir"

    $installProc = Start-Process -FilePath $resolvedInstaller `
        -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/DIR=`"$tempInstallDir`"") `
        -Wait -PassThru
    if ($installProc.ExitCode -ne 0) {
        throw "Silent installation failed with exit code $($installProc.ExitCode)"
    }

    $exePath = Join-Path $tempInstallDir "rustnotes.exe"
    $dllPath = Join-Path $tempInstallDir "pdfium.dll"

    Assert-PeX64 -FilePath $exePath -Label "Installed rustnotes.exe"
    Assert-PeX64 -FilePath $dllPath -Label "Installed pdfium.dll"

    foreach ($rel in @("assets\icon.ico", "assets\icon.png", "LICENSE.txt", "README.txt")) {
        $p = Join-Path $tempInstallDir $rel
        if (-not (Test-Path $p)) {
            throw "Expected installed file missing: $p"
        }
    }

    $forbiddenLibs = Get-ChildItem -Path $tempInstallDir -Recurse -Filter "*.lib"
    if ($forbiddenLibs) {
        throw "Unexpected .lib file bundled in installation: $($forbiddenLibs.FullName -join ', ')"
    }

    Write-Host "=== Validating Installed Runtime & PDFium Initialization ===" -ForegroundColor Cyan
    $stdoutLog = Join-Path $env:TEMP "rustnotes-install-verify-stdout.txt"
    $stderrLog = Join-Path $env:TEMP "rustnotes-install-verify-stderr.txt"
    $verifyProc = Start-Process -FilePath $exePath `
        -ArgumentList "--verify-runtime" `
        -WorkingDirectory $tempInstallDir `
        -Wait -PassThru -NoNewWindow `
        -RedirectStandardOutput $stdoutLog `
        -RedirectStandardError $stderrLog

    $outText = if (Test-Path $stdoutLog) { Get-Content $stdoutLog -Raw } else { "" }
    $errText = if (Test-Path $stderrLog) { Get-Content $stderrLog -Raw } else { "" }
    Remove-Item -Force $stdoutLog, $stderrLog -ErrorAction SilentlyContinue

    if (-not [string]::IsNullOrWhiteSpace($outText)) { Write-Host $outText.Trim() }
    if (-not [string]::IsNullOrWhiteSpace($errText)) { Write-Host $errText.Trim() -ForegroundColor Red }

    if ($verifyProc.ExitCode -ne 0) {
        throw "Installed --verify-runtime failed with exit code $($verifyProc.ExitCode)"
    }
    if ($outText -notmatch "pdfium=initialized") {
        throw "Runtime verification did not report pdfium=initialized"
    }

    Write-Host "Silent install smoke test PASSED." -ForegroundColor Green
}
finally {
    $uninstaller = Join-Path $tempInstallDir "unins000.exe"
    if (Test-Path $uninstaller) {
        $uninstProc = Start-Process -FilePath $uninstaller `
            -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART") `
            -Wait -PassThru
        if ($uninstProc.ExitCode -ne 0) {
            Write-Warning "Silent uninstaller exited with code $($uninstProc.ExitCode)"
        }
    }
    if (Test-Path $tempInstallDir) {
        Remove-Item -Recurse -Force $tempInstallDir -ErrorAction SilentlyContinue
    }
}
