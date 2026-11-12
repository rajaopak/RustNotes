use super::*;
use crate::model::{
    CURRENT_NOTEBOOK_SCHEMA_VERSION, CURRENT_SCHEMA_VERSION, ColorData, CustomTemplate, DrawObject,
    FontFamilyPreset, Library, LibraryIndex, Notebook, NotebookMeta, Page, PageBackground,
    PageTemplate, StrokePoint, StrokeStyle, TextAlign, ThemeMode,
};
use eframe::egui::Color32;
use std::fs;

#[test]
fn test_migrate_v1_to_v2() {
    let raw = serde_json::json!({
        "version": 1,
        "notebooks": [],
        "folders": []
    });

    let lib = migrate(raw).expect("migrate v1");
    assert_eq!(lib.schema_version, CURRENT_SCHEMA_VERSION);
}

#[test]
fn test_migrate_v1_and_v2_with_legacy_shapes_to_v3() {
    let legacy_json = serde_json::json!({
        "schema_version": 2,
        "notebooks": [
            {
                "id": "nb-legacy-1",
                "title": "Legacy Notebook",
                "cover_color": { "r": 25, "g": 40, "b": 70, "a": 255 },
                "created_at": 1700000000,
                "updated_at": 1700000000,
                "pages": [
                    {
                        "id": "page-1",
                        "width": 800.0,
                        "height": 1100.0,
                        "template": "Blank",
                        "paper_color": { "r": 255, "g": 255, "b": 255, "a": 255 },
                        "bookmarked": false,
                        "objects": [
                            {
                                "type": "Shape",
                                "id": "legacy-shape-1",
                                "shape_type": "Rectangle",
                                "x": 50.0,
                                "y": 60.0,
                                "width": 120.0,
                                "height": 90.0,
                                "color": { "r": 0, "g": 0, "b": 0, "a": 255 },
                                "stroke_width": 2.0,
                                "fill": null
                            }
                        ]
                    }
                ]
            }
        ],
        "folders": []
    });

    let lib = migrate(legacy_json).expect("migrate legacy shape to v3");
    assert_eq!(lib.schema_version, 3);
    assert_eq!(lib.notebooks.len(), 1);
    let page = &lib.notebooks[0].pages[0];
    assert_eq!(page.objects.len(), 1);

    match &page.objects[0] {
        DrawObject::Shape {
            id,
            rotation,
            corner_radius,
            label,
            locked,
            curve_points,
            ..
        } => {
            assert_eq!(id, "legacy-shape-1");
            assert_eq!(*rotation, 0.0);
            assert_eq!(*corner_radius, 0.0);
            assert!(label.is_none());
            assert!(!(*locked));
            assert!(curve_points.is_empty());
        }
        _ => panic!("Expected DrawObject::Shape"),
    }
}

#[test]
fn test_migrate_rejects_future_version() {
    let future_ver = CURRENT_SCHEMA_VERSION + 99;
    let raw = serde_json::json!({
        "schema_version": future_ver,
        "notebooks": [],
        "folders": []
    });

    let res = migrate(raw);
    assert!(res.is_err());
}

#[test]
fn test_storage_normal_save_and_load() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut lib = Library::default();
    lib.notebooks.push(Notebook::new(
        "Temp Note",
        PageTemplate::Grid,
        Color32::from_rgb(10, 20, 30),
    ));

    save_library_to_dir(dir, &lib).expect("save");
    let load_res = load_library_from_dir(dir);

    assert_eq!(load_res.status, LoadStatus::LoadedDirect);
    assert_eq!(load_res.library.notebooks.len(), 1);
    assert_eq!(load_res.library.notebooks[0].title, "Temp Note");
}

#[test]
fn test_storage_recovery_from_previous_when_current_corrupt() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut lib1 = Library::default();
    lib1.notebooks.push(Notebook::new(
        "Version 1 Note",
        PageTemplate::Ruled,
        Color32::from_rgb(10, 20, 30),
    ));
    save_library_to_dir(dir, &lib1).expect("save first");

    let mut lib2 = lib1.clone();
    lib2.notebooks.push(Notebook::new(
        "Version 2 Note",
        PageTemplate::Blank,
        Color32::from_rgb(40, 50, 60),
    ));
    save_library_to_dir(dir, &lib2).expect("save second");

    fs::write(dir.join("library.json"), "{ NOT A VALID JSON :::").expect("write corrupt");

    let load_res = load_library_from_dir(dir);
    assert_eq!(load_res.status, LoadStatus::RecoveredFromPrevious);
    assert_eq!(load_res.library.notebooks.len(), 1);
    assert_eq!(load_res.library.notebooks[0].title, "Version 1 Note");
}

