use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=pdfium.dll");

    let manifest_dir = match env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => return,
    };

    let src_dll = manifest_dir.join("pdfium.dll");
    if !src_dll.exists() {
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

    // 1. Salin ke target profile folder (target/debug atau target/release)
    let dest_profile = target_profile_dir.join("pdfium.dll");
    if let Err(e) = fs::copy(&src_dll, &dest_profile)
        && !dest_profile.exists()
    {
        println!(
            "cargo:warning=Gagal menyalin pdfium.dll ke {}: {}",
            dest_profile.display(),
            e
        );
    }

    // 2. Salin ke target profile deps folder (target/debug/deps untuk unit tests)
    let deps_dir = target_profile_dir.join("deps");
    if deps_dir.exists() {
        let dest_deps = deps_dir.join("pdfium.dll");
        if let Err(e) = fs::copy(&src_dll, &dest_deps)
            && !dest_deps.exists()
        {
            println!(
                "cargo:warning=Gagal menyalin pdfium.dll ke {}: {}",
                dest_deps.display(),
                e
            );
        }
    }

    // 3. Salin juga ke target default folder jika target di-override oleh CARGO_TARGET_DIR
    let default_target = manifest_dir.join("target");
    if default_target.exists() {
        let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
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
