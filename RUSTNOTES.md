# Dokumentasi Lengkap Proyek: RustNotes

> **RustNotes** adalah aplikasi buku catatan digital (*digital notebook workspace*) berkinerja tinggi yang terinspirasi dari **Goodnotes**, dibangun **100% menggunakan bahasa pemrograman Rust** secara murni (*pure native GUI*) tanpa ketergantungan pada Electron, Chromium, NodeJS, maupun WebView.

---

## 1. Ikhtisar & Visi Proyek

Awal mula proyek ini berasal dari eksplorasi aplikasi `goodnotes-dupe` (Paperdesk) yang menggunakan tumpukan teknologi Electron + React + Vite. Versi lama tersebut memiliki berbagai kelemahan:
- **Konsumsi Sumber Daya Tinggi:** Mengonsumsi RAM 400–800 MB dan ukuran instalasi lebih dari 150 MB.
- **Bug Kritis pada Inking:** Pen tools tidak dapat di-*undo/redo* dengan andal karena mutasi referensi di React closure, laser pointer patah-patah tanpa peluruhan animasi, dan lasso tool salah mendeteksi mode geser (*move*) dibanding melingkari objek.

**RustNotes** dibangun ulang secara menyeluruh dari nol menggunakan ekosistem **Pure Rust** dengan pustaka grafis **`eframe` / `egui`** yang langsung berkomunikasi dengan GPU (OpenGL/DirectX/WGPU).

### Keunggulan RustNotes:
| Metrik | Versi Lama (Electron) | **RustNotes (Pure Rust)** |
|---|---|---|
| **Ukuran File Executable** | ~150 MB+ | **5.4 MB** (Single Standalone .exe) |
| **Konsumsi Memori RAM** | 400 MB – 800 MB | **~80 MB – 95 MB** |
| **Waktu Startup** | 2 – 4 detik | **< 0.1 detik (Instan)** |
| **Mesin Render** | Web DOM / Chromium SVG | **Native GPU Painter (60–144 FPS)** |
| **Keandalan Undo/Redo** | Sering desinkronisasi | **Deterministik (Immutable Stack)** |
| **Ketergantungan Eksternal**| Node.js, Electron runtime | **Nol (Zero External Runtime)** |

---

## 2. Struktur Direktori Proyek

Proyek ini berada di folder `D:\rustnotes\goodnotes-rust\` dengan arsitektur kode modular:

```text
goodnotes-rust/
├── Cargo.toml            # Konfigurasi dependensi dan profil rilis Rust
├── src/
│   ├── main.rs           # Entry point aplikasi & konfigurasi native window
│   ├── app.rs            # Manajemen state utama, antarmuka Library, Tab, Modal, & Editor Navbar
│   ├── canvas.rs         # Mesin inking kanvas, state machine peralatan, undo/redo, & render GPU
│   ├── geometry.rs       # Kalkulasi bounding box, point-in-polygon (lasso), hit testing, & transformasi
│   ├── model.rs          # Definisi struktur data (Library, Notebook, Page, DrawObject, Template)
│   └── storage.rs        # Mekanisme penyimpanan lokal atomic write ke %APPDATA%\RustNotes\
└── target/
    └── release/
        └── rustnotes.exe # Biner executable final mandiri untuk Windows (5.4 MB)