#[test]
fn test_storage_quarantine_when_both_corrupt() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    fs::write(dir.join("library.json"), "CORRUPT JSON 1").expect("write corrupt");
    fs::write(dir.join("library.previous.json"), "CORRUPT JSON 2").expect("write corrupt");

    let load_res = load_library_from_dir(dir);
    match load_res.status {
        LoadStatus::CorruptedQuarantined { path } => {
            assert!(path.exists());
            assert!(path.to_string_lossy().contains("library.corrupt."));
        }
        other => panic!("Expected CorruptedQuarantined, got {:?}", other),
    }

    assert_eq!(load_res.library.notebooks.len(), 0);
}

#[test]
fn test_storage_future_version_readonly_and_no_overwrite() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let future_json = serde_json::json!({
        "schema_version": 999,
        "notebooks": [],
        "folders": []
    });
    fs::write(dir.join("library.json"), future_json.to_string()).expect("write future json");

    let load_res = load_library_from_dir(dir);
    match load_res.status {
        LoadStatus::NewerVersionReadOnly { version } => {
            assert_eq!(version, 999);
        }
        other => panic!("Expected NewerVersionReadOnly, got {:?}", other),
    }

    let save_res = save_library_to_dir(dir, &load_res.library);
    assert!(save_res.is_err());
}

#[test]
fn test_notebook_atomic_save_and_load() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let nb = Notebook::new("Fase 6 Test Book", PageTemplate::Grid, Color32::RED);
    let nb_id = nb.id.clone();

    save_notebook_to_dir(dir, &nb).expect("save notebook");
    assert!(
        dir.join("notebooks")
            .join(&nb_id)
            .join("notebook.json")
            .exists()
    );

    let loaded = load_notebook_from_dir(dir, &nb_id).expect("load notebook");
    assert_eq!(loaded.id, nb_id);
    assert_eq!(loaded.title, "Fase 6 Test Book");
    assert_eq!(loaded.schema_version, CURRENT_NOTEBOOK_SCHEMA_VERSION);
}

#[test]
fn test_notebook_recovery_and_quarantine() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut nb1 = Notebook::new("V1", PageTemplate::Blank, Color32::BLUE);
    let nb_id = nb1.id.clone();
    save_notebook_to_dir(dir, &nb1).expect("save v1");

    nb1.title = "V2".to_string();
    save_notebook_to_dir(dir, &nb1).expect("save v2");

    let nb_file = dir.join("notebooks").join(&nb_id).join("notebook.json");
    fs::write(&nb_file, "{ corrupted JSON...").expect("corrupt current");

    // Should recover from backup
    let rec = load_notebook_from_dir(dir, &nb_id).expect("recover");
    assert_eq!(rec.title, "V1");

    // Now corrupt all backup generations to verify quarantine trigger
    let prev_file = dir
        .join("notebooks")
        .join(&nb_id)
        .join("notebook.previous.json");
    let bak1_file = dir
        .join("notebooks")
        .join(&nb_id)
        .join("notebook.bak.1.json");
    let bak2_file = dir
        .join("notebooks")
        .join(&nb_id)
        .join("notebook.bak.2.json");
    let bak3_file = dir
        .join("notebooks")
        .join(&nb_id)
        .join("notebook.bak.3.json");
    fs::write(&nb_file, "{ bad1").expect("corrupt 1");
    let _ = fs::write(&prev_file, "{ bad2");
    let _ = fs::write(&bak1_file, "{ bad3");
    let _ = fs::write(&bak2_file, "{ bad4");
    let _ = fs::write(&bak3_file, "{ bad5");

    let err = load_notebook_from_dir(dir, &nb_id);
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("quarantined"));
}

