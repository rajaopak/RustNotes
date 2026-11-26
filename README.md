# 📝 RustNotes

<p align="center">
  <img src="assets/icon.png" alt="RustNotes Logo" width="128" height="128">
</p>

<p align="center">
  <strong>Aplikasi buku catatan digital (digital notebook) berkinerja tinggi yang terinspirasi dari Goodnotes, dibangun 100% menggunakan Pure Rust (Native Desktop GUI).</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust%202024%20Edition-orange?logo=rust" alt="Rust 2024">
  <img src="https://img.shields.io/badge/GUI-eframe%20%2F%20egui%200.31-blue" alt="egui 0.31">
  <img src="https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20(x64)-0078D6?logo=windows" alt="Windows 64-bit">
  <img src="https://img.shields.io/badge/Runtime-Zero%20Web%20Runtime-success" alt="Zero Web Runtime">
  <img src="https://img.shields.io/badge/Tests-312%20Passing-brightgreen" alt="Tests">
</p>

---

## ⚡ Ikhtisar & Keunggulan

Sebagian besar aplikasi catatan modern dibangun menggunakan tumpukan teknologi berbasis web (Electron, Chromium, React, Node.js) yang memakan ratusan megabyte RAM dan sering mengalami latensi *inking*. **RustNotes** dirancang dari nol menggunakan **Pure Rust** dengan pustaka Immediate Mode GUI **`eframe` / `egui 0.31`** yang berkomunikasi langsung dengan GPU melalui painter native, tanpa Electron, Chromium, Node.js, maupun WebView.

### 📊 Perbandingan Metrik: Electron vs RustNotes

| Metrik | Aplikasi Catatan Berbasis Web/Electron | **RustNotes (Pure Rust)** |
|---|:---:|:---:|
| **Ukuran Distribusi** | 150 MB – 300 MB+ | **~9.5 MB** (`rustnotes.exe`) + **~7.5 MB** (`pdfium.dll` x64 untuk PDF) |
| **Konsumsi Memori RAM** | 450 MB – 1.2 GB | **~75 MB – 95 MB** |
| **Waktu Startup** | 2 – 5 detik | **< 0.1 detik (Instan)** |
| **Mesin Render** | Web DOM / HTML Canvas / SVG | **Native GPU Immediate Mode Painter (60–144+ FPS)** |
| **Ketergantungan Runtime** | Node.js, V8 Engine, Chromium, WebView | **Nol Web Runtime** *(hanya `pdfium.dll` BSD-3-Clause di samping `.exe`)* |
| **Undo / Redo Reliability** | Sering desinkronisasi pada state DOM | **Deterministik (Per-Page Immutable Snapshot Stack)** |

---

## ✨ Fitur Utama

### 🖋️ 1. Mesin Inking & 12 Peralatan Kanvas Lengkap
- **12 Alat Aktif:** `Pen`, `Pencil`, `Highlighter`, `Eraser`, `Lasso`, `Shape`, `Connector`, `Text`, `StickyNote`, `Equation` (LaTeX), `Laser`, dan `Hand`.
- **Pen & Pencil:** Interpolasi kurva kuadratik Bézier (*midpoint smoothing*) dengan sensitivitas tekanan stylus yang dapat dikonfigurasi.
- **Highlighter Bebas Tumpukan Gelap (*Overlap-Free*):** Menghasilkan pita semi-transparan rata (*flat chisel ends*) dengan pengaturan opasitas dan opsi sensitivitas tekanan tanpa bintik gelap di persimpangan goresan.
- **Multi-layer Neon Laser Pointer:** Laser interaktif dengan 4 lapisan cahaya neon berlapis, peluruhan halus, serta mode warna **Solid** maupun **Rainbow**.
- **Smart Shape Recognition & Connector Diagram:** Mendukung 9 jenis bentuk (`Rectangle`, `Ellipse`, `Line`, `Triangle`, `RightTriangle`, `ScaleneTriangle`, `Parallelogram`, `Diamond`, `Curve`), konektor `Straight` / `Orthogonal` / `Curved` dengan *connection sites*, *Quick Diagramming* (`Tab` / `Enter`), *Smart Alignment Guides*, dan kunci objek (`Ctrl+L`).
- **Text Tool Presisi:** Inisialisasi bebas border/placeholder mengganggu, dukungan keluarga font (`Sans-Serif`, `Serif`, `Monospace`), gaya **Bold**, *Italic*, *Underline*, dan *Strikethrough*, perataan Kiri/Tengah/Kanan yang akurat di dalam bounding box, serta pengubahan ukuran interaktif yang mulus.
- **Penghapus Cerdas:** Mode penghapus parsial (*Partial Stroke Splitter*) dan penghapus utuh (*Whole Stroke Eraser*) berbasis *spatial hash grid* + *Zoom Window* dengan *auto-advance* dan *Palm Rejection*.