```

---

## 3. Rincian Penjelasan Setiap Berkas Sumber

### 3.1 `Cargo.toml`
Mendefinisikan dependensi yang dipilih secara hati-hati agar tetap ramping dan cepat:
- `eframe = "0.31"`: Framework GUI native untuk Rust berbasis `egui`.
- `egui = "0.31"`: Pustaka Immediate Mode GUI berkecepatan tinggi.
- `serde` & `serde_json`: Serialisasi/deserialisasi berkas perpustakaan catatan.
- `rfd = "0.15"` (*Rusty File Dialogs*): Membuka dialog simpan/buka berkas asli Windows (*native dialog*).
- `dirs = "5"`: Mengambil path direktori standar sistem operasi (misalnya `%APPDATA%` di Windows).
- `uuid` & `chrono`: Penomoran identitas unik (UUID v4) dan pencatatan waktu pembaruan (*timestamp*).

---

### 3.2 `src/model.rs` (Data Models)
Menampung seluruh entitas data aplikasi:
- **`PageTemplate`**: Pilihan desain kertas: `Blank`, `Ruled`, `Grid`, `Dotted`, dan `Cornell`.
- **`ShapeKind`**: Jenis bentuk geometris: `Rectangle`, `Ellipse`, `Line`, dan `Triangle`.
- **`StrokePoint`**: Titik goresan pena yang memuat koordinat `x`, `y`, serta `pressure` pena.
- **`ColorData`**: Representasi warna RGBA ter-serialisasi yang mudah dikonversi bolak-balik dengan `egui::Color32`.
- **`DrawObject`**: Enum polimorfik objek gambar pada halaman:
  1. `Stroke`: Goresan pena bebas dengan ketebalan, warna, daftar titik, dan penanda `is_highlighter`.
  2. `Shape`: Bentuk geometri dengan posisi, ukuran, tebal garis, warna garis, dan warna isian opsional.
  3. `Text`: Kotak teks mandiri dengan ukuran font dan warna.
  4. `StickyNote`: Catatan tempel berwarna dengan teks di dalamnya.
- **`Page`**: Mewakili satu lembar catatan: ID unik, dimensi kertas (`800 x 1100`), jenis template, warna dasar kertas, dan daftar objek (`Vec<DrawObject>`).
- **`Notebook`**: Entitas buku catatan: ID, judul, warna sampul, daftar halaman (`Vec<Page>`), penanda favorit, penanda tempat sampah (*trashed*), dan riwayat waktu.
- **`Library`**: Akar struktur data yang menampung seluruh koleksi `notebooks` dan `folders`.

---

### 3.3 `src/geometry.rs` (Geometry & Hit Testing)
Berisi seluruh logika matematika 2D murni:
- **`object_bounds(obj: &DrawObject) -> Rect`**: Menghitung *bounding box* presisi dari objek apa pun (memperhitungkan tebal garis dan padding).
- **`combined_bounds(objects: &[&DrawObject]) -> Option<Rect>`**: Menyatukan beberapa *bounding box* menjadi satu kotak seleksi gabungan.
- **`point_to_segment_dist(p, a, b)`**: Menghitung jarak terpendek dari titik kursor ke ruas garis.
- **`hit_test_object(obj, p, tolerance)`**: Menentukan apakah kursor menyentuh objek (digunakan pada alat penghapus *Eraser*).
- **`point_in_polygon(p, polygon)`**: Algoritma *ray-casting* untuk menguji apakah suatu titik berada di dalam kurva seleksi lasso tertutup.
- **`segments_cross(a, b, c, d)`**: Algoritma orientasi CCW (*counter-clockwise*) untuk mendeteksi apakah garis lasso memotong goresan pena yang panjang.
- **`object_in_lasso(obj, polygon)`**: Algoritma penentu apakah suatu objek terpilih oleh alat lasso.
- **`translate_object(obj, dx, dy)`**: Menggeser posisi objek secara real-time saat dipindahkan.
- **`scale_object(obj, origin, sx, sy)`**: Mengubah ukuran objek berdasarkan titik acuan sudut (*resize handle*).

---

### 3.4 `src/canvas.rs` (Inking & Rendering Engine)
Merupakan inti dari kanvas menggambar dan editor catatan:
- **`CanvasState`**:
  - Menyimpan alat yang sedang aktif (`current_tool`), warna aktif, ketebalan aktif, tingkat pembesaran (`zoom`), dan koordinat geser (*pan*).
  - **Undo & Redo Stacks:** Menggunakan `HashMap<String, Vec<Vec<DrawObject>>>` per halaman. Setiap kali ada penambahan goresan, penghapusan, atau transformasi objek, *snapshot* halaman lama disimpan ke *undo stack*. Ini menjamin operasi `undo()` dan `redo()` bersifat instan, deterministik, dan bebas bug mutasi.
  - **Laser Pointer System:** Jejak partikel titik dengan timestamp `Instant`. Animasi peluruhan (*fade out*) berjalan mulus selama 650 milidetik dengan lingkaran cahaya merah (*glowing head*) di ujung laser.
  - **Lasso State:** Menampung titik-titik kurva lasso dan ID objek yang sedang terpilih.
- **`render_page_canvas()`**:
  - Menangani event input mouse/stylus dengan transformasi ruang koordinat (*Screen Space* $\leftrightarrow$ *Page Space*).
  - Menggambar latar belakang kertas beserta pola grid/garis/titik sesuai template halaman.
  - Merender seluruh objek halaman di layar menggunakan fungsi-fungsi GPU painter bawaan `egui`.
  - Menggambar garis seleksi lasso putus-putus dan kotak *bounding box* seleksi dengan handle pengubah ukuran di sudutnya.

---

### 3.5 `src/app.rs` (Application Shell & UI)
Mengatur seluruh antarmuka pengguna (UI) menyerupai Goodnotes:
- **Goodnotes Visual Theme (`setup_goodnotes_theme`)**:
  - Panel latar: `#1d1b1c`
  - Window modal: `#2d2b2c`
  - Garis pembatas: `#3c3939`
  - Aksen seleksi: Goodnotes Navy `#27365b` dan Blue `#0072ce`