#[test]
fn test_notebook_rolling_backup_generations() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut nb = Notebook::new("Gen1", PageTemplate::Blank, Color32::WHITE);
    let nb_id = nb.id.clone();
    save_notebook_to_dir(dir, &nb).expect("save gen1");

    nb.title = "Gen2".to_string();
    save_notebook_to_dir(dir, &nb).expect("save gen2");

    nb.title = "Gen3".to_string();
    save_notebook_to_dir(dir, &nb).expect("save gen3");

    let nb_dir = dir.join("notebooks").join(&nb_id);
    assert!(nb_dir.join("notebook.json").exists());
    assert!(nb_dir.join("notebook.bak.1.json").exists());
    assert!(nb_dir.join("notebook.bak.2.json").exists());
}

#[test]
fn test_load_notebook_future_schema_version_no_quarantine() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut nb = Notebook::new("Future Note", PageTemplate::Blank, Color32::WHITE);
    nb.schema_version = CURRENT_NOTEBOOK_SCHEMA_VERSION + 10;
    let nb_id = nb.id.clone();

    let nb_dir = dir.join("notebooks").join(&nb_id);
    fs::create_dir_all(&nb_dir).expect("mkdir");
    fs::write(
        nb_dir.join("notebook.json"),
        serde_json::to_string(&nb).unwrap(),
    )
    .expect("write future nb");

    let res = load_notebook_from_dir(dir, &nb_id);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("lebih baru"));
    // Berkas tidak boleh dikarantina!
    assert!(nb_dir.join("notebook.json").exists());
}

#[test]
fn test_library_index_migration_from_legacy() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut legacy_lib = Library::default();
    legacy_lib.notebooks.push(Notebook::new(
        "Old Book 1",
        PageTemplate::Ruled,
        Color32::GREEN,
    ));
    legacy_lib.notebooks.push(Notebook::new(
        "Old Book 2",
        PageTemplate::Grid,
        Color32::KHAKI,
    ));
    let nb1_id = legacy_lib.notebooks[0].id.clone();
    let nb2_id = legacy_lib.notebooks[1].id.clone();

    save_library_to_dir(dir, &legacy_lib).expect("save legacy");
    assert!(dir.join("library.json").exists());
    assert!(!dir.join("library_index.json").exists());

    let index_res = load_library_index_from_dir(dir);
    assert_eq!(index_res.status, LoadIndexStatus::MigratedFromLegacy);
    assert_eq!(index_res.index.notebooks.len(), 2);
    assert!(dir.join("library_index.json").exists());
    assert!(!dir.join("library.json").exists());
    assert!(dir.join("backup").exists());

    let nb1 = load_notebook_from_dir(dir, &nb1_id).expect("load nb1");
    assert_eq!(nb1.title, "Old Book 1");
    let nb2 = load_notebook_from_dir(dir, &nb2_id).expect("load nb2");
    assert_eq!(nb2.title, "Old Book 2");
}

#[test]
fn test_notebook_zip_export_and_import() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let nb = Notebook::new("Zip Export Test", PageTemplate::Cornell, Color32::BLUE);
    let nb_id = nb.id.clone();
    save_notebook_to_dir(dir, &nb).expect("save");

    let mut index = LibraryIndex::default();
    index.notebooks.push(NotebookMeta::from_notebook(&nb));
    save_library_index_to_dir(dir, &index).expect("save index");

    let zip_path = dir.join("exported_notebook.zip");
    export_notebook_to_zip(dir, &nb_id, &zip_path).expect("export zip");
    assert!(zip_path.exists());

    let tmp_dest = tempfile::tempdir().expect("tempdir dest");
    let dest_dir = tmp_dest.path();

    let imported = import_notebook_from_zip(dest_dir, &zip_path).expect("import zip");
    assert_eq!(imported.id, nb_id);
    assert_eq!(imported.title, "Zip Export Test");

    let dest_index = load_library_index_from_dir(dest_dir);
    assert_eq!(dest_index.index.notebooks.len(), 1);
    assert_eq!(dest_index.index.notebooks[0].title, "Zip Export Test");
}

#[test]
fn test_import_notebook_from_zip_collision_resolution() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let nb = Notebook::new("Collision Book", PageTemplate::Blank, Color32::BLACK);
    let orig_id = nb.id.clone();
    save_notebook_to_dir(dir, &nb).expect("save initial");

    let zip_path = dir.join("export_collision.zip");
    export_notebook_to_zip(dir, &orig_id, &zip_path).expect("export");

    // Import ke direktori yang SAMA di mana ID sudah ada (Opsi 1A)
    let imported = import_notebook_from_zip(dir, &zip_path).expect("import with collision");
    // ID baru harus dibuat agar tidak menimpa
    assert_ne!(imported.id, orig_id);
    assert_eq!(imported.title, "Collision Book (Imported)");
    // Kedua buku harus ada di disk
    assert!(dir.join("notebooks").join(&orig_id).exists());
    assert!(dir.join("notebooks").join(&imported.id).exists());
}

