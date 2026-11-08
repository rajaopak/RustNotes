use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic;
use std::path::PathBuf;

pub fn setup_panic_handler() {
    panic::set_hook(Box::new(|info| {
        let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let log_dir = get_logs_dir();
        let _ = fs::create_dir_all(&log_dir);

        let log_file_path = log_dir.join(format!("crash_{timestamp}.log"));

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "Lokasi tidak diketahui".to_string());

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Pesan panic tidak diketahui".to_string()
        };

        let report = format!(
            "====================================================\n\
             RUSTNOTES CRASH REPORT\n\
             ====================================================\n\
             Waktu: {}\n\
             Aplikasi Versi: {}\n\
             OS Target: Windows 64-bit (x86_64-pc-windows-msvc)\n\
             Lokasi: {}\n\
             Pesan: {}\n\
             ====================================================\n\
             Jika masalah ini berulang, laporkan ke tim pengembang RustNotes\n\
             dengan menyertakan berkas log ini.\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            env!("CARGO_PKG_VERSION"),
            location,
            payload
        );

        // Cetak ke stderr
        eprintln!("{report}");

        // Tulis ke file
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&log_file_path)
        {
            let _ = f.write_all(report.as_bytes());
        }
    }));
}

pub fn get_logs_dir() -> PathBuf {
    if let Some(mut base) = dirs::data_dir() {
        base.push("RustNotes");
        base.push("logs");
        base
    } else {
        PathBuf::from("logs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logs_dir_path() {
        let dir = get_logs_dir();
        assert!(dir.to_string_lossy().contains("logs"));
    }

    #[test]
    fn test_setup_panic_handler_idempotent() {
        setup_panic_handler();
        assert!(!get_logs_dir().as_os_str().is_empty());
    }
}