### 🎛️ 2. Floating Quick Tool Bar 4 Sisi (Kontekstual & Fleksibel)
- **Tata Letak 4 Sisi Fleksibel:** Dapat di-*dock* pada sisi **Atas (Top)**, **Bawah (Bottom)**, **Kiri (Left)**, atau **Kanan (Right)** kanvas melalui *drag-to-redock* (dengan pratinjau zona drop di 4 sisi) maupun menu dock cepat.
- **Toggle Tampil / Sembunyi:** Dapat ditampilkan atau disembunyikan kapan saja dari bilah editor, dan preferensi posisi serta visibilitasnya disimpan persisten ke `settings.json`.
- **Kontrol Kontekstual Per-Tool & Anti-Tabrakan:** Menampilkan pengaturan cepat sesuai alat yang aktif (warna, ukuran/ketebalan, tipe garis, bentuk, perataan teks, mode eraser/lasso, mode warna laser, dll.) beserta tombol akses cepat ke pengaturan lengkap tool. Posisi bar otomatis menghindari panel *Side Pages*, kapsul Undo/Page, kapsul Zoom, *Zoom Window*, dan modal/popup.

### 🧮 3. Mesin LaTeX Native (Formula, Tabel, Geometri TikZ, & Grafik 2D/3D)
- **Dukungan LaTeX Kaya Secara Offline:**
  - **Formula Matematika:** Fraksi, akar, eksponen/subskrip, integral/penjumlahan, matriks (`pmatrix`/`bmatrix`/`cases`), simbol Yunani & operator.
  - **Tabel:** Lingkungan `tabular` dan `array` dengan perataan kolom serta garis pemisah sel.
  - **Geometri & Diagram TikZ:** Mendukung `\documentclass{article}`, `\usepackage{pgfplots}`, dan `\begin{tikzpicture}` untuk garis, lingkaran, poligon, kurva, koordinat, dan label.
  - **Grafik 2D & Permukaan 3D PGFPlots:** Mendukung `\begin{axis}` dengan `\addplot` 2D maupun `\addplot3[surf]` 3D lengkap dengan domain $x, y$ dan pewarnaan permukaan.
- **Pratinjau & Transformasi Akurat:** Modal penyunting dengan pratinjau langsung + template cepat, skalabilitas linear proporsional saat di-resize langsung maupun lewat Lasso, serta dukungan penuh pada ekspor PNG, SVG vektor, dan PDF.

### 📐 4. Seleksi Lasso & Transformasi Objek Multi-Halaman
- **Seleksi Fleksibel:** Seleksi bebas maupun persegi panjang dengan kurva lasso (*ray-casting* & deteksi perpotongan CCW).
- **Transformasi Terisolasi & Stabil:** Geser (*move*), skala (*resize*), putar (*rotate*), serta *flip* horizontal/vertikal yang terisolasi bersih per halaman.
- **Manajemen Objek & Clipboard Independen:** Salin, Potong, Tempel, Duplikasi, *Bring to Front / Send to Back*, kunci objek (`Ctrl+L`), serta integrasi tempel gambar dari clipboard OS Windows (`Ctrl+V`).

### 📑 5. Manajemen Halaman, Template, & Organisasi Dokumen
- **Rotasi Penuh Halaman:** Putar halaman 90° CW, 90° CCW, atau 180° — dimensi kertas dan seluruh objek (termasuk LaTeX, teks, shape, konektor, dan gambar) ikut berputar presisi.
- **7 Preset Ukuran Kertas & 10 Template Bawaan:** `A4`, `Letter`, `A5`, `16:9 Slide`, `4:3 Standard`, `Square`, dan `Legacy` (Portrait/Landscape) dengan template `Blank`, `Ruled`, `RuledNarrow`, `RuledWide`, `Grid`, `GridFine`, `Dotted`, `DottedFine`, `Cornell`, dan `Isometric`, plus dukungan *Custom Templates*.
- **Organisasi & Pencarian:** Folder bersarang, *Favorites*, *Trash* dengan pemulihan, pencarian global dan per-buku (`Ctrl+F`) berbasis OCR tulisan tangan lokal, serta UI dwibahasa (**Indonesia / English**) dan tema (**Dark / Light / System**).