#[test]
fn test_asset_saving_and_loading() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let nb_id = "test-nb-assets";

    let sample_data = b"FAKE_IMAGE_DATA_123456";
    let asset_id = save_asset_to_notebook(dir, nb_id, sample_data, "png").expect("save asset");
    assert!(asset_id.ends_with(".png"));

    let loaded = load_asset_from_notebook(dir, nb_id, &asset_id).expect("load asset");
    assert_eq!(loaded, sample_data);
}

#[test]
fn test_export_page_to_png() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let nb_id = "png-export-test";

    let mut page = Page::with_size(PageTemplate::Blank, 200.0, 300.0);
    page.objects.push(DrawObject::Stroke {
        id: "s-export-1".to_string(),
        points: vec![
            StrokePoint {
                x: 50.0,
                y: 50.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 150.0,
                y: 50.0,
                pressure: 1.0,
            },
        ],
        color: ColorData {
            r: 220,
            g: 20,
            b: 20,
            a: 255,
        },
        width: 6.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });
    page.objects.push(DrawObject::Text {
        id: "t-export-1".to_string(),
        x: 40.0,
        y: 100.0,
        text: "HELLO".to_string(),
        font_size: 20.0,
        color: ColorData {
            r: 10,
            g: 20,
            b: 200,
            a: 255,
        },
        rotation: 0.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
    });

    let out_png = dir.join("test_page.png");
    export_page_to_png(dir, nb_id, &page, &out_png).expect("export png");
    assert!(out_png.exists());
    let img = image::open(&out_png).expect("load exported png").to_rgba8();
    let px = img.get_pixel(200, 100);
    assert_eq!(px[0], 220);
    assert_eq!(px[1], 20);
    assert_eq!(px[2], 20);
}

#[test]
fn test_export_notebook_to_pdf_and_reimport() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let nb = Notebook::new("PDF Roundtrip", PageTemplate::Blank, Color32::WHITE);
    let out_pdf = dir.join("exported.pdf");

    let export_res = export_notebook_to_pdf(dir, &nb, &out_pdf);
    export_res.expect("Ekspor PDF harus berhasil");
    assert!(out_pdf.exists());
    let pdf_bytes = fs::read(&out_pdf).expect("read exported pdf");
    let imported_pages = import_pdf_to_pages(dir, &nb.id, &pdf_bytes).expect("import pages");
    assert_eq!(imported_pages.len(), nb.pages.len());
    assert!(imported_pages[0].background.is_some());
}

#[test]
fn test_export_pdf_temporary_file_cleanup() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let nb = Notebook::new("Temp Cleanup", PageTemplate::Blank, Color32::WHITE);
    let out_pdf = dir.join("clean.pdf");

    let export_res = export_notebook_to_pdf(dir, &nb, &out_pdf);
    assert!(export_res.is_ok());
    assert!(out_pdf.exists());

    // Pastikan tidak ada berkas *.tmp.* yang tertinggal di direktori tujuan
    let read_dir = fs::read_dir(dir).expect("read_dir");
    for entry in read_dir.flatten() {
        let fname = entry.file_name().to_string_lossy().to_string();
        assert!(
            !fname.contains("tmp."),
            "Berkas sementara tidak boleh tertinggal setelah export: {fname}"
        );
    }
}

#[test]
fn test_export_empty_notebook_to_pdf_rejects() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut nb = Notebook::new("Empty Notebook", PageTemplate::Blank, Color32::WHITE);
    nb.pages.clear();
    let out_pdf = dir.join("empty.pdf");

    let export_res = export_notebook_to_pdf(dir, &nb, &out_pdf);
    assert!(export_res.is_err());
    let err_msg = export_res.unwrap_err();
    assert!(err_msg.contains("tidak memiliki halaman") || err_msg.contains("PDFium"));
}

#[test]
fn test_render_page_to_rgba_image_in_memory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut page = Page::new(PageTemplate::Grid);
    page.width = 400.0;
    page.height = 300.0;

    let res = render_page_to_rgba_image(dir, "nb_mem", &page);
    assert!(res.is_ok());
    let img = res.unwrap();
    // 2x resolution: 400 * 2 = 800, 300 * 2 = 600
    assert_eq!(img.width(), 800);
    assert_eq!(img.height(), 600);
}

