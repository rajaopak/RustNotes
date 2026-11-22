# RustNotes Windows Release & Distribution Guide

## 1. Versioning (Single Source of Truth)

The canonical application version is defined in `Cargo.toml` (`[package].version`).

- Application binary (`rustnotes.exe`) reads `env!("CARGO_PKG_VERSION")` at compile time.
- `package_portable.ps1` parses `Cargo.toml` automatically.
- `installer/rustnotes.iss` accepts `/DMyAppVersion=<version>` from CI or local build scripts.
- `.github/workflows/windows-release.yml` validates that any `v*` release tag matches `Cargo.toml`.

## 2. Runtime Dependencies & PDFium Requirements

| Component | Purpose | Architecture | License | Bundling |
|---|---|---|---|---|
| `rustnotes.exe` | Native eframe/egui desktop binary | `x86_64` (`0x8664`) | MIT (`LICENSE`) | Primary executable |
| `pdfium.dll` | Dynamic PDF import/export via `pdfium-render` 0.8 | `x86_64` (`0x8664`) | BSD 3-Clause | Bundled adjacent to `rustnotes.exe` |
| `assets/icon.ico`, `assets/icon.png` | Window & Windows shortcut icons | — | MIT | Embedded (`icon.png`) + shortcut assets (`icon.ico`) |

- **Canonical `pdfium.dll` location:** Repository root (`pdfium.dll`, tracked in Git).
- **Build-time validation (`build.rs`):** Release builds fail immediately if `pdfium.dll` is missing, is not a valid Windows PE binary, or does not match `x86_64` (`0x8664`). `build.rs` copies the validated DLL into the active Cargo profile directory (`target\release\` or `target\x86_64-pc-windows-msvc\release\`).
- **Installer packaging (`installer/rustnotes.iss`):** Sources both `rustnotes.exe` and `pdfium.dll` from `{#BuildDir}` and installs them side-by-side in `{app}`. `.lib` / `.dll.lib` files are never bundled at runtime.

## 3. Data & Storage Paths

| Data Type | Installed Mode | Portable Mode (`portable.txt` present) |
|---|---|---|
| Application executable & `pdfium.dll` | `{autopf}\RustNotes\` | `<ExtractedDir>\RustNotes-Portable\` |
| Notebooks, library index, templates, settings | `%LOCALAPPDATA%\RustNotes\` | `<ExtractedDir>\RustNotes-Portable\data\` |
| Crash logs | `%LOCALAPPDATA%\RustNotes\logs\` | `<ExtractedDir>\RustNotes-Portable\data\logs\` |

- Installed mode never writes user data into `Program Files` and never requires administrator privileges to save notes.
- Portable mode is activated explicitly when `portable.txt` (or `.portable` / `data/`) exists next to `rustnotes.exe`, or when `RUSTNOTES_PORTABLE=1` is set.

## 4. Building & Validating Locally

```powershell
# 1. Verify formatting, lints, and tests (including PDFium initialization & PDF roundtrip)
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test

# 2. Build Portable ZIP (dist/RustNotes-Portable.zip) and validate runtime
.\package_portable.ps1

# 3. Build Windows Installer (dist/RustNotes-Setup.exe) — requires Inno Setup 6
iscc installer\rustnotes.iss

# 4. Run Automated Silent-Install Smoke Test (verifies pdfium.dll + --verify-runtime)
.\scripts\verify_installer.ps1 -InstallerPath "dist\RustNotes-Setup.exe"
```

## 5. Troubleshooting `pdfium.dll not found`

1. Verify `pdfium.dll` exists in the exact same directory as `rustnotes.exe` (e.g. `{app}\pdfium.dll` or `RustNotes-Portable\pdfium.dll`).
2. Run `rustnotes.exe --verify-runtime` from that directory. On failure, it prints all searched paths and distinguishes missing files, invalid PE headers, x86/ARM64 architecture mismatches, and OS loader dependency errors.
3. For custom/override locations, set `RUSTNOTES_PDFIUM_PATH` (or `PDFIUM_PATH`) to the full path or directory containing an `x86_64` `pdfium.dll`.

## 6. Clean Machine Verification Checklist

### Portable ZIP (`RustNotes-Portable.zip`)
1. Extract `RustNotes-Portable.zip` to a folder outside the repository on a machine without Rust/Cargo installed.
2. Run `rustnotes.exe --verify-runtime` (verifies `pdfium=initialized`) and launch `rustnotes.exe` normally.
3. Create a notebook, draw a stroke, import/export a PDF, and close the app.
4. Confirm `.\data\` was created next to `rustnotes.exe` and contains the notebook & settings.
5. Reopen `rustnotes.exe` and verify the notebook persists.

### Windows Installer (`RustNotes-Setup.exe`)
1. Run `RustNotes-Setup.exe` as a standard user and verify Start Menu / optional Desktop shortcuts.
2. Confirm both `rustnotes.exe` and `pdfium.dll` exist in the installation directory, or run `rustnotes.exe --verify-runtime`.
3. Launch RustNotes from the Start Menu, create a notebook, and test PDF import and PDF export.
4. Close RustNotes and verify notes are stored under `%LOCALAPPDATA%\RustNotes\`.
5. Run a newer `RustNotes-Setup.exe` over the existing installation; verify notes & settings remain intact.
6. Uninstall via Windows Installed Apps; verify binaries and shortcuts are removed while `%LOCALAPPDATA%\RustNotes\` is preserved unless explicitly opted in during interactive uninstall.