- **Sistem Tab Multi-Dokumen**:
  - Menampilkan deretan tab di bilah paling atas (`#284872`) jika terdapat lebih dari satu buku yang sedang dibuka.
  - Pengguna dapat beralih antar buku atau menutup tab secara instan.
- **Navigasi Sidebar Kiri**:
  - Logo dan nama aplikasi **RustNotes**.
  - Navigasi: *Documents*, *Favorites*, dan *Trash* lengkap dengan penghitung jumlah buku.
  - Tombol utilitas: Ekspor dan Impor backup perpustakaan dalam format JSON.
- **Galeri Dokumen (Library Grid)**:
  - Header: Judul tampilan, kolom pencarian teks instan, dan tombol kapsul *New Notebook* biru.
  - Kartu Sampul Buku Catatan Realistis (**`draw_goodnotes_cover`**):
    - Sudut kanan membulat khas binder Goodnotes.
    - Garis punggung buku (*spine strip*) di sebelah kiri dengan kilau gradasi.
    - Kartu label putih di bagian tengah atas untuk menampilkan judul buku.
    - Garis tanda air (*watermark lines*) di sudut kanan bawah.
    - Ikon bintang emas (*gold star*) jika buku dijadikan favorit.
  - Menu Konteks (Klik Kanan pada Buku): Buka, Tambah/Hapus Favorit, Duplikasi Buku, atau Pindahkan ke Trash.
- **Modal Pembuatan Buku Dua Kolom (*Create Notebook Modal*)**:
  - **Kolom Kiri:** Input nama buku, pratinjau langsung sampul buku, dan pemilihan 8 warna sampul khas Goodnotes.
  - **Kolom Kanan:** Pilihan template kertas (*Ruled Narrow*, *Squared Grid*, *Dotted Paper*, *Blank Paper*, dan *Cornell Notes*).
- **Editor Workspace**:
  - **Bilah Atas Navy (`#284872`):** Tombol kembali ke Library, judul dokumen, ikon-ikon alat gambar, navigasi halaman (halaman sebelumnya, halaman selanjutnya, jumlah halaman, dan tombol tambah halaman), serta tombol Undo/Redo.
  - **Kapsul Kontrol Melayang (*Floating Tool Capsule*):** Pemilihan ketebalan goresan (1.5px, 3px, 5px, 8px), pemilihan warna tinta, sub-menu bentuk geometri, dan tombol zoom kanvas.
  - **Kanvas Utama:** Menampilkan kertas di atas latar meja kerja gelap (`#1d1c1a`) dengan bayangan tepi halus.

---

### 3.6 `src/storage.rs` (Storage Engine)
Mengatur penyimpanan data lokal secara aman di Windows:
- **Lokasi Penyimpanan:** `%APPDATA%\RustNotes\library.json`
- **Mekanisme Atomic Write:**
  - Data JSON diserialisasi terlebih dahulu ke berkas sementara `library.json.tmp`.
  - Jika berkas `library.json` sebelumnya ada, berkas tersebut diduplikasi menjadi `library.previous.json` sebagai snapshot cadangan darurat.
  - Berkas sementara di-*rename* secara atomik ke `library.json`. Hal ini menjamin bahwa jika aplikasi tiba-tiba ditutup atau komputer mati mendadak, data buku catatan tidak akan pernah korup.

---

## 4. Rincian Peralatan Menggambar (Tools Palette)