#[test]
fn test_custom_template_storage_roundtrip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let tmpl = CustomTemplate {
        id: "tpl-abc-123".to_string(),
        name: "Template Sketsa".to_string(),
        template: PageTemplate::Dotted,
        background: None,
        paper_color: ColorData {
            r: 240,
            g: 240,
            b: 240,
            a: 255,
        },
        width: 800.0,
        height: 1100.0,
    };

    save_custom_template(dir, &tmpl).expect("save custom template");
    let loaded = load_custom_templates(dir);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, tmpl.id);
    assert_eq!(loaded[0].name, tmpl.name);

    delete_custom_template(dir, &tmpl.id).expect("delete custom template");
    let after_del = load_custom_templates(dir);
    assert!(after_del.is_empty());
}

#[test]
fn test_export_and_import_rnote() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let nb = Notebook::new(
        "Catatan Resmi",
        PageTemplate::Ruled,
        Color32::from_rgb(40, 72, 114),
    );
    save_notebook_to_dir(dir, &nb).expect("save notebook");

    let rnote_file = dir.join("catatan.rnote");
    export_notebook_to_rnote(dir, &nb.id, &rnote_file).expect("export rnote");
    assert!(rnote_file.exists());

    let tmp_dest = tempfile::tempdir().expect("tempdir dest");
    let dest_dir = tmp_dest.path();

    let imported = import_notebook_from_rnote(dest_dir, &rnote_file).expect("import rnote");
    assert_eq!(imported.id, nb.id);
    assert_eq!(imported.title, nb.title);
    assert_eq!(imported.pages.len(), nb.pages.len());
}

#[test]
fn test_settings_save_and_load() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let initial = load_settings_from_dir(dir);
    assert_eq!(initial.theme_mode, ThemeMode::Dark);

    let mut custom = initial;
    custom.theme_mode = ThemeMode::Light;
    custom.pen_widths = vec![2.0, 4.0, 6.0];
    save_settings_to_dir(dir, &custom).expect("save settings");

    let reloaded = load_settings_from_dir(dir);
    assert_eq!(reloaded.theme_mode, ThemeMode::Light);
    assert_eq!(reloaded.pen_widths, vec![2.0, 4.0, 6.0]);
}

#[test]
fn test_export_page_to_svg_and_file() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let svg_path = tmp.path().join("test_page.svg");

    let mut page = Page::new(PageTemplate::Ruled);
    page.width = 600.0;
    page.height = 800.0;
    let starter = crate::geometry::generate_diagram_starter(
        crate::geometry::DiagramStarterKind::Flowchart,
        600.0,
        800.0,
    );
    page.objects.extend(starter);

    page.objects.push(DrawObject::Stroke {
        id: uuid::Uuid::new_v4().to_string(),
        points: vec![
            StrokePoint {
                x: 50.0,
                y: 50.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 100.0,
                y: 60.0,
                pressure: 1.0,
            },
        ],
        color: ColorData {
            r: 255,
            g: 0,
            b: 0,
            a: 255,
        },
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });
    page.objects.push(DrawObject::Text {
        id: uuid::Uuid::new_v4().to_string(),
        x: 200.0,
        y: 40.0,
        text: "Header Alur".to_string(),
        font_size: 16.0,
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        rotation: 0.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
    });

    let svg = export_page_to_svg(&page);
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("viewBox=\"0 0 600 800\""));
    assert!(svg.contains("<rect width=\"600\" height=\"800\""));
    assert!(svg.contains("<line x1=\"0\""));
    assert!(svg.contains("<path d=\"M 50.0 50.0 L 100.0 60.0\""));
    assert!(svg.contains("<polyline points="));
    assert!(svg.contains("<polygon points="));
    assert!(svg.contains("Mulai"));
    assert!(svg.contains("Header Alur"));
    assert!(svg.ends_with("</svg>\n"));

    export_page_to_svg_file(&svg_path, &page).expect("export svg file");
    assert!(svg_path.exists());
    let read_back = std::fs::read_to_string(&svg_path).expect("read svg");
    assert_eq!(read_back, svg);
}