### 📦 6. Distribusi Windows (Installer Berbasis Versi & Edisi Portabel)
- **Windows Installer (`RustNotes-Setup.exe`):**
  - Membundel `rustnotes.exe`, pustaka PDF **`pdfium.dll`** (x64), aset ikon, serta asosiasi berkas `.rnote`.
  - **Versi lebih baru:** Otomatis mendeteksi instalasi RustNotes di Windows dan memperbarui aplikasi secara *in-place* tanpa mengganggu atau menghapus catatan pengguna.
  - **Versi sama:** Memberi tahu pengguna bahwa versi tersebut sudah terpasang dan keluar dengan aman (kecuali diminta `/REINSTALL`).
  - **Versi lebih lama (*outdated*):** Menampilkan peringatan dan memblokir instalasi agar pengguna tidak menimpa instalasi baru dengan versi lama yang berisiko memicu bug atau masalah kompatibilitas.
- **Edisi Portabel (`RustNotes-Portable.zip`):**
  - Berjalan mandiri di foldernya sendiri (`portable.txt` + `rustnotes.exe` + `pdfium.dll`) dengan seluruh catatan, pengaturan, template, dan log disimpan di `.\data\` di samping `rustnotes.exe`.

---

## ⌨️ Pintasan Keyboard Default

Seluruh **27 pintasan** dapat dikustomisasi secara bebas melalui dialog pintasan (`F1` atau `?`) dan disimpan ke `settings.json`.

| Pintasan Default | Aksi |
|---|---|
| `Ctrl + Z` / `Ctrl + Y` | Membatalkan (*Undo*) / Mengulang (*Redo*) |
| `Ctrl + S` | Simpan dokumen saat ini secara instan |
| `Ctrl + F` | Buka pencarian teks & OCR di dalam buku catatan |
| `Ctrl + A` | Pilih semua objek di halaman aktif |
| `Ctrl + C` / `Ctrl + X` | Salin (*Copy*) / Potong (*Cut*) objek seleksi |
| `Ctrl + V` | Tempelkan (*Paste*) objek aplikasi atau gambar Windows clipboard |
| `Ctrl + D` | Duplikasi objek seleksi langsung di tempat |
| `Ctrl + L` | Kunci (*Lock*) / Buka kunci (*Unlock*) objek terpilih |
| `Delete` | Hapus objek terpilih |
| `P` / `B` / `H` / `E` | Pen / Pencil / Highlighter / Eraser |
| `L` / `T` / `S` / `C` / `N` | Lasso / Text / Shape / Connector / Sticky Note |
| `Alt + E` / `R` / `Space` | Equation (LaTeX) / Laser Pointer / Hand (Geser Kanvas) |
| `F6` / `Alt + Z` / `Alt + G` | Toggle Panel Halaman / Zoom Window / Grid Snapping |
| `Tab` / `Enter` | *Quick Diagramming* bentuk anak (kanan) / saudara (bawah) |
| `Ctrl + Scroll` / `Ctrl + 0` | Zoom In/Out pada kursor / Reset Zoom 100% |
| `F1` / `?` | Buka dialog pintasan keyboard & panduan |

---

## 🚀 Memulai (Build & Paket Distribusi)

### Prasyarat
- **Sistem Operasi:** Windows 10 atau Windows 11 (64-bit).
- **Rust Toolchain:** Rust edisi 2024 ($\ge 1.85$) target `x86_64-pc-windows-msvc`.
- *(Opsional untuk installer)* **Inno Setup 6** (`ISCC.exe`) untuk mengompilasi `installer/rustnotes.iss`.

### Langkah Kompilasi & Verifikasi

```powershell
git clone https://github.com/rajaopak/RustNotes.git
cd RustNotes/goodnotes-rust

# 1. Verifikasi format, linter, dan seluruh unit/integration tests (312 tests)
cargo fmt --check
cargo clippy -- -D warnings
cargo test

# 2. Kompilasi biner Release (build.rs otomatis memvalidasi & menyalin pdfium.dll x64)
cargo build --release

# 3. Validasi runtime lokal & inisialisasi PDFium
.\target\release\rustnotes.exe --verify-runtime