| Ikon & Nama Alat | Fungsi & Cara Kerja |
|---|---|
| **🖊 Pen (Pena)** | Goresan solid dengan warna dan ketebalan yang dipilih. Mendukung tekanan pena (*pressure sensitivity*). |
| **✏ Pencil (Pensil)** | Goresan bertekstur lembut dengan opasitas sedikit transparan (75%), memberikan sensasi sketsa pensil asli. |
| **🖍 Highlighter (Stabilo)**| Goresan semi-transparan dengan ketebalan lebih lebar. Tulisan tinta atau garis kertas di bawahnya tetap terlihat jelas tanpa tertutup. |
| **🧹 Eraser (Penghapus)** | Penghapus tingkat objek (*object-level hit-testing*). Menyentuhkan kursor ke goresan atau bentuk akan langsung menghapusnya. Hasil hapusan tercatat di riwayat dan dapat di-*undo*. |
| **✂ Lasso Selection** | Menggambar poligon bebas untuk melingkari objek. Objek yang dilingkari akan memunculkan *bounding box* seleksi: dapat digeser (*drag*), diubah ukuran (*corner handles*), atau dihapus via tombol `Delete`/`Backspace`. |
| **⬜ Shapes (Bentuk)** | Menggambar bentuk presisi: Persegi Panjang (*Rectangle*), Lingkaran/Elips (*Circle/Ellipse*), Garis (*Line*), dan Segitiga (*Triangle*). |
| **🔤 Text (Teks)** | Menaruh blok teks di kanvas pada koordinat yang diklik dengan ukuran dan warna yang fleksibel. |
| **📝 Sticky Note** | Membuat catatan tempel berwarna kuning dengan bingkai pembatas untuk menulis memo cepat. |
| **🔴 Laser Pointer** | Jejak sinar laser merah berpendar (*glowing laser head*) dengan ekor yang memudar (*smooth fade out*) dalam 650 ms untuk keperluan presentasi atau penjelasan. |
| **✋ Hand (Pan)** | Menggeser kanvas catatan tanpa meninggalkan goresan gambar. |

---

## 5. Template Kertas yang Tersedia

1. **Ruled Narrow:** Garis-garis horizontal berjarak 30 piksel untuk menulis catatan harian atau tulisan tangan standar.
2. **Squared (Grid):** Pola kotak-kotak berjarak 30 piksel, ideal untuk perhitungan matematika, diagram teknis, atau tabel.
3. **Dotted Paper:** Pola titik-titik halus simetris, sangat cocok untuk *bullet journaling* dan sketsa bebas.
4. **Blank Paper:** Kertas putih polos bersih tanpa garis pembatas.
5. **Cornell Notes:** Desain kertas catatan metode Cornell dengan kolom petunjuk (*cue column*) di sebelah kiri dan area ringkasan (*summary area*) di bagian bawah kertas.

---

## 6. Pintasan Keyboard (Keyboard Shortcuts)

- `Ctrl + Z`: Membatalkan aksi terakhir (*Undo*).
- `Ctrl + Y` atau `Ctrl + Shift + Z`: Mengulangi aksi yang dibatalkan (*Redo*).
- `Delete` atau `Backspace`: Menghapus objek yang sedang dilingkari oleh alat Lasso.
- `Escape`: Menutup jendela dialog modal atau membatalkan seleksi aktif.

---

## 7. Panduan Membangun & Menjalankan (Build & Run)

### Prasyarat
- Sistem Operasi: **Windows 10 / Windows 11 (64-bit)**
- **Rust Toolchain:** `rustc` dan `cargo` (terpasang melalui `rustup`).

### Perintah Kompilasi
Untuk mengompilasi biner rilis dengan optimasi penuh:
```powershell
cd D:\rustnotes\goodnotes-rust
cargo build --release
```

Hasil biner executable mandiri akan terbentuk di:
`D:\rustnotes\goodnotes-rust\target\release\rustnotes.exe`

Aplikasi dapat langsung dijalankan dengan mengeklik dua kali pada file `rustnotes.exe` atau melalui terminal PowerShell:
```powershell
.\target\release\rustnotes.exe
```
Executable ini bersifat **portabel** dan dapat dipindahkan ke komputer Windows mana pun tanpa perlu menginstal Node.js, Electron, atau runtime lainnya.