#[test]
fn test_duplicate_notebook_storage_without_assets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let orig_nb = Notebook::new("Original Note", PageTemplate::Blank, Color32::WHITE);
    save_notebook_to_dir(dir, &orig_nb).expect("save original");

    let mut clone_nb = orig_nb.clone();
    clone_nb.id = uuid::Uuid::new_v4().to_string();
    clone_nb.title = "Original Note copy".to_string();

    duplicate_notebook_storage(dir, &orig_nb.id, &clone_nb).expect("duplicate");

    let loaded_clone = load_notebook_from_dir(dir, &clone_nb.id).expect("load clone");
    assert_eq!(loaded_clone.id, clone_nb.id);
    assert_eq!(loaded_clone.title, "Original Note copy");
}

#[test]
fn test_duplicate_notebook_storage_with_image_assets() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut orig_nb = Notebook::new("Notebook with Images", PageTemplate::Blank, Color32::WHITE);
    save_notebook_to_dir(dir, &orig_nb).expect("save orig");

    let fake_image_data = b"\x89PNG\r\n\x1a\nfake_image_content";
    let asset_id =
        save_asset_to_notebook(dir, &orig_nb.id, fake_image_data, "png").expect("save asset");

    orig_nb.pages[0].objects.push(DrawObject::Image {
        id: "img_obj_1".to_string(),
        asset_id: asset_id.clone(),
        x: 50.0,
        y: 50.0,
        width: 200.0,
        height: 150.0,
        rotation: 0.0,
        lock_aspect: true,
    });
    save_notebook_to_dir(dir, &orig_nb).expect("save orig with image");

    assert!(validate_notebook_assets(dir, &orig_nb).is_empty());

    let mut clone_nb = orig_nb.clone();
    clone_nb.id = uuid::Uuid::new_v4().to_string();
    clone_nb.title = "Notebook with Images copy".to_string();

    duplicate_notebook_storage(dir, &orig_nb.id, &clone_nb).expect("duplicate with assets");

    let missing = validate_notebook_assets(dir, &clone_nb);
    assert!(
        missing.is_empty(),
        "Clone should not have missing assets, but found: {missing:?}"
    );

    let loaded_bytes =
        load_asset_from_notebook(dir, &clone_nb.id, &asset_id).expect("load asset from clone");
    assert_eq!(loaded_bytes, fake_image_data);
}

#[test]
fn test_duplicate_notebook_storage_isolation_and_delete_original() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut orig_nb = Notebook::new("Temporary Notebook", PageTemplate::Blank, Color32::WHITE);
    save_notebook_to_dir(dir, &orig_nb).expect("save orig");

    let fake_data = b"pdf_content_or_image_12345";
    let asset_id = save_asset_to_notebook(dir, &orig_nb.id, fake_data, "jpg").expect("save asset");

    orig_nb.pages[0].background = Some(PageBackground::Image {
        asset_id: asset_id.clone(),
    });
    save_notebook_to_dir(dir, &orig_nb).expect("save orig");

    let mut clone_nb = orig_nb.clone();
    clone_nb.id = uuid::Uuid::new_v4().to_string();
    clone_nb.title = "Cloned Notebook".to_string();

    duplicate_notebook_storage(dir, &orig_nb.id, &clone_nb).expect("duplicate");

    let orig_dir = dir.join("notebooks").join(&orig_nb.id);
    std::fs::remove_dir_all(&orig_dir).expect("delete orig");
    assert!(!orig_dir.exists());

    let loaded_clone = load_notebook_from_dir(dir, &clone_nb.id).expect("load clone");
    assert_eq!(loaded_clone.title, "Cloned Notebook");

    let clone_asset_bytes = load_asset_from_notebook(dir, &clone_nb.id, &asset_id)
        .expect("load asset from clone after orig deleted");
    assert_eq!(clone_asset_bytes, fake_data);
    assert!(validate_notebook_assets(dir, &clone_nb).is_empty());
}

#[test]
fn test_validate_notebook_assets_detects_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    let mut nb = Notebook::new("Missing Asset Note", PageTemplate::Blank, Color32::WHITE);
    nb.pages[0].objects.push(DrawObject::Image {
        id: "missing_img".to_string(),
        asset_id: "non_existent_hash.png".to_string(),
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        rotation: 0.0,
        lock_aspect: true,
    });

    let missing = validate_notebook_assets(dir, &nb);
    assert_eq!(missing, vec!["non_existent_hash.png".to_string()]);
}

