# Automated Windows Installer Lifecycle Smoke Test for RustNotes
param(
    [string]$InstallerPath = "dist\RustNotes-Setup.exe",
    [string]$OlderInstallerPath = "",
    [string]$NewerInstallerPath = "",
    [string]$ExpectedBaseVersion = "",
    [string]$ExpectedNewerVersion = "",
    [switch]$AllowExistingInstall
)

$ErrorActionPreference = "Stop"

$UninstallRegPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{D37F8E45-926B-4D3C-85AA-A4F1D7728B48}_is1"

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

function Assert-RuntimeHealthy {
    param([string]$ExePath, [string]$WorkingDir)
    $stdoutLog = Join-Path $env:TEMP ("rustnotes-install-verify-stdout-" + [Guid]::NewGuid().ToString("N").Substring(0, 8) + ".txt")
    $stderrLog = Join-Path $env:TEMP ("rustnotes-install-verify-stderr-" + [Guid]::NewGuid().ToString("N").Substring(0, 8) + ".txt")
    try {
        $verifyProc = Start-Process -FilePath $ExePath `
            -ArgumentList "--verify-runtime" `
            -WorkingDirectory $WorkingDir `
            -Wait -PassThru -NoNewWindow `
            -RedirectStandardOutput $stdoutLog `
            -RedirectStandardError $stderrLog

        $outText = if (Test-Path $stdoutLog) { Get-Content $stdoutLog -Raw } else { "" }
        $errText = if (Test-Path $stderrLog) { Get-Content $stderrLog -Raw } else { "" }

        if (-not [string]::IsNullOrWhiteSpace($outText)) { Write-Host $outText.Trim() }
        if (-not [string]::IsNullOrWhiteSpace($errText)) { Write-Host $errText.Trim() -ForegroundColor Red }

        if ($verifyProc.ExitCode -ne 0) {
            throw "Installed --verify-runtime failed with exit code $($verifyProc.ExitCode)"
        }
        if ($outText -notmatch "pdfium=initialized") {
            throw "Runtime verification did not report pdfium=initialized"
        }
    }
    finally {
        Remove-Item -Force $stdoutLog, $stderrLog -ErrorAction SilentlyContinue
    }
}

function Assert-InstalledFilesValid {
    param([string]$InstallDir)
    $exePath = Join-Path $InstallDir "rustnotes.exe"
    $dllPath = Join-Path $InstallDir "pdfium.dll"

    Assert-PeX64 -FilePath $exePath -Label "Installed rustnotes.exe"
    Assert-PeX64 -FilePath $dllPath -Label "Installed pdfium.dll"

    foreach ($rel in @("assets\icon.ico", "assets\icon.png", "LICENSE.txt", "README.txt")) {
        $p = Join-Path $InstallDir $rel
        if (-not (Test-Path $p)) {
            throw "Expected installed file missing: $p"
        }
    }

    if (Test-Path (Join-Path $InstallDir "portable.txt")) {
        throw "Installed build must NOT contain portable.txt"
    }

    $forbiddenLibs = Get-ChildItem -Path $InstallDir -Recurse -Filter "*.lib"
    if ($forbiddenLibs) {
        throw "Unexpected .lib file bundled in installation: $($forbiddenLibs.FullName -join ', ')"
    }
}

function Assert-CanaryPreserved {
    param([string]$CanaryPath, [string]$ExpectedHash, [string]$Stage)
    if (-not (Test-Path $CanaryPath)) {
        throw "User notes canary missing after '$Stage' ($CanaryPath)"
    }
    $actualHash = (Get-FileHash -Path $CanaryPath -Algorithm SHA256).Hash
    if ($actualHash -ne $ExpectedHash) {
        throw "User notes canary modified during '$Stage' (expected $ExpectedHash, got $actualHash)"
    }
}

if (-not $AllowExistingInstall -and (Test-Path $UninstallRegPath)) {
    throw "Existing per-user RustNotes installation detected in HKCU ($UninstallRegPath). Refusing to run destructive test unless -AllowExistingInstall is specified."
}

$resolvedInstaller = (Resolve-Path $InstallerPath).Path
$resolvedOlder = if (-not [string]::IsNullOrWhiteSpace($OlderInstallerPath)) { (Resolve-Path $OlderInstallerPath).Path } else { "" }
$resolvedNewer = if (-not [string]::IsNullOrWhiteSpace($NewerInstallerPath)) { (Resolve-Path $NewerInstallerPath).Path } else { "" }

$tempInstallDir = Join-Path $env:TEMP ("RustNotes-InstallTest-" + [Guid]::NewGuid().ToString("N").Substring(0, 8))
$localDataDir = Join-Path $env:LOCALAPPDATA "RustNotes"
$createdLocalDataDir = -not (Test-Path $localDataDir)
if ($createdLocalDataDir) {
    New-Item -ItemType Directory -Force -Path $localDataDir | Out-Null
}

$canaryPath = Join-Path $localDataDir (".installer-smoke-canary-" + [Guid]::NewGuid().ToString("N").Substring(0, 8) + ".json")
'{"canary":"rustnotes-user-notes-must-survive-install-and-upgrade"}' | Set-Content -Path $canaryPath -Encoding UTF8
$canaryHash = (Get-FileHash -Path $canaryPath -Algorithm SHA256).Hash

try {
    Write-Host "=== [1/4] Fresh Silent Install Smoke Test ===" -ForegroundColor Cyan
    Write-Host "Installer: $resolvedInstaller"
    Write-Host "Target:    $tempInstallDir"

    $installProc = Start-Process -FilePath $resolvedInstaller `
        -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER", "/DIR=`"$tempInstallDir`"") `
        -Wait -PassThru
    if ($installProc.ExitCode -ne 0) {
        throw "Silent installation failed with exit code $($installProc.ExitCode)"
    }

    Assert-InstalledFilesValid -InstallDir $tempInstallDir
    Assert-RuntimeHealthy -ExePath (Join-Path $tempInstallDir "rustnotes.exe") -WorkingDir $tempInstallDir
    Assert-CanaryPreserved -CanaryPath $canaryPath -ExpectedHash $canaryHash -Stage "fresh install"

    $regProps = Get-ItemProperty -Path $UninstallRegPath
    if (-not [string]::IsNullOrWhiteSpace($ExpectedBaseVersion) -and ($regProps.DisplayVersion -ne $ExpectedBaseVersion)) {
        throw "Expected DisplayVersion '$ExpectedBaseVersion', got '$($regProps.DisplayVersion)'"
    }
    $recordedBaseVersion = $regProps.DisplayVersion
    Write-Host "Fresh install verified (DisplayVersion=$recordedBaseVersion)." -ForegroundColor Green

    Write-Host "=== [2/4] Same-Version Re-Run Rejection Test ===" -ForegroundColor Cyan
    $sameProc = Start-Process -FilePath $resolvedInstaller `
        -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER") `
        -Wait -PassThru
    if ($sameProc.ExitCode -eq 0) {
        throw "Expected same-version re-run without /REINSTALL to abort cleanly with non-zero exit code, but got 0"
    }
    Assert-CanaryPreserved -CanaryPath $canaryPath -ExpectedHash $canaryHash -Stage "same-version check"
    $afterSameVer = (Get-ItemProperty -Path $UninstallRegPath).DisplayVersion
    if ($afterSameVer -ne $recordedBaseVersion) {
        throw "DisplayVersion changed unexpectedly during same-version run: '$afterSameVer'"
    }
    Write-Host "Same-version guard PASSED (aborted with exit code $($sameProc.ExitCode))." -ForegroundColor Green

    if (-not [string]::IsNullOrWhiteSpace($resolvedOlder)) {
        Write-Host "=== [3/4] Older Installer Downgrade Guard Test ===" -ForegroundColor Cyan
        $olderProc = Start-Process -FilePath $resolvedOlder `
            -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER") `
            -Wait -PassThru
        if ($olderProc.ExitCode -eq 0) {
            throw "Downgrade MUST be blocked! Older installer exited with code 0"
        }
        $afterDowngradeVer = (Get-ItemProperty -Path $UninstallRegPath).DisplayVersion
        if ($afterDowngradeVer -ne $recordedBaseVersion) {
            throw "DisplayVersion changed after blocked downgrade: expected '$recordedBaseVersion', got '$afterDowngradeVer'"
        }
        Assert-InstalledFilesValid -InstallDir $tempInstallDir
        Assert-CanaryPreserved -CanaryPath $canaryPath -ExpectedHash $canaryHash -Stage "downgrade guard"
        Write-Host "Downgrade guard PASSED (blocked with exit code $($olderProc.ExitCode))." -ForegroundColor Green
    }

    if (-not [string]::IsNullOrWhiteSpace($resolvedNewer)) {
        Write-Host "=== [4/4] Newer Installer In-Place Upgrade + Notes Preservation Test ===" -ForegroundColor Cyan
        $upgradeProc = Start-Process -FilePath $resolvedNewer `
            -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CURRENTUSER") `
            -Wait -PassThru
        if ($upgradeProc.ExitCode -ne 0) {
            throw "Newer installer in-place upgrade failed with exit code $($upgradeProc.ExitCode)"
        }
        Assert-InstalledFilesValid -InstallDir $tempInstallDir
        Assert-RuntimeHealthy -ExePath (Join-Path $tempInstallDir "rustnotes.exe") -WorkingDir $tempInstallDir
        Assert-CanaryPreserved -CanaryPath $canaryPath -ExpectedHash $canaryHash -Stage "in-place upgrade"

        $upgradedProps = Get-ItemProperty -Path $UninstallRegPath
        if (-not [string]::IsNullOrWhiteSpace($ExpectedNewerVersion) -and ($upgradedProps.DisplayVersion -ne $ExpectedNewerVersion)) {
            throw "Expected upgraded DisplayVersion '$ExpectedNewerVersion', got '$($upgradedProps.DisplayVersion)'"
        }
        Write-Host "In-place upgrade PASSED (DisplayVersion=$($upgradedProps.DisplayVersion))." -ForegroundColor Green
    }

    Write-Host "Windows Installer lifecycle smoke test PASSED." -ForegroundColor Green
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
    try {
        Assert-CanaryPreserved -CanaryPath $canaryPath -ExpectedHash $canaryHash -Stage "silent uninstall"
    }
    finally {
        Remove-Item -Force $canaryPath -ErrorAction SilentlyContinue
        if ($createdLocalDataDir -and (Test-Path $localDataDir)) {
            $remaining = Get-ChildItem -Force $localDataDir -ErrorAction SilentlyContinue
            if (-not $remaining) {
                Remove-Item -Force $localDataDir -ErrorAction SilentlyContinue
            }
        }
    }
    if (Test-Path $tempInstallDir) {
        Remove-Item -Recurse -Force $tempInstallDir -ErrorAction SilentlyContinue
    }
}
