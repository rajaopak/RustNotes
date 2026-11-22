use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn validate_windows_pe_x64(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed to read '{}': {e}", path.display()))?;
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
        return Err(format!(
            "'{}' is not a valid Windows PE/DLL binary (missing MZ header, {} bytes).",
            path.display(),
            bytes.len()
        ));
    }
    let pe_offset =
        u32::from_le_bytes([bytes[0x3C], bytes[0x3D], bytes[0x3E], bytes[0x3F]]) as usize;
    if pe_offset + 6 > bytes.len() || &bytes[pe_offset..pe_offset + 4] != b"PE\0\0" {
        return Err(format!(
            "'{}' is not a valid Windows PE/DLL binary (missing PE signature).",
            path.display()
        ));
    }
    let machine = u16::from_le_bytes([bytes[pe_offset + 4], bytes[pe_offset + 5]]);
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" && target_arch == "x86_64" && machine != 0x8664 {
        return Err(format!(
            "'{}' has incompatible PE architecture 0x{machine:04X}; expected 0x8664 (x86_64/x64).",
            path.display()
        ));
    }
    Ok(())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=pdfium.dll");

    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => return,
    };
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let is_release = profile == "release";

    let src_dll = manifest_dir.join("pdfium.dll");
    if !src_dll.exists() {
        if is_release {
            panic!(
                "Required runtime dependency '{}' is missing. Release builds cannot succeed without pdfium.dll.",
                src_dll.display()
            );
        }
        println!(
            "cargo:warning=pdfium.dll not found at '{}'; PDF import/export will fail at runtime.",
            src_dll.display()
        );
        return;
    }

    if let Err(err) = validate_windows_pe_x64(&src_dll) {
        if is_release {
            panic!("Invalid pdfium.dll for release build: {err}");
        }
        println!("cargo:warning=Invalid pdfium.dll: {err}");
        return;
    }

    let out_dir = match env::var("OUT_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => return,
    };

    // OUT_DIR is typically: <target-dir>/<profile>/build/<package-hash>/out
    // Going up 3 levels gives <target-dir>/<profile>/ (e.g. target/debug or target/release)
    let mut target_profile_dir = out_dir.clone();
    for _ in 0..3 {
        if let Some(p) = target_profile_dir.parent() {
            target_profile_dir = p.to_path_buf();
        }
    }

    // 1. Copy to target profile folder (e.g. target/release or target/x86_64-pc-windows-msvc/release)
    let dest_profile = target_profile_dir.join("pdfium.dll");
    if let Err(e) = fs::copy(&src_dll, &dest_profile) {
        if is_release {
            panic!(
                "Failed to copy pdfium.dll to '{}': {e}",
                dest_profile.display()
            );
        } else if !dest_profile.exists() {
            println!(
                "cargo:warning=Failed to copy pdfium.dll to {}: {}",
                dest_profile.display(),
                e
            );
        }
    }

    // 2. Copy to target profile deps folder (e.g. target/debug/deps for unit tests)
    let deps_dir = target_profile_dir.join("deps");
    if deps_dir.exists() {
        let dest_deps = deps_dir.join("pdfium.dll");
        if let Err(e) = fs::copy(&src_dll, &dest_deps)
            && !dest_deps.exists()
        {
            println!(
                "cargo:warning=Failed to copy pdfium.dll to {}: {}",
                dest_deps.display(),
                e
            );
        }
    }

    // 3. Also copy to default target/<profile> if CARGO_TARGET_DIR or --target was used
    let default_target = manifest_dir.join("target");
    if default_target.exists() {
        let default_profile_dir = default_target.join(&profile);
        if default_profile_dir.exists() {
            let _ = fs::copy(&src_dll, default_profile_dir.join("pdfium.dll"));
            let default_deps = default_profile_dir.join("deps");
            if default_deps.exists() {
                let _ = fs::copy(&src_dll, default_deps.join("pdfium.dll"));
            }
        }
    }
}