#[test]
fn test_highlighter_underlay_layering_and_mask_export() {
    let mut page = Page::new(PageTemplate::Blank);
    page.width = 400.0;
    page.height = 300.0;

    // First add a pen stroke
    page.objects.push(DrawObject::Stroke {
        id: "pen_1".to_string(),
        points: vec![
            StrokePoint {
                x: 50.0,
                y: 50.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 150.0,
                y: 50.0,
                pressure: 1.0,
            },
        ],
        color: ColorData {
            r: 10,
            g: 10,
            b: 10,
            a: 255,
        },
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });

    // Then add a highlighter stroke (drawn after pen)
    page.objects.push(DrawObject::Stroke {
        id: "hl_1".to_string(),
        points: vec![
            StrokePoint {
                x: 40.0,
                y: 50.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 160.0,
                y: 50.0,
                pressure: 1.0,
            },
        ],
        color: ColorData {
            r: 255,
            g: 220,
            b: 0,
            a: 95,
        },
        width: 15.0,
        is_highlighter: true,
        style: StrokeStyle::Solid,
    });

    // In SVG export, the highlighters group must appear BEFORE the pen stroke
    let svg = export_page_to_svg(&page);
    let hl_idx = svg
        .find("<g id=\"highlighters\">")
        .expect("Must contain highlighters group");
    let pen_idx = svg
        .find("<path d=\"M 50.0 50.0 L 150.0 50.0\"")
        .expect("Must contain pen stroke");
    assert!(
        hl_idx < pen_idx,
        "Highlighters must be rendered in the underlay layer before pen strokes"
    );

    // In PNG export, verify export succeeds without error
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_png = tmp.path().join("hl_test.png");
    export_page_to_png(tmp.path(), "nb_hl", &page, &out_png).expect("PNG export must succeed");
    assert!(out_png.exists());
}

#[test]
fn test_asset_path_traversal_protection() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    // 1. sanitize_path_component harus memblokir traversal
    assert!(super::notebook_store::sanitize_path_component("../../../etc/passwd").is_err());
    assert!(super::notebook_store::sanitize_path_component("..\\..\\windows\\system32").is_err());
    assert!(super::notebook_store::sanitize_path_component("").is_err());
    assert!(super::notebook_store::sanitize_path_component("   ").is_err());
    assert!(super::notebook_store::sanitize_path_component("folder/file.png").is_err());
    assert!(super::notebook_store::sanitize_path_component("folder\\file.png").is_err());

    // Valid components pass
    assert!(super::notebook_store::sanitize_path_component("valid-asset-id_123.png").is_ok());
    assert!(
        super::notebook_store::sanitize_path_component("550e8400-e29b-41d4-a716-446655440000")
            .is_ok()
    );

    // 2. load_asset_from_notebook menolak path traversal
    let res = super::notebook_store::load_asset_from_notebook(dir, "nb_1", "../secret.txt");
    assert!(res.is_err());

    let res_nb = super::notebook_store::load_asset_from_notebook(dir, "../nb_1", "asset.png");
    assert!(res_nb.is_err());

    // 3. save_asset_to_notebook menolak traversal di notebook_id dan extension
    let res_save = super::notebook_store::save_asset_to_notebook(dir, "../nb_1", b"data", "png");
    assert!(res_save.is_err());

    // Extension berbahaya dinetralisir ke alfanumerik aman atau "bin"
    let ok_save =
        super::notebook_store::save_asset_to_notebook(dir, "nb_1", b"data", "../../../evil");
    assert!(ok_save.is_ok());
    let saved_name = ok_save.unwrap();
    assert!(!saved_name.contains('/') && !saved_name.contains('\\') && !saved_name.contains(".."));

    // 4. validate_notebook_assets mendeteksi asset_id berbahaya sebagai missing
    let mut nb = Notebook::new("Test", PageTemplate::Blank, eframe::egui::Color32::WHITE);
    nb.id = "nb_1".to_string();
    let mut page = Page::with_size(PageTemplate::Blank, 200.0, 300.0);
    page.background = Some(PageBackground::Image {
        asset_id: "../../evil.png".to_string(),
    });
    nb.pages.push(page);
    let missing = super::notebook_store::validate_notebook_assets(dir, &nb);
    assert_eq!(missing, vec!["../../evil.png".to_string()]);
}