# 4. (Opsional) Paketkan Edisi Portabel ke dist\RustNotes-Portable.zip
.\package_portable.ps1

# 5. (Opsional, butuh Inno Setup 6) Bangun & uji lifecycle Windows Installer
iscc installer\rustnotes.iss
.\scripts\verify_installer.ps1 -InstallerPath "dist\RustNotes-Setup.exe"
```

Lihat [docs/RELEASE.md](docs/RELEASE.md) untuk rincian lengkap aturan rilis, PDFium, dan kebijakan versi installer.

---

## 🏗️ Struktur Arsitektur Kode

```text
goodnotes-rust/
├── Cargo.toml                      # Konfigurasi dependensi dan profil rilis (LTO)
├── build.rs                        # Validasi PE x64 & penyalinan otomatis pdfium.dll
├── pdfium.dll                      # Pustaka rendering & impor/ekspor PDF (x86_64, BSD-3-Clause)
├── lib/pdfium.dll.lib              # Import library (hanya untuk link build, tidak dibundel)
├── package_portable.ps1            # Skrip pemaketan RustNotes-Portable.zip
├── installer/rustnotes.iss         # Skrip Inno Setup 6 (upgrade/same-version/downgrade guard)
├── scripts/verify_installer.ps1    # Uji otomatis installer (lifecycle, PDFium, & proteksi notes)
├── docs/RELEASE.md                 # Panduan rilis, PDFium, dan kebijakan versi installer
├── .github/workflows/              # CI Windows (ci.yml & windows-release.yml)
├── assets/                         # Aset ikon aplikasi (icon.ico, icon.png)
└── src/
    ├── main.rs                     # Entry point, --verify-runtime, file association, fonts
    ├── lib.rs                      # Root crate library
    ├── model.rs                    # Model domain: Library, Notebook, Page, DrawObject, AppSettings
    ├── crash_handler.rs            # Panic hook otomatis menulis log ke folder data aktif\logs\
    ├── app/                        # Antarmuka Pengguna & Orkestrasi Aplikasi
    │   ├── mod.rs                  # Ekspor & lifecycle frame eframe::App
    │   ├── types.rs                # Enum UI, tab, modal, konteks menu, clipboard queue
    │   ├── state.rs                # State utama RustNotesApp & sinkronisasi penyimpanan
    │   ├── theme.rs                # Tema visual Goodnotes (Dark / Light / System)
    │   ├── cover.rs                # Generator sampul buku dinamis
    │   ├── shortcuts.rs            # Manajemen 27 pintasan keyboard yang dapat dikustomisasi
    │   ├── document_ops.rs         # Operasi duplikasi, pindah, hapus, impor/ekspor halaman
    │   ├── editor_navbar.rs        # Bilah alat atas, kontrol halaman, dan popup pengaturan tool
    │   ├── floating_tool_bar.rs    # Floating Quick Tool Bar 4 sisi (Top/Bottom/Left/Right)
    │   ├── editor_sidebar.rs       # Panel thumbnail halaman dengan drag-drop & aksi massal
    │   ├── editor_view.rs          # Viewport editor kanvas, kapsul Undo/Zoom, & isolasi overlay
    │   ├── library_view.rs         # Galeri rak buku, folder bersarang, Favorites, Trash, pencarian
    │   └── modals/                 # Dialog modular (document_modals, page_modals, settings_modal, mod)
    ├── canvas/                     # Mesin Render Kanvas, Input, & Interaksi
    │   ├── mod.rs                  # Re-export modul canvas
    │   ├── types.rs                # Enum 12 ActiveTool, EraserMode, ResizeCorner, LaserTrailPoint
    │   ├── state.rs                # Undo/Redo stack terisolasi per halaman & cache tekstur/mesh
    │   ├── interaction.rs          # State machine pointer, seleksi lasso, drag/resize terisolasi
    │   ├── sampling.rs             # Pengambilan sampel goresan & filtering stylus
    │   ├── render.rs               # Render kanvas utama & koordinasi layer
    │   ├── primitives.rs           # Render 10 template kertas, teks, gambar, & bayangan
    │   ├── highlighter_renderer.rs # Renderer stabilo overlap-free
    │   ├── gizmo.rs                # Handle transformasi, rotasi pivot, & smart alignment guides
    │   ├── widgets.rs              # Editor teks inline, quick toolbar objek, & modal LaTeX
    │   ├── laser.rs                # Laser pointer neon 4 lapis dengan peluruhan waktu
    │   ├── zoom_window.rs          # Jendela zoom tulis dengan auto-advance
    │   └── tests.rs                # 79 unit test kanvas
    ├── geometry/                   # Kalkulasi Matematika 2D & Mesin LaTeX (Zero UI State Mutation)
    │   ├── mod.rs                  # Re-export modul geometry
    │   ├── primitives.rs           # Bounding box, ray casting, jarak segmen
    │   ├── curves.rs               # Utilitas kurva & interpolasi
    │   ├── tessellation.rs         # Tesselasi pita segitiga goresan & stabilo
    │   ├── highlighter.rs          # Geometri pita stabilo flat-chisel
    │   ├── shapes.rs               # Pengenalan bentuk otomatis (Smart Shape Recognition)
    │   ├── shape_ops.rs            # Pembuatan & geometri 9 jenis bentuk + label
    │   ├── connectors.rs           # Routing konektor, snap site, & quick diagramming
    │   ├── snapping.rs             # Grid snapping & smart alignment guides
    │   ├── eraser.rs               # Pemotongan goresan parsial (Stroke Splitter)
    │   ├── hit_test.rs             # Hit-testing & pengujian objek dalam lasso
    │   ├── transform.rs            # Translasi, scale, rotasi objek, & rotasi halaman penuh
    │   ├── spatial_grid.rs         # Akselerasi spasial grid untuk hit-test cepat
    │   ├── palm_rejection.rs       # Filter penolakan sentuhan telapak tangan
    │   ├── ocr.rs                  # Ekstraksi fitur tulisan tangan offline
    │   ├── math/                   # Mesin LaTeX native (mod, expr, table, tikz/PGFPlots 2D & 3D)
    │   └── tests.rs                # 132 unit test geometri & LaTeX
    └── storage/                    # Subsistem Penyimpanan & Proteksi Integritas Data
        ├── mod.rs                  # Re-export API penyimpanan
        ├── io.rs                   # Atomic write, rolling backup 3 generasi, deteksi Mode Portabel
        ├── library_store.rs        # Indeks library, migrasi skema, & quarantine aman
        ├── notebook_store.rs       # Penyimpanan per-notebook, deduplikasi aset SHA-256, recovery
        ├── archive.rs              # Ekspor/impor arsip ZIP (.rnote / .zip) anti path-traversal
        ├── export_png.rs           # Rasterizer PNG (termasuk grafik/tabel/formula LaTeX)
        ├── export_svg.rs           # Generator vector XML SVG
        ├── export_pdf.rs           # Integrasi PDFium (impor/ekspor PDF & validasi runtime PE)
        ├── templates.rs            # Penyimpanan custom template & AppSettings
        └── tests.rs                # 45 unit test storage, export, & recovery
