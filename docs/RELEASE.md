# RustNotes Windows Release & Distribution Guide

## 1. Versioning (Single Source of Truth)

The canonical application version is defined in `Cargo.toml` (`[package].version`).

- Application binary (`rustnotes.exe`) reads `env!("CARGO_PKG_VERSION")` at compile time.
- `package_portable.ps1` parses `Cargo.toml` automatically.
- `installer/rustnotes.iss` accepts `/DMyAppVersion=<version>` from CI or local build scripts.
- `.github/workflows/windows-release.yml` validates that any `v*` release tag matches `Cargo.toml`.

## 2. Runtime Dependencies & Licensing

| Component | Purpose | Architecture | License | Bundling |
|---|---|---|---|---|
| `rustnotes.exe` | Native eframe/egui desktop binary | `x86_64` (`0x8664`) | MIT (`LICENSE`) | Primary executable |
| `pdfium.dll` | Dynamic PDF import/export via `pdfium-render` 0.8 | `x86_64` (`0x8664`) | BSD 3-Clause | Bundled adjacent to `rustnotes.exe` |
| `assets/icon.ico`, `assets/icon.png` | Window & Windows shortcut icons | — | MIT | Embedded (`icon.png`) + shortcut assets (`icon.ico`) |

## 3. Data & Storage Paths

| Data Type | Installed Mode | Portable Mode (`portable.txt` present) |
|---|---|---|
| Application executable & `pdfium.dll` | `{autopf}\RustNotes\` | `<ExtractedDir>\RustNotes-Portable\` |
| Notebooks, library index, templates, settings | `%LOCALAPPDATA%\RustNotes\` | `<ExtractedDir>\RustNotes-Portable\data\` |
| Crash logs | `%LOCALAPPDATA%\RustNotes\logs\` | `<ExtractedDir>\RustNotes-Portable\data\logs\` |

- Installed mode never writes user data into `Program Files` and never requires administrator privileges to save notes.
- Portable mode is activated explicitly when `portable.txt` (or `.portable` / `data/`) exists next to `rustnotes.exe`, or when `RUSTNOTES_PORTABLE=1` is set.

## 4. Building Locally

```powershell
# 1. Verify formatting, lints, and tests
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test

# 2. Build Portable ZIP (dist/RustNotes-Portable.zip) and validate runtime
.\package_portable.ps1

# 3. Build Windows Installer (dist/RustNotes-Setup.exe) — requires Inno Setup 6
iscc installer\rustnotes.iss
```

## 5. Clean Machine Verification Checklist

### Portable ZIP (`RustNotes-Portable.zip`)
1. Extract `RustNotes-Portable.zip` to a folder outside the repository on a machine without Rust/Cargo installed.
2. Run `rustnotes.exe --verify-runtime` (optional CLI check) and launch `rustnotes.exe` normally.
3. Create a notebook, draw a stroke, export to PDF, and close the app.
4. Confirm `.\data\` was created next to `rustnotes.exe` and contains the notebook & settings.
5. Reopen `rustnotes.exe` and verify the notebook persists.

### Windows Installer (`RustNotes-Setup.exe`)
1. Run `RustNotes-Setup.exe` as a standard user and verify Start Menu / optional Desktop shortcuts.
2. Launch RustNotes from the Start Menu, create a notebook, and test PDF export.
3. Close RustNotes and verify notes are stored under `%LOCALAPPDATA%\RustNotes\`.
4. Run a newer `RustNotes-Setup.exe` over the existing installation; verify notes & settings remain intact.
5. Uninstall via Windows Installed Apps; verify binaries and shortcuts are removed while `%LOCALAPPDATA%\RustNotes\` is preserved unless explicitly opted in during interactive uninstall.