#[test]
fn test_transactional_archive_import_staging_leaves_no_zombies() {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();

    // Buat berkas zip yang memiliki notebook.json valid, namun berisi asset dengan nama berbahaya (path traversal)
    let bad_zip = dir.join("bad_archive.rnote");
    {
        let file = std::fs::File::create(&bad_zip).expect("create zip");
        let mut zip = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        let mut nb = Notebook::new(
            "Corrupt Import Test",
            PageTemplate::Blank,
            eframe::egui::Color32::WHITE,
        );
        nb.id = "target-nb-id-123".to_string();
        let nb_json = serde_json::to_string(&nb).unwrap();
        zip.start_file("notebook.json", opts).unwrap();
        zip.write_all(nb_json.as_bytes()).unwrap();

        // Tambahkan asset berbahaya yang mencoba path traversal
        zip.start_file("assets/.._.._evil.png", opts).unwrap();
        zip.write_all(b"malicious payload").unwrap();

        zip.finish().unwrap();
    }

    // Eksekusi impor dari arsip berbahaya
    let res = super::archive::import_notebook_from_zip(dir, &bad_zip);
    assert!(
        res.is_err(),
        "Impor arsip dengan nama aset tidak sah harus ditolak"
    );

    // Verifikasi bahwa direktori target notebook TIDAK pernah dibuat atau ditinggalkan di disk
    let nb_target_dir = dir.join("notebooks").join("target-nb-id-123");
    assert!(
        !nb_target_dir.exists(),
        "Transactional staging harus menjamin tidak ada direktori parsial/zombie yang tertinggal di disk jika impor gagal"
    );
}

#[test]
fn test_sticky_note_png_rotation_rendering() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    let nb_id = "sticky_rot_test";

    let mut page = Page::with_size(PageTemplate::Blank, 400.0, 400.0);
    page.paper_color = ColorData {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    // Tambahkan Sticky Note di tengah halaman yang diputar 45 derajat (PI / 4)
    page.objects.push(DrawObject::StickyNote {
        id: "sticky-1".to_string(),
        x: 150.0,
        y: 150.0,
        width: 100.0,
        height: 100.0,
        text: "Catatan".to_string(),
        color: ColorData {
            r: 254,
            g: 226,
            b: 114,
            a: 255,
        },
        text_color: ColorData {
            r: 30,
            g: 30,
            b: 30,
            a: 255,
        },
        font_size: 16.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Center,
        rotation: std::f32::consts::FRAC_PI_4, // 45 derajat
    });

    let img = super::export_png::render_page_to_rgba_image(dir, nb_id, &page)
        .expect("render page to rgba image");

    // Center sticky note adalah Pos2(200, 200). Pada skala 2x: Pos2(400, 400).
    // Pixel pusat harus berwarna kuning Sticky Note (#FEE272 = 254, 226, 114)
    let center_px = img.get_pixel(400, 400);
    assert_eq!(center_px[0], 254);
    assert_eq!(center_px[1], 226);
    assert_eq!(center_px[2], 114);

    // Titik (200, 130) di kanvas (skala 2x: 400, 260) berada di luar kotak unrotated (y: 150..250),
    // tetapi berada di dalam kotak yang berputar 45 derajat (ujung atas berputar ke y = 200 - 50*sqrt(2) ≈ 129.3).
    // Pada skala 2x: y = 200 * 2 - 100*sqrt(2) ≈ 400 - 141.4 = 258.6.
    // Di pixel (400, 280), harus terisi warna Sticky Note karena rotasi!
    let top_rot_px = img.get_pixel(400, 280);
    assert_eq!(
        top_rot_px[0], 254,
        "Pixel rotated top-tip harus terisi warna sticky note kuning"
    );
    assert_eq!(top_rot_px[1], 226);
    assert_eq!(top_rot_px[2], 114);

    // Titik sudut unrotated (155, 155) -> (skala 2x: 310, 310) sekarang berada di LUAR kotak berotasi 45 derajat,
    // sehingga harus berwarna kertas putih murni (255, 255, 255).
    let corner_outside_px = img.get_pixel(310, 310);
    assert_eq!(
        corner_outside_px[0], 255,
        "Corner yang ditinggalkan saat rotasi harus tetap berwarna kertas putih"
    );
    assert_eq!(corner_outside_px[1], 255);
    assert_eq!(corner_outside_px[2], 255);
}