```

---

## 🛡️ Keamanan & Privasi Data

- **100% Offline & Lokal:** Tidak ada data atau telemetri yang dikirim ke server eksternal.
  - **Installed Mode:** Data disimpan di **`%LOCALAPPDATA%\RustNotes\`** (tidak menulis ke `Program Files`, sehingga aman saat update/reinstall installer).
  - **Portable Mode:** Jika terdapat berkas `portable.txt` (atau `.portable` / folder `data/`) di samping `rustnotes.exe`, seluruh data disimpan lokal di **`.\data\`**.
- **Penyimpanan Atomik & Rolling Backup:** Menggunakan berkas temporer `.tmp` $\rightarrow$ `sync_all` $\rightarrow$ rotasi cadangan `.previous.json` & `.bak.1..3.json` sebelum *rename* atomik (lengkap dengan mekanisme rollback jika file terkunci sementara di Windows).
- **Schema Versioning:** `CURRENT_SCHEMA_VERSION = 3` (Library) dan `CURRENT_NOTEBOOK_SCHEMA_VERSION` dengan penolakan penimpaan berkas dari versi aplikasi yang lebih baru.
- **Ekstraksi Arsip Aman:** Proteksi path traversal (`..`) dan batas ukuran ekstraksi untuk arsip `.rnote`/`.zip`.

---

## 📄 Lisensi

Proyek ini dilisensikan di bawah lisensi terbuka [MIT License](LICENSE).
`pdfium.dll` merupakan perangkat lunak sumber terbuka berlisensi **BSD 3-Clause License**.
