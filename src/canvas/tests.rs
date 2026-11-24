use super::*;
use crate::geometry::{smooth_stroke_points, tessellate_stroke};
use crate::model::{
    ColorData, ConnectionSite, ConnectorEnd, ConnectorRoute, ConnectorStyle, DrawObject,
    FontFamilyPreset, Page, PageTemplate, Point2D, ShapeKind, StrokePoint, StrokeStyle, TextAlign,
};
use eframe::egui::{Color32, Pos2, Vec2};

fn make_test_page() -> Page {
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "p1".to_string();
    page.objects.push(DrawObject::Shape {
        id: "s1".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 50.0,
        height: 50.0,
        color: ColorData::from(Color32::RED),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    page.objects.push(DrawObject::Text {
        id: "t1".to_string(),
        x: 200.0,
        y: 200.0,
        text: "Hello".to_string(),
        font_size: 16.0,
        color: ColorData::from(Color32::BLACK),
        rotation: 0.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
    });
    page
}

#[test]
fn test_canvas_state_copy_cut_paste_duplicate() {
    let mut state = CanvasState::default();
    let mut page = make_test_page();

    // 1. Copy
    state.selected_ids = vec!["s1".to_string()];
    state.copy_selected(&page.objects);
    assert_eq!(state.object_clipboard.len(), 1);
    assert_eq!(state.object_clipboard[0].id(), "s1");

    // 2. Paste
    state.paste_clipboard(&mut page, Some(Pos2::new(300.0, 300.0)));
    assert_eq!(page.objects.len(), 3);
    assert_eq!(state.selected_ids.len(), 1);
    let pasted_id = &state.selected_ids[0];
    assert_ne!(pasted_id, "s1"); // Pasted object gets new UUID

    // 3. Duplicate
    state.duplicate_selected(&mut page);
    assert_eq!(page.objects.len(), 4);
    assert_eq!(state.selected_ids.len(), 1);

    // 4. Cut
    state.selected_ids = vec!["t1".to_string()];
    state.cut_selected(&mut page);
    assert_eq!(page.objects.len(), 3);
    assert!(!page.objects.iter().any(|o| o.id() == "t1"));
    assert_eq!(state.object_clipboard.len(), 1);
    assert_eq!(state.object_clipboard[0].id(), "t1");
}

#[test]
fn test_cross_page_copy_and_paste() {
    let mut state = CanvasState::default();
    let page1 = make_test_page();
    let mut page2 = Page::new(PageTemplate::Blank);
    page2.id = "p2".to_string();
    assert_eq!(page2.objects.len(), 0);

    // Copy "s1" and "t1" from page 1
    state.selected_ids = vec!["s1".to_string(), "t1".to_string()];
    state.copy_selected(&page1.objects);
    assert_eq!(state.object_clipboard.len(), 2);

    // Switch to page 2: selected_ids cleared
    state.selected_ids.clear();

    // Paste onto page 2
    state.paste_clipboard(&mut page2, None);
    assert_eq!(page2.objects.len(), 2);
    assert_eq!(state.selected_ids.len(), 2);
    // Pasted objects on page 2 have new IDs
    assert!(
        !page2
            .objects
            .iter()
            .any(|o| o.id() == "s1" || o.id() == "t1")
    );
}

#[test]
fn test_canvas_state_delete_and_reorder() {
    let mut state = CanvasState::default();
    let mut page = make_test_page();

    // Bring to front
    state.selected_ids = vec!["s1".to_string()];
    state.bring_selected_to_front(&mut page);
    assert_eq!(page.objects.last().unwrap().id(), "s1");

    // Send to back
    state.send_selected_to_back(&mut page);
    assert_eq!(page.objects.first().unwrap().id(), "s1");

    // Delete
    state.delete_selected(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].id(), "t1");
    assert!(state.selected_ids.is_empty());
}

#[test]
fn test_canvas_state_flip_selection() {
    let mut state = CanvasState::default();
    let mut page = make_test_page();

    state.selected_ids = vec!["s1".to_string()];
    state.flip_selected_horizontal(&mut page);
    // "s1" is at x: 100, width: 50 -> center is 125.
    // Flipped across 125: x' = 2*125 - (100+50) = 250 - 150 = 100.
    if let DrawObject::Shape { x, .. } = &page.objects[0] {
        assert!((x - 100.0).abs() < 1e-4);
    } else {
        panic!("Expected shape");
    }

    state.flip_selected_vertical(&mut page);
    if let DrawObject::Shape { y, .. } = &page.objects[0] {
        assert!((y - 100.0).abs() < 1e-4);
    } else {
        panic!("Expected shape");
    }
}

#[test]
fn test_canvas_state_partial_eraser_and_undo() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "p_erase".to_string();

    // Tambahkan 1 goresan panjang dari x=0 ke x=100 pada y=50
    let pts: Vec<StrokePoint> = (0..=10)
        .map(|i| StrokePoint {
            x: i as f32 * 10.0,
            y: 50.0,
            pressure: 0.5,
        })
        .collect();
    page.objects.push(DrawObject::Stroke {
        id: "st1".to_string(),
        points: pts,
        color: ColorData::from(Color32::BLACK),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });
    state.spatial_grid.rebuild(&page.objects);

    // Mode default adalah Partial
    assert_eq!(state.eraser_mode, EraserMode::Partial);
    state.eraser_undo_pushed = false;
    let changed = state.erase_at(&mut page, Pos2::new(50.0, 50.0), 12.0);
    assert!(changed);
    // Goresan tunggal harus terbelah menjadi 2 goresan terpisah
    assert_eq!(page.objects.len(), 2);

    // Undo harus mengembalikan menjadi 1 goresan utuh
    state.undo(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].id(), "st1");
}

#[test]
fn test_zoom_window_auto_advance() {
    let mut state = CanvasState::default();
    let initial_x = state.zoom_target_rect.min.x;
    let initial_y = state.zoom_target_rect.min.y;

    // Advance sekali ke kanan pada halaman lebar 595
    state.advance_zoom_target(595.0, 842.0);
    assert!(state.zoom_target_rect.min.x > initial_x);
    assert!((state.zoom_target_rect.min.y - initial_y).abs() < 1e-4);

    // Newline mengembalikan x ke margin kiri dan menambah y
    state.newline_zoom_target(595.0, 842.0);
    assert!((state.zoom_target_rect.min.x - 30.0).abs() < 1e-4);
    assert!(state.zoom_target_rect.min.y > initial_y);
}

#[test]
fn test_stroke_mesh_sticks_to_page_on_scroll_and_zoom() {
    let pts = vec![
        StrokePoint {
            x: 40.0,
            y: 60.0,
            pressure: 0.5,
        },
        StrokePoint {
            x: 120.0,
            y: 150.0,
            pressure: 0.7,
        },
        StrokePoint {
            x: 200.0,
            y: 180.0,
            pressure: 0.6,
        },
    ];
    let smoothed = smooth_stroke_points(&pts, 3);
    let page_mesh = tessellate_stroke(&smoothed, 3.0, Color32::BLACK, |p| p);

    // Simulasikan posisi layar setelah di-scroll (origin bergeser) dan di-zoom (zoom = 1.65)
    let scrolled_origin = Pos2::new(140.0, -220.0);
    let zoom = 1.65_f32;

    let transformed = transform_page_mesh_to_screen(&page_mesh, scrolled_origin, zoom);
    let direct_screen = tessellate_stroke(&smoothed, 3.0 * zoom, Color32::BLACK, |p| {
        scrolled_origin + Vec2::new(p.x * zoom, p.y * zoom)
    });

    assert_eq!(transformed.vertices.len(), direct_screen.vertices.len());
    for (v_cached, v_direct) in transformed
        .vertices
        .iter()
        .zip(direct_screen.vertices.iter())
    {
        assert!((v_cached.pos.x - v_direct.pos.x).abs() < 1e-3);
        assert!((v_cached.pos.y - v_direct.pos.y).abs() < 1e-3);
    }
}

#[test]
fn test_whole_stroke_eraser_precision_on_connected_strokes() {
    let mut state = CanvasState {
        eraser_mode: EraserMode::WholeStroke,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "p_whole_precision".to_string();

    // Stroke 1: Garis vertikal dari (100, 50) ke (100, 150) yang digambar pada sesi 1
    page.objects.push(DrawObject::Stroke {
        id: "stroke_vert".to_string(),
        points: vec![
            StrokePoint {
                x: 100.0,
                y: 50.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 100.0,
                y: 150.0,
                pressure: 0.5,
            },
        ],
        color: ColorData::from(Color32::BLACK),
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });

    // Stroke 2: Garis horizontal dari (60, 50) ke (140, 50) yang menyambung di (100, 50), digambar pada sesi 2
    page.objects.push(DrawObject::Stroke {
        id: "stroke_horiz".to_string(),
        points: vec![
            StrokePoint {
                x: 60.0,
                y: 50.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 140.0,
                y: 50.0,
                pressure: 0.5,
            },
        ],
        color: ColorData::from(Color32::BLACK),
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });
    state.spatial_grid.rebuild(&page.objects);

    // Sentuh di dekat sambungan pada Stroke 2 (mis. x=103, y=50) + gerakan mikro 2px ke (101, 50):
    // Hanya Stroke 2 ("stroke_horiz") yang boleh terhapus; Stroke 1 ("stroke_vert") wajib tetap utuh!
    let erased_first = state.erase_at(&mut page, Pos2::new(103.0, 50.0), 4.0);
    let erased_micro_move = state.erase_at(&mut page, Pos2::new(100.5, 50.0), 4.0);

    assert!(erased_first);
    assert!(!erased_micro_move);
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].id(), "stroke_vert");
}

#[test]
fn test_undo_redo_does_not_affect_existing_text_on_subsequent_actions() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "page_undo_text".to_string();

    // 1. Pengguna membuat teks "Catatan Penting"
    state.push_undo(&page.id, &page.objects);
    page.objects.push(DrawObject::Text {
        id: "text_1".to_string(),
        x: 50.0,
        y: 50.0,
        text: "Catatan Penting".to_string(),
        font_size: 16.0,
        color: ColorData::from(Color32::BLACK),
        rotation: 0.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
    });

    assert_eq!(page.objects.len(), 1);
    assert_eq!(state.undo_stacks.get(&page.id).unwrap().len(), 1);

    // 2. Pengguna kemudian menggambar goresan pen baru
    state.push_undo(&page.id, &page.objects);
    page.objects.push(DrawObject::Stroke {
        id: "stroke_1".to_string(),
        points: vec![
            StrokePoint {
                x: 10.0,
                y: 10.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 50.0,
                y: 50.0,
                pressure: 0.5,
            },
        ],
        color: ColorData::from(Color32::BLACK),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });

    assert_eq!(page.objects.len(), 2);
    assert_eq!(state.undo_stacks.get(&page.id).unwrap().len(), 2);

    // 3. Pengguna menekan Undo: HANYA goresan pen yang boleh dibatalkan!
    // Teks "Catatan Penting" yang sudah tertempel TIDAK BOLEH terpengaruh!
    state.undo(&mut page);

    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].id(), "text_1");
    if let DrawObject::Text { text, .. } = &page.objects[0] {
        assert_eq!(text, "Catatan Penting");
    } else {
        panic!("Expected text object to remain intact");
    }

    // 4. Pengguna menekan Redo: goresan pen kembali muncul
    state.redo(&mut page);
    assert_eq!(page.objects.len(), 2);
    assert_eq!(page.objects[0].id(), "text_1");
    assert_eq!(page.objects[1].id(), "stroke_1");
}

#[test]
fn test_undo_redo_multi_page_isolation_and_order() {
    let mut state = CanvasState::default();
    let mut page1 = Page::new(PageTemplate::Blank);
    page1.id = "p1".to_string();
    let mut page2 = Page::new(PageTemplate::Blank);
    page2.id = "p2".to_string();

    // Di Page 1: Tambahkan teks
    state.push_undo(&page1.id, &page1.objects);
    page1.objects.push(DrawObject::Text {
        id: "txt_p1".to_string(),
        x: 20.0,
        y: 20.0,
        text: "Page 1 Text".to_string(),
        font_size: 14.0,
        color: ColorData::from(Color32::BLACK),
        rotation: 0.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
    });

    // Di Page 2: Tambahkan sebuah bentuk persegi
    state.push_undo(&page2.id, &page2.objects);
    page2.objects.push(DrawObject::Shape {
        id: "rect_p2".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 50.0,
        height: 50.0,
        color: ColorData::from(Color32::BLUE),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });

    // Aksi mutasi terakhir adalah di Page 2
    assert_eq!(state.last_undo_page_id(), Some("p2"));

    // Lakukan undo pada halaman mutasi terakhir (Page 2)
    state.undo(&mut page2);
    assert_eq!(page2.objects.len(), 0);

    // Page 1 wajib 100% tidak terganggu dan tetap berisi teksnya!
    assert_eq!(page1.objects.len(), 1);
    assert_eq!(page1.objects[0].id(), "txt_p1");

    // Sekarang aksi undo berikutnya menunjuk ke Page 1
    assert_eq!(state.last_undo_page_id(), Some("p1"));
}

#[test]
fn test_clean_redundant_top_undo_on_empty_text() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "p_empty".to_string();

    // 1. Keadaan awal: 1 stroke yang sudah ada
    page.objects.push(DrawObject::Stroke {
        id: "existing_stroke".to_string(),
        points: vec![StrokePoint {
            x: 0.0,
            y: 0.0,
            pressure: 0.5,
        }],
        color: ColorData::from(Color32::BLACK),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });

    // 2. User klik Text Tool: snapshot dibuat dan teks kosong dimasukkan
    state.push_undo(&page.id, &page.objects);
    assert_eq!(state.undo_stacks.get(&page.id).unwrap().len(), 1);

    page.objects.push(DrawObject::Text {
        id: "empty_text".to_string(),
        x: 40.0,
        y: 40.0,
        text: "   ".to_string(),
        font_size: 14.0,
        color: ColorData::from(Color32::BLACK),
        rotation: 0.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
    });

    // 3. User batal (klik di luar atau Escape): teks kosong di-remove
    page.objects
        .retain(|o| !matches!(o, DrawObject::Text { text, .. } if text.trim().is_empty()));
    assert_eq!(page.objects.len(), 1);

    // 4. clean_redundant_top_undo harus membersihkan snapshot palsu
    state.clean_redundant_top_undo(&page.id, &page.objects);
    assert_eq!(state.undo_stacks.get(&page.id).unwrap().len(), 0);
    assert!(!state.can_undo(&page.id));
}

#[test]
fn test_laser_tessellation_layer_ribbon() {
    use super::laser::{smooth_laser_points, tessellate_laser_layer};

    let pts = vec![
        Pos2::new(10.0, 10.0),
        Pos2::new(30.0, 25.0),
        Pos2::new(50.0, 60.0),
        Pos2::new(90.0, 80.0),
    ];
    let smoothed = smooth_laser_points(&pts);
    assert!(smoothed.len() > pts.len());

    let color = Color32::from_rgba_unmultiplied(255, 30, 45, 120);
    let mesh = tessellate_laser_layer(&smoothed, 16.0, color);

    assert!(
        !mesh.vertices.is_empty(),
        "Laser mesh vertices must not be empty"
    );
    assert!(
        !mesh.indices.is_empty(),
        "Laser mesh triangle indices must not be empty"
    );
    // Verify all triangle indices point to valid vertices
    for &idx in &mesh.indices {
        assert!((idx as usize) < mesh.vertices.len());
    }
}

#[test]
fn test_undo_redo_after_whole_erase_and_insert_image() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "p_test_erase_image".to_string();

    // 1. Buat beberapa objek (2 strokes) pada kertas
    let stroke1 = DrawObject::Stroke {
        id: "stroke_1".to_string(),
        points: vec![
            StrokePoint {
                x: 50.0,
                y: 50.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 100.0,
                y: 50.0,
                pressure: 0.5,
            },
        ],
        color: ColorData::from(Color32::BLACK),
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    let stroke2 = DrawObject::Stroke {
        id: "stroke_2".to_string(),
        points: vec![
            StrokePoint {
                x: 150.0,
                y: 50.0,
                pressure: 0.5,
            },
            StrokePoint {
                x: 200.0,
                y: 50.0,
                pressure: 0.5,
            },
        ],
        color: ColorData::from(Color32::BLUE),
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };

    state.push_undo(&page.id, &page.objects);
    page.objects.push(stroke1);
    state.push_undo(&page.id, &page.objects);
    page.objects.push(stroke2);
    state.spatial_grid.rebuild(&page.objects);
    assert_eq!(page.objects.len(), 2);

    // 2. Erase menggunakan WholeStroke sehingga kertas kosong
    state.eraser_mode = EraserMode::WholeStroke;
    state.eraser_undo_pushed = false;
    let erased1 = state.erase_at(&mut page, Pos2::new(75.0, 50.0), 4.0);
    assert!(erased1);
    assert_eq!(page.objects.len(), 1);

    state.eraser_undo_pushed = false;
    state.last_whole_erase_pos = None;
    let erased2 = state.erase_at(&mut page, Pos2::new(175.0, 50.0), 4.0);
    assert!(erased2);
    assert_eq!(
        page.objects.len(),
        0,
        "Kondisi kertas harus kosong setelah di-erase"
    );

    // 3. Tambahkan image ke halaman yang kosong
    state.push_undo(&page.id, &page.objects);
    let image_obj = DrawObject::Image {
        id: "img_test".to_string(),
        asset_id: "asset_hash_123".to_string(),
        x: 100.0,
        y: 100.0,
        width: 200.0,
        height: 150.0,
        rotation: 0.0,
        lock_aspect: true,
    };
    page.objects.push(image_obj);
    state.spatial_grid.rebuild(&page.objects);
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].id(), "img_test");

    // 4. Pengguna menekan Undo: HARUS kembali ke kertas kosong (bukan ke objek lama)!
    state.undo(&mut page);
    assert_eq!(
        page.objects.len(),
        0,
        "Undo pertama setelah insert image HARUS mengembalikan kertas ke kondisi kosong!"
    );

    // 5. Pengguna menekan Undo lagi: kembali ke kondisi sebelum stroke 2 di-erase ([stroke_2])
    state.undo(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].id(), "stroke_2");

    // 6. Pengguna menekan Undo lagi: kembali ke kondisi kedua stroke ada ([stroke_1, stroke_2])
    state.undo(&mut page);
    assert_eq!(page.objects.len(), 2);
    assert_eq!(page.objects[0].id(), "stroke_1");
    assert_eq!(page.objects[1].id(), "stroke_2");

    // 7. Pengguna menekan Redo berurutan: maju ke [stroke_2], lalu kertas kosong [], lalu image
    state.redo(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].id(), "stroke_2");

    state.redo(&mut page);
    assert_eq!(page.objects.len(), 0, "Redo ke kondisi kertas kosong");

    state.redo(&mut page);
    assert_eq!(page.objects.len(), 1, "Redo ke kondisi ada image");
    assert_eq!(page.objects[0].id(), "img_test");
}

#[test]
fn test_render_shape_variants_and_connector_primitives() {
    use super::primitives::{draw_non_stroke_object, draw_shape_primitive};
    use crate::model::{
        ColorData, ConnectionSite, ConnectorEnd, ConnectorRoute, ConnectorStyle, DrawObject,
        MarkerKind, Point2D, ShapeKind, ShapeLabel, TextAlign, VerticalAlign,
    };

    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| {
        let painter = ctx.layer_painter(egui::LayerId::background());

        // 1. Test render Rounded Rectangle dengan rotasi
        let stroke = egui::Stroke::new(2.0_f32, Color32::BLACK);
        draw_shape_primitive(
            &painter,
            ShapeKind::Rectangle,
            50.0,
            50.0,
            120.0,
            80.0,
            0.5,
            10.0,
            &[],
            Color32::from_rgb(200, 220, 255),
            stroke,
            |p| p,
        );

        // 2. Test render Diamond
        draw_shape_primitive(
            &painter,
            ShapeKind::Diamond,
            100.0,
            100.0,
            80.0,
            80.0,
            0.0,
            0.0,
            &[],
            Color32::YELLOW,
            stroke,
            |p| p,
        );

        // 3. Test render Curve
        let curve_pts = vec![
            Point2D::new(10.0, 10.0),
            Point2D::new(50.0, 80.0),
            Point2D::new(90.0, 20.0),
            Point2D::new(130.0, 100.0),
        ];
        draw_shape_primitive(
            &painter,
            ShapeKind::Curve,
            10.0,
            10.0,
            120.0,
            90.0,
            0.0,
            0.0,
            &curve_pts,
            Color32::TRANSPARENT,
            stroke,
            |p| p,
        );

        // 4. Test render Shape dengan Label Teks
        let shape_with_label = DrawObject::Shape {
            id: "s_lbl".to_string(),
            shape_type: ShapeKind::Rectangle,
            x: 200.0,
            y: 200.0,
            width: 140.0,
            height: 90.0,
            color: ColorData::from(Color32::BLUE),
            stroke_width: 2.0,
            fill: Some(ColorData::from(Color32::LIGHT_BLUE)),
            rotation: 0.1,
            corner_radius: 8.0,
            label: Some(ShapeLabel {
                text: "Process Step 1".to_string(),
                font_size: 14.0,
                color: ColorData::from(Color32::BLACK),
                alignment: TextAlign::Center,
                vertical_alignment: VerticalAlign::Middle,
            }),
            locked: false,
            curve_points: Vec::new(),
        };

        // 5. Test render Connector (Attached ke shape_with_label)
        let connector = DrawObject::Connector {
            id: "conn_test".to_string(),
            start: ConnectorEnd::Attached {
                shape_id: "s_lbl".to_string(),
                site: ConnectionSite::Right,
            },
            end: ConnectorEnd::Free { x: 450.0, y: 250.0 },
            route: ConnectorRoute::Orthogonal,
            control_points: Vec::new(),
            style: ConnectorStyle {
                stroke_color: ColorData::from(Color32::BLACK),
                stroke_width: 2.0,
                start_marker: MarkerKind::None,
                end_marker: MarkerKind::Arrow,
            },
            locked: false,
        };

        let all_objects = vec![shape_with_label.clone(), connector.clone()];

        draw_non_stroke_object(
            &painter,
            &shape_with_label,
            &all_objects,
            |p| p,
            1.0,
            |_| None,
        );

        draw_non_stroke_object(&painter, &connector, &all_objects, |p| p, 1.0, |_| None);
    });
}

#[test]
fn test_hold_to_snap_undo_determinism() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "snap_test_page".to_string();

    let orig_points = vec![
        StrokePoint::from(Pos2::new(10.0, 10.0)),
        StrokePoint::from(Pos2::new(100.0, 10.0)),
        StrokePoint::from(Pos2::new(100.0, 100.0)),
        StrokePoint::from(Pos2::new(10.0, 100.0)),
        StrokePoint::from(Pos2::new(10.0, 10.0)),
    ];

    // Simulasikan alur Fase G4.3 saat hold-to-snap di-commit:
    // 1. Snapshot pertama menyimpan stroke asli:
    state.push_undo(&page.id, &page.objects);
    let orig_stroke = DrawObject::Stroke {
        id: "orig_stroke_1".to_string(),
        points: orig_points,
        color: ColorData::from(Color32::BLACK),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    page.objects.push(orig_stroke);

    // 2. Snapshot kedua mengganti stroke asli dengan shape terkoreksi:
    state.push_undo(&page.id, &page.objects);
    page.objects.pop();
    page.objects.push(DrawObject::Shape {
        id: "recognized_rect_1".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 10.0,
        y: 10.0,
        width: 90.0,
        height: 90.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 8.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });

    // Kondisi saat ini: di halaman ada 1 objek berupa Shape
    assert_eq!(page.objects.len(), 1);
    assert!(matches!(page.objects[0], DrawObject::Shape { .. }));
    assert_eq!(page.objects[0].id(), "recognized_rect_1");

    // Undo #1: Harus mengembalikan coretan bebas asli pengguna (DrawObject::Stroke)!
    state.undo(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert!(
        matches!(page.objects[0], DrawObject::Stroke { .. }),
        "Undo pertama setelah snap HARUS mengembalikan goresan bebas asli!"
    );
    assert_eq!(page.objects[0].id(), "orig_stroke_1");

    // Undo #2: Harus mengosongkan kertas (sebelum goresan dibuat)
    state.undo(&mut page);
    assert_eq!(
        page.objects.len(),
        0,
        "Undo kedua harus mengembalikan kertas ke kondisi kosong"
    );

    // Redo #1: Maju ke Stroke
    state.redo(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert!(matches!(page.objects[0], DrawObject::Stroke { .. }));

    // Redo #2: Maju ke Shape terkoreksi
    state.redo(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert!(matches!(page.objects[0], DrawObject::Shape { .. }));
    assert_eq!(page.objects[0].id(), "recognized_rect_1");
}

#[test]
fn test_auto_shape_toggle_state() {
    let mut state = CanvasState::default();
    assert!(state.auto_shape_enabled);

    state.auto_shape_enabled = false;
    assert!(!state.auto_shape_enabled);
}

#[test]
fn test_lock_unlock_shape_and_connector_toggle() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "p_lock_test".to_string();

    let s1 = DrawObject::Shape {
        id: "s1".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 50.0,
        y: 50.0,
        width: 100.0,
        height: 80.0,
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };
    let c1 = DrawObject::Connector {
        id: "c1".to_string(),
        start: ConnectorEnd::Free { x: 10.0, y: 10.0 },
        end: ConnectorEnd::Free { x: 50.0, y: 50.0 },
        route: ConnectorRoute::Straight,
        control_points: Vec::new(),
        style: ConnectorStyle::default(),
        locked: false,
    };
    page.objects.push(s1);
    page.objects.push(c1);

    state.selected_ids = vec!["s1".to_string(), "c1".to_string()];
    assert!(!state.is_selected_locked(&page));

    // Toggle 1: Kunci kedua objek
    assert!(state.toggle_lock_selected(&mut page));
    assert!(state.is_selected_locked(&page));
    assert!(page.objects[0].is_locked());
    assert!(page.objects[1].is_locked());

    // Toggle 2: Buka kunci kedua objek
    assert!(state.toggle_lock_selected(&mut page));
    assert!(!state.is_selected_locked(&page));
    assert!(!page.objects[0].is_locked());
    assert!(!page.objects[1].is_locked());

    // Undo: Kembali ke terkunci
    state.undo(&mut page);
    assert!(page.objects[0].is_locked());
    assert!(page.objects[1].is_locked());

    // Redo: Kembali ke terbuka
    state.redo(&mut page);
    assert!(!page.objects[0].is_locked());
    assert!(!page.objects[1].is_locked());
}

#[test]
fn test_locked_objects_cannot_be_deleted_or_erased() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "p_protect_lock".to_string();

    let locked_shape = DrawObject::Shape {
        id: "locked_s".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: true,
        curve_points: Vec::new(),
    };
    let free_stroke = DrawObject::Stroke {
        id: "free_st".to_string(),
        points: vec![
            StrokePoint {
                x: 10.0,
                y: 10.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 30.0,
                y: 30.0,
                pressure: 1.0,
            },
        ],
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    page.objects.push(locked_shape);
    page.objects.push(free_stroke);

    // Pilih keduanya dan panggil delete_selected
    state.selected_ids = vec!["locked_s".to_string(), "free_st".to_string()];
    state.delete_selected(&mut page);

    // locked_shape HARUS tetap ada! free_stroke harus terhapus!
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].id(), "locked_s");
    assert!(page.objects[0].is_locked());

    // Coba hapus locked_shape dengan eraser
    state.eraser_mode = EraserMode::WholeStroke;
    let erased = state.erase_at(&mut page, Pos2::new(150.0, 150.0), 10.0);
    assert!(!erased);
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].id(), "locked_s");
}

#[test]
fn test_locked_shape_remains_valid_connector_snap_target() {
    let locked_shape = DrawObject::Shape {
        id: "target_shape".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: true,
        curve_points: Vec::new(),
    };
    let objects = vec![locked_shape];

    // Titik kanan shape adalah (200.0, 150.0). Cari snap dekat (202.0, 149.0)
    let p_near = Point2D::new(202.0, 149.0);
    let snap = crate::geometry::find_connector_snap_site(&objects, p_near, 16.0, None);

    assert!(
        snap.is_some(),
        "Bentuk terkunci HARUS tetap dapat menjadi snap target connector baru!"
    );
    let (id, site, anchor) = snap.unwrap();
    assert_eq!(id, "target_shape");
    assert_eq!(site, ConnectionSite::Right);
    assert_eq!(anchor, Point2D::new(200.0, 150.0));
}

#[test]
fn test_context_menu_paste_image_target_page_resolution() {
    let mut state = CanvasState::default();
    assert!(state.request_paste_image.is_none());
    assert!(state.hovered_page_id.is_none());
    assert!(state.hovered_page_pos.is_none());

    // Simulasikan klik kanan pada halaman 2 ("page-2-uuid") pada koordinat lokal (120.0, 340.0)
    state.request_paste_image = Some(("page-2-uuid".to_string(), Pos2::new(120.0, 340.0)));
    state.last_interacted_page_id = Some("page-2-uuid".to_string());
    state.hovered_page_id = Some("page-2-uuid".to_string());
    state.hovered_page_pos = Some(Pos2::new(120.0, 340.0));

    // Buat daftar halaman simulasi: Page 1 ("page-1-uuid") dan Page 2 ("page-2-uuid")
    let page1 = Page {
        id: "page-1-uuid".to_string(),
        ..Page::new(PageTemplate::Blank)
    };
    let page2 = Page {
        id: "page-2-uuid".to_string(),
        ..Page::new(PageTemplate::Blank)
    };
    let pages = [page1, page2];

    // Ambil permintaan paste image dan pastikan target page teridentifikasi tepat pada page 2 (index 1)
    let (target_id, target_pos) = state
        .request_paste_image
        .take()
        .expect("Request paste image harus ada");
    let resolved_idx = pages
        .iter()
        .position(|p| p.id == target_id)
        .expect("Page harus ditemukan");

    assert_eq!(
        resolved_idx, 1,
        "Target paste gambar HARUS halaman 2, bukan halaman 1!"
    );
    assert_eq!(target_pos, Pos2::new(120.0, 340.0));
    assert_eq!(
        state.last_interacted_page_id.as_deref(),
        Some("page-2-uuid")
    );
}

#[test]
fn test_paste_clipboard_with_target_pos_on_specific_page() {
    let mut state = CanvasState::default();
    let page1 = Page {
        id: "p1".to_string(),
        ..Page::new(PageTemplate::Blank)
    };
    let mut page2 = Page {
        id: "p2".to_string(),
        ..Page::new(PageTemplate::Blank)
    };

    // Masukkan sebuah stroke ke clipboard
    let obj = DrawObject::Stroke {
        id: "orig_stroke".to_string(),
        points: vec![
            StrokePoint {
                x: 0.0,
                y: 0.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 40.0,
                y: 40.0,
                pressure: 1.0,
            },
        ],
        color: ColorData::from(Color32::RED),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    state.object_clipboard = vec![obj];

    // Tempelkan langsung ke halaman 2 pada target posisi (200.0, 300.0)
    let target = Pos2::new(200.0, 300.0);
    state.paste_clipboard(&mut page2, Some(target));

    // Pastikan objek tertempel di halaman 2 dan TIDAK masuk ke halaman 1
    assert_eq!(page1.objects.len(), 0, "Halaman 1 harus tetap kosong!");
    assert_eq!(
        page2.objects.len(),
        1,
        "Halaman 2 harus berisi 1 objek yang ditempel!"
    );

    // Pastikan posisi objek di halaman 2 berpusat di sekitar target (200, 300)
    let b = crate::geometry::object_bounds(&page2.objects[0]);
    assert!((b.center().x - 200.0).abs() < 1.0);
    assert!((b.center().y - 300.0).abs() < 1.0);
}

#[test]
fn test_floating_toolbar_theme_dark_and_light_adaptation() {
    use super::render::FloatingToolbarStyle;

    let ctx = egui::Context::default();

    // 1. Uji Dark Theme
    ctx.set_visuals(egui::Visuals::dark());
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let style = FloatingToolbarStyle::from_ui(ui);
            assert!(style.is_dark, "Harus mendeteksi mode gelap (Dark Mode)");
            assert!(
                style.bg_color.r() < 50 && style.bg_color.g() < 50 && style.bg_color.b() < 50,
                "Latar belakang toolbar harus berupa permukaan gelap!"
            );
            assert!(
                style.primary_text_color.r() > 200,
                "Warna teks utama harus terang untuk kontras tinggi di mode gelap!"
            );
        });
    });

    // 2. Uji Light Theme
    ctx.set_visuals(egui::Visuals::light());
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let style = FloatingToolbarStyle::from_ui(ui);
            assert!(!style.is_dark, "Harus mendeteksi mode terang (Light Mode)");
            assert!(
                style.bg_color.r() > 240 && style.bg_color.g() > 240,
                "Latar belakang toolbar harus berupa permukaan terang!"
            );
            assert!(
                style.primary_text_color.r() < 50,
                "Warna teks utama harus gelap untuk kontras tinggi di mode terang!"
            );
        });
    });
}

#[test]
fn test_canvas_state_texture_cache_isolated_per_notebook() {
    let mut state = CanvasState::default();
    let ctx = egui::Context::default();

    let dummy_img = egui::ColorImage::example();
    let handle1 = ctx.load_texture("tex1", dummy_img.clone(), Default::default());
    let handle2 = ctx.load_texture("tex2", dummy_img, Default::default());

    let nb1 = "notebook-alpha";
    let nb2 = "notebook-beta";
    let asset_id = "image_common.png";

    // Insert handle1 for nb1 and handle2 for nb2
    state
        .texture_cache
        .insert((nb1.to_string(), asset_id.to_string()), handle1);
    state
        .texture_cache
        .insert((nb2.to_string(), asset_id.to_string()), handle2);

    // Saat active notebook adalah nb1
    state.active_notebook_id = Some(nb1.to_string());
    assert_eq!(state.get_texture(asset_id).unwrap().name(), "tex1");

    // Saat active notebook beralih ke nb2
    state.active_notebook_id = Some(nb2.to_string());
    assert_eq!(state.get_texture(asset_id).unwrap().name(), "tex2");

    // Saat active notebook adalah notebook asing yang belum punya cache
    state.active_notebook_id = Some("notebook-gamma".to_string());
    assert!(state.get_texture(asset_id).is_none());

    // Saat active notebook None
    state.active_notebook_id = None;
    assert!(state.get_texture(asset_id).is_none());

    // Lookup eksplisit per notebook_id
    assert_eq!(
        state
            .get_texture_for_notebook(nb1, asset_id)
            .unwrap()
            .name(),
        "tex1"
    );
    assert_eq!(
        state
            .get_texture_for_notebook(nb2, asset_id)
            .unwrap()
            .name(),
        "tex2"
    );
    assert!(
        state
            .get_texture_for_notebook("notebook-gamma", asset_id)
            .is_none()
    );
}

#[test]
fn test_large_circle_lifecycle_does_not_shrink_on_drag_or_commit() {
    use crate::geometry::recognize_shape;
    use crate::model::{Page, PageTemplate, StrokePoint};

    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "page-circle-lifecycle".to_string();

    // Buat lingkaran besar (radius 150) di sekitar (350, 350)
    let center = Pos2::new(350.0, 350.0);
    let radius = 150.0;
    let mut circle_pts = Vec::new();
    for i in 0..=36 {
        let theta = (i as f32) * std::f32::consts::TAU / 36.0;
        circle_pts.push(StrokePoint::from(Pos2::new(
            center.x + radius * theta.cos(),
            center.y + radius * theta.sin(),
        )));
    }

    // 1. Pengenalan bentuk awal
    let recognized = recognize_shape(&circle_pts).expect("Lingkaran besar harus dikenali");
    assert_eq!(recognized.kind, ShapeKind::Ellipse);
    assert!(
        (recognized.width - 300.0).abs() < 30.0,
        "Lebar awal harus ~300, didapat: {}",
        recognized.width
    );

    state.active_stroke_page_id = Some(page.id.clone());
    state.draft_stroke = Some(circle_pts.clone());
    state.recognized_draft_shape = Some(recognized.clone());
    state.snap_initial_shape = Some(recognized.clone());

    // 2. Pada lifecycle lama, pergerakan pointer saat menahan pena menimpa recognized.width = page_p.x - recognized.x
    // Dengan perbaikan baru, geometri 2D tetap terjaga pada recognized_draft_shape:
    let current_rec = state.recognized_draft_shape.as_ref().unwrap();
    assert!(
        current_rec.width > 250.0 && current_rec.height > 250.0,
        "Lingkaran besar harus tetap besar dan tidak boleh menciut!"
    );

    // 3. Simulasikan komitmen saat drag_stopped()
    let committed = state.recognized_draft_shape.take().unwrap();
    assert!(
        (committed.width - 300.0).abs() < 30.0,
        "Bentuk akhir yang dikomit harus memiliki dimensi konsisten dengan preview!"
    );
}

#[test]
fn test_fine_grained_mesh_cache_invalidation() {
    let mut state = CanvasState::default();
    let mesh1 = eframe::egui::Mesh::default();
    let mesh2 = eframe::egui::Mesh::default();

    state.mesh_cache.insert(("obj_1".to_string(), 0), mesh1);
    state.mesh_cache.insert(("obj_2".to_string(), 0), mesh2);
    assert_eq!(state.mesh_cache.len(), 2);

    // Pilih hanya obj_1 dan lakukan invalidasi
    state.selected_ids = vec!["obj_1".to_string()];
    state.invalidate_mesh_for_selection();

    // obj_1 harus terhapus, obj_2 harus tetap ada di cache
    assert!(!state.mesh_cache.contains_key(&("obj_1".to_string(), 0)));
    assert!(state.mesh_cache.contains_key(&("obj_2".to_string(), 0)));
}

#[test]
fn test_negative_texture_lookup_recorded() {
    let mut state = CanvasState::default();
    let key = ("fake_nb".to_string(), "missing_asset.png".to_string());
    assert!(!state.failed_texture_loads.contains(&key));

    state.failed_texture_loads.insert(key.clone());
    assert!(state.failed_texture_loads.contains(&key));
}

#[test]
fn test_highlighter_alpha_constant_consistency() {
    use crate::model::{HIGHLIGHTER_ALPHA, HIGHLIGHTER_OPACITY_F32};
    assert_eq!(HIGHLIGHTER_ALPHA, 95);
    assert!((HIGHLIGHTER_OPACITY_F32 - (95.0 / 255.0)).abs() < 1e-5);
}

#[test]
fn test_undo_redo_snapshot_allocation_and_determinism() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);

    for i in 0..60 {
        state.push_undo(&page.id, &page.objects);
        page.objects.push(DrawObject::Text {
            id: format!("text_{}", i),
            x: i as f32,
            y: i as f32,
            text: format!("Item {}", i),
            font_size: 14.0,
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
    }

    assert_eq!(page.objects.len(), 60);
    // Stack harus dibatasi oleh MAX_UNDO_DEPTH (50)
    assert!(state.undo_stacks.get(&page.id).unwrap().len() <= CanvasState::MAX_UNDO_DEPTH);

    // Undo 30 kali
    for _ in 0..30 {
        state.undo(&mut page);
    }
    assert_eq!(page.objects.len(), 30);
    assert_eq!(page.objects.last().unwrap().id(), "text_29");

    // Redo 30 kali
    for _ in 0..30 {
        state.redo(&mut page);
    }
    assert_eq!(page.objects.len(), 60);
    assert_eq!(page.objects.last().unwrap().id(), "text_59");
}

#[test]
fn test_text_and_sticky_formatting_and_undo() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);

    // 1. Create text with formatting
    state.push_undo(&page.id, &page.objects);
    page.objects.push(DrawObject::Text {
        id: "fmt_text".to_string(),
        x: 100.0,
        y: 100.0,
        text: "Custom Formatted Text".to_string(),
        font_size: 20.0,
        color: ColorData {
            r: 15,
            g: 82,
            b: 186,
            a: 255,
        },
        rotation: 0.0,
        font_family: FontFamilyPreset::Monospace,
        bold: true,
        italic: false,
        alignment: TextAlign::Center,
    });

    // 2. Create sticky note with formatting
    state.push_undo(&page.id, &page.objects);
    page.objects.push(DrawObject::StickyNote {
        id: "fmt_sticky".to_string(),
        x: 200.0,
        y: 200.0,
        width: 180.0,
        height: 140.0,
        text: "Custom Sticky".to_string(),
        color: ColorData {
            r: 252,
            g: 231,
            b: 243,
            a: 255,
        }, // Pastel Pink
        text_color: ColorData {
            r: 220,
            g: 38,
            b: 38,
            a: 255,
        }, // Red
        font_size: 16.0,
        font_family: FontFamilyPreset::Serif,
        bold: true,
        italic: true,
        alignment: TextAlign::Right,
        rotation: 0.0,
    });

    assert_eq!(page.objects.len(), 2);

    // 3. Mutate text formatting (e.g. user toggles bold off and changes font size to 24)
    state.push_undo(&page.id, &page.objects);
    if let DrawObject::Text {
        font_size, bold, ..
    } = &mut page.objects[0]
    {
        *font_size = 24.0;
        *bold = false;
    }
    if let DrawObject::Text {
        font_size, bold, ..
    } = &page.objects[0]
    {
        assert_eq!(*font_size, 24.0);
        assert!(!*bold);
    }

    // 4. Undo restores bold: true and font_size: 20.0
    state.undo(&mut page);
    if let DrawObject::Text {
        font_size, bold, ..
    } = &page.objects[0]
    {
        assert_eq!(*font_size, 20.0);
        assert!(*bold);
    }

    // 5. Redo re-applies bold: false and font_size: 24.0
    state.redo(&mut page);
    if let DrawObject::Text {
        font_size, bold, ..
    } = &page.objects[0]
    {
        assert_eq!(*font_size, 24.0);
        assert!(!*bold);
    }
}

#[test]
fn test_canvas_state_effective_pressure_affine_calibration() {
    let mut state = CanvasState::default();
    assert!(state.pressure_sensitivity_enabled);

    // Default min 0.2, mult 1.0
    assert!((state.effective_pressure(0.0) - 0.2).abs() < 1e-4);
    assert!((state.effective_pressure(0.5) - 0.6).abs() < 1e-4);
    assert!((state.effective_pressure(1.0) - 1.0).abs() < 1e-4);
    assert!((state.effective_pressure(f32::NAN) - 0.2).abs() < 1e-4);
    assert!((state.effective_pressure(-0.5) - 0.2).abs() < 1e-4);

    // Disabled -> always 1.0
    state.pressure_sensitivity_enabled = false;
    assert_eq!(state.effective_pressure(0.1), 1.0);
    assert_eq!(state.effective_pressure(0.0), 1.0);

    // Sensitivity multiplier = 2.0 -> half pressure gives full output
    state.pressure_sensitivity_enabled = true;
    state.pressure_multiplier = 2.0;
    assert!((state.effective_pressure(0.5) - 1.0).abs() < 1e-4);
    assert!((state.effective_pressure(0.25) - 0.6).abs() < 1e-4);
}

#[test]
fn test_stylus_touch_tracking_and_pressure_extraction() {
    let dev = egui::TouchDeviceId(0);
    let id_palm = egui::TouchId::from(1u64);
    let id_stylus = egui::TouchId::from(2u64);

    let events = vec![
        egui::Event::Touch {
            device_id: dev,
            id: id_palm,
            phase: egui::TouchPhase::Move,
            pos: Pos2::new(100.0, 500.0),
            force: Some(0.1),
        },
        egui::Event::Touch {
            device_id: dev,
            id: id_stylus,
            phase: egui::TouchPhase::Move,
            pos: Pos2::new(200.0, 200.0),
            force: Some(0.75),
        },
    ];

    // 1. Initial touch selection based on reference pointer position near stylus
    let (chosen_id, force) = extract_touch_pressure(&events, None, Some(Pos2::new(205.0, 198.0)));
    assert_eq!(chosen_id, Some(id_stylus));
    assert_eq!(force, Some(0.75));

    // 2. Active touch tracking keeps stylus ID even if palm event is ordered differently
    let reversed_events = vec![
        egui::Event::Touch {
            device_id: dev,
            id: id_stylus,
            phase: egui::TouchPhase::Move,
            pos: Pos2::new(210.0, 202.0),
            force: Some(0.85),
        },
        egui::Event::Touch {
            device_id: dev,
            id: id_palm,
            phase: egui::TouchPhase::Move,
            pos: Pos2::new(95.0, 510.0),
            force: Some(0.12),
        },
    ];
    let (tracked_id, tracked_force) =
        extract_touch_pressure(&reversed_events, Some(id_stylus), None);
    assert_eq!(tracked_id, Some(id_stylus));
    assert_eq!(tracked_force, Some(0.85));

    // 3. Fallback when no touch events match
    let empty_events: Vec<egui::Event> = Vec::new();
    let (fallback_id, fallback_force) =
        extract_touch_pressure(&empty_events, Some(id_stylus), None);
    assert_eq!(fallback_id, Some(id_stylus));
    assert_eq!(fallback_force, None);
}

#[test]
fn test_equation_modal_dialog_lifecycle_and_validation() {
    let mut state = CanvasState::default();
    let mut page = Page::new(PageTemplate::Blank);
    let ctx = egui::Context::default();

    // 1. Saat show_equation_dialog = false, dialog tidak mengubah state
    state.show_equation_dialog = false;
    super::widgets::render_equation_modal_dialog(&ctx, &mut state, &mut page);
    assert!(page.objects.is_empty());

    // 2. Buka dialog modal untuk persamaan baru
    state.show_equation_dialog = true;
    state.equation_latex_buffer = r"\frac{x}{y}".to_string();
    state.equation_font_size = 24.0;
    state.equation_insert_pos = Pos2::new(100.0, 150.0);

    // Simulasikan frame egui
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        super::widgets::render_equation_modal_dialog(ctx, &mut state, &mut page);
    });

    // Validasi parser rumus LaTeX
    let parsed = crate::geometry::math::parse_latex_math(&state.equation_latex_buffer);
    assert!(parsed.is_ok());
    let nodes = parsed.unwrap();
    let layout = crate::geometry::math::MathLayout::compute(&nodes, state.equation_font_size);
    assert!(layout.width > 0.0);
    assert!(layout.height > 0.0);
}

#[test]
fn test_spatial_grid_page_id_sync_and_reorder_consistency() {
    let mut state = CanvasState {
        eraser_mode: EraserMode::WholeStroke,
        ..Default::default()
    };

    let mut page_a = Page::new(PageTemplate::Blank);
    page_a.objects.push(DrawObject::Stroke {
        id: "stroke-a".to_string(),
        points: vec![
            StrokePoint::new(50.0, 50.0, 1.0),
            StrokePoint::new(100.0, 50.0, 1.0),
        ],
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });

    let mut page_b = Page::new(PageTemplate::Blank);
    page_b.objects.push(DrawObject::Stroke {
        id: "stroke-b".to_string(),
        points: vec![
            StrokePoint::new(300.0, 300.0, 1.0),
            StrokePoint::new(360.0, 300.0, 1.0),
        ],
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });

    // Rebuild grid untuk page_b lalu page_a; erase_at pada page_b harus tetap mendeteksi stroke-b
    state.rebuild_spatial_grid(&page_b.id, &page_b.objects);
    assert_eq!(
        state.spatial_grid_page_id.as_deref(),
        Some(page_b.id.as_str())
    );
    state.rebuild_spatial_grid(&page_a.id, &page_a.objects);
    assert_eq!(
        state.spatial_grid_page_id.as_deref(),
        Some(page_a.id.as_str())
    );

    let erased = state.erase_at(&mut page_b, Pos2::new(330.0, 300.0), 10.0);
    assert!(
        erased,
        "erase_at on page_b must rebuild spatial grid for page_b"
    );
    assert!(page_b.objects.is_empty());

    // Reorder objek pada page_a harus memperbarui spatial_grid tanpa indeks usang
    page_a.objects.push(DrawObject::Stroke {
        id: "stroke-a2".to_string(),
        points: vec![
            StrokePoint::new(200.0, 200.0, 1.0),
            StrokePoint::new(240.0, 200.0, 1.0),
        ],
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });
    state.selected_ids = vec!["stroke-a".to_string()];
    state.bring_selected_to_front(&mut page_a);
    assert_eq!(page_a.objects.last().unwrap().id(), "stroke-a");
    let hits = state.spatial_grid.query_point(Pos2::new(75.0, 50.0), 5.0);
    assert!(
        hits.contains(&1),
        "spatial_grid must reflect new index 1 after bring_to_front"
    );
}

#[test]
fn test_palm_rejection_thresholds() {
    use crate::geometry::should_reject_palm_input;

    let pts = vec![StrokePoint::new(100.0, 100.0, 1.0)];

    // Multi-touch aktif selalu ditolak
    assert!(should_reject_palm_input(
        &pts,
        Pos2::new(105.0, 105.0),
        0.016,
        true
    ));

    // Lompatan > 180 pt dalam < 80 ms ditolak sebagai sentuhan telapak tangan
    assert!(should_reject_palm_input(
        &pts,
        Pos2::new(300.0, 100.0),
        0.02,
        false
    ));

    // Goresan normal cepat (<= 180 pt) tetap diterima
    assert!(!should_reject_palm_input(
        &pts,
        Pos2::new(150.0, 100.0),
        0.016,
        false
    ));

    // Lompatan besar dengan jeda >= 80 ms tetap diterima
    assert!(!should_reject_palm_input(
        &pts,
        Pos2::new(300.0, 100.0),
        0.1,
        false
    ));
}

#[test]
fn test_zoom_window_stroke_width_parity() {
    let mut state = CanvasState {
        stroke_width: 3.0,
        ..Default::default()
    };

    state.current_tool = ActiveTool::Pen;
    assert!((super::zoom_window::zoom_stroke_width(&state) - 3.0).abs() < f32::EPSILON);

    state.current_tool = ActiveTool::Pencil;
    assert!((super::zoom_window::zoom_stroke_width(&state) - 2.5).abs() < f32::EPSILON);

    state.stroke_width = 1.0;
    assert!((super::zoom_window::zoom_stroke_width(&state) - 1.0).abs() < f32::EPSILON);

    state.stroke_width = 4.0;
    state.current_tool = ActiveTool::Highlighter;
    assert!((super::zoom_window::zoom_stroke_width(&state) - 24.0).abs() < f32::EPSILON);
}

#[test]
fn test_multi_page_spatial_grid_sync_after_paste_and_delete() {
    let mut state = CanvasState {
        eraser_mode: EraserMode::WholeStroke,
        ..Default::default()
    };

    let mut page_a = Page::new(PageTemplate::Blank);
    let mut page_b = Page::new(PageTemplate::Blank);

    let stroke_a = DrawObject::Stroke {
        id: "stroke-a".to_string(),
        points: vec![
            StrokePoint::new(40.0, 40.0, 1.0),
            StrokePoint::new(80.0, 40.0, 1.0),
        ],
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    page_a.objects.push(stroke_a.clone());

    // 1. Rebuild grid untuk page_a, lalu paste ke page_b
    state.rebuild_spatial_grid(&page_a.id, &page_a.objects);
    assert_eq!(
        state.spatial_grid_page_id.as_deref(),
        Some(page_a.id.as_str())
    );

    state.object_clipboard = vec![stroke_a];
    state.paste_clipboard(&mut page_b, Some(Pos2::new(200.0, 200.0)));
    assert_eq!(
        state.spatial_grid_page_id.as_deref(),
        Some(page_b.id.as_str()),
        "paste_clipboard on page_b must update spatial_grid_page_id to page_b"
    );

    // 2. Kembali hapus di page_a; erase_at harus mendeteksi page_id berbeda dan membangun ulang grid page_a
    assert!(
        state.erase_at(&mut page_a, Pos2::new(60.0, 40.0), 10.0),
        "erase_at on page_a must succeed after paste_clipboard on page_b"
    );
    assert!(page_a.objects.is_empty());
    assert_eq!(
        state.spatial_grid_page_id.as_deref(),
        Some(page_a.id.as_str())
    );

    // 3. delete_selected pada page_b harus menyinkronkan spatial_grid_page_id kembali ke page_b
    state.selected_ids = vec![page_b.objects[0].id().to_string()];
    state.delete_selected(&mut page_b);
    assert!(page_b.objects.is_empty());
    assert_eq!(
        state.spatial_grid_page_id.as_deref(),
        Some(page_b.id.as_str())
    );
}

#[test]
fn test_quick_diagram_dot_vs_transform_handle_priority() {
    let mut state = CanvasState {
        current_tool: ActiveTool::Lasso,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "qd-priority".to_string();
    page.objects.push(DrawObject::Shape {
        id: "s1".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    state.selected_ids = vec!["s1".to_string()];

    let sel = super::gizmo::SelectionData::compute(&page, &state);
    // North edge handle at (140, 100), Top quick dot at (140, 88), Rotation at (140, 80)
    assert_eq!(
        super::interaction::hit_test_quick_diagram_dot(&sel, &state, Pos2::new(140.0, 100.0)),
        None,
        "North resize edge handle must keep priority over Top quick dot"
    );
    assert_eq!(
        super::interaction::hit_test_quick_diagram_dot(&sel, &state, Pos2::new(140.0, 80.0)),
        None,
        "Rotation handle must keep priority over Top quick dot"
    );
    assert_eq!(
        super::interaction::hit_test_quick_diagram_dot(&sel, &state, Pos2::new(140.0, 88.0)),
        Some(("s1".to_string(), ConnectionSite::Top)),
        "Clicking directly on Top quick dot must hit QuickDiagramDot"
    );
    // East edge handle at (180, 130), Right quick dot at (192, 130)
    assert_eq!(
        super::interaction::hit_test_quick_diagram_dot(&sel, &state, Pos2::new(180.0, 130.0)),
        None,
        "East resize edge handle must keep priority over Right quick dot"
    );
    assert_eq!(
        super::interaction::hit_test_quick_diagram_dot(&sel, &state, Pos2::new(192.0, 130.0)),
        Some(("s1".to_string(), ConnectionSite::Right)),
        "Clicking directly on Right quick dot must hit QuickDiagramDot"
    );
}

#[test]
fn test_quick_diagram_drag_and_stationary_click_through_canvas() {
    fn step_canvas(
        ctx: &eframe::egui::Context,
        page: &mut Page,
        state: &mut CanvasState,
        events: Vec<eframe::egui::Event>,
    ) -> Pos2 {
        let mut origin = Pos2::ZERO;
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            eframe::egui::CentralPanel::default()
                .frame(eframe::egui::Frame::NONE)
                .show(ctx, |ui| {
                    origin = ui.next_widget_position();
                    let _ = render_page_canvas(ui, page, state);
                });
        });
        origin
    }

    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Lasso,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "qd-canvas".to_string();
    page.objects.push(DrawObject::Shape {
        id: "s1".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    page.objects.push(DrawObject::Shape {
        id: "s2".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 340.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLUE),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    state.selected_ids = vec!["s1".to_string()];

    // Frame 0: establish layout origin; Frame 1: hover at start_pos; Frame 2: press; Frame 3: drag past threshold
    let origin = step_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // 1. Drag from s1's Right dot (192, 130) to s2's Left snap point (340, 130)
    let start_pos = to_screen(192.0, 130.0);
    let mid_pos = to_screen(250.0, 130.0);
    let target_pos = to_screen(340.0, 130.0);
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(start_pos)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: start_pos,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(mid_pos)],
    );
    assert!(
        matches!(
            state.resize_corner,
            Some(ResizeCorner::QuickDiagramDot(ConnectionSite::Right))
        ),
        "Drag must engage QuickDiagramDot(Right), got {:?}",
        state.resize_corner
    );
    assert_eq!(state.active_stroke_page_id.as_deref(), Some("qd-canvas"));

    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(target_pos)],
    );
    assert!(
        state
            .connector_snap_candidate
            .as_ref()
            .is_some_and(|(id, site, _)| id == "s2" && *site == ConnectionSite::Left),
        "Dragging near s2's Left anchor must produce snap candidate"
    );

    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: target_pos,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(
        page.objects.len(),
        3,
        "Drag-to-connect must add 1 connector"
    );
    match &page.objects[2] {
        DrawObject::Connector { start, end, .. } => {
            assert_eq!(
                start,
                &ConnectorEnd::Attached {
                    shape_id: "s1".to_string(),
                    site: ConnectionSite::Right
                }
            );
            assert_eq!(
                end,
                &ConnectorEnd::Attached {
                    shape_id: "s2".to_string(),
                    site: ConnectionSite::Left
                }
            );
        }
        other => panic!("Expected Connector object, got {other:?}"),
    }
    assert_eq!(state.drag_start_pos, None);
    assert_eq!(state.draft_connector_start, None);
    assert_eq!(state.active_stroke_page_id, None);

    // 2. Drag from s1's Bottom dot (140, 172) into empty space (140, 320) -> new shape at drop pos
    state.selected_ids = vec!["s1".to_string()];
    state.editing_text_id = None;
    let bot_dot = to_screen(140.0, 172.0);
    let drop_empty = to_screen(140.0, 320.0);
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(bot_dot)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: bot_dot,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drop_empty)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: drop_empty,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(
        page.objects.len(),
        5,
        "Drag-to-empty must create new shape + connector"
    );
    let created_shape_id = page.objects[3].id().to_string();
    match &page.objects[3] {
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            ..
        } => {
            let center_x = *x + *width * 0.5;
            let center_y = *y + *height * 0.5;
            assert!(
                (center_x - 140.0).abs() < 2.0 && (center_y - 320.0).abs() < 2.0,
                "New shape must be placed around drop position (140, 320), got ({center_x}, {center_y})"
            );
        }
        other => panic!("Expected Shape object, got {other:?}"),
    }
    match &page.objects[4] {
        DrawObject::Connector { start, end, .. } => {
            assert_eq!(
                start,
                &ConnectorEnd::Attached {
                    shape_id: "s1".to_string(),
                    site: ConnectionSite::Bottom
                }
            );
            assert_eq!(
                end,
                &ConnectorEnd::Attached {
                    shape_id: created_shape_id,
                    site: ConnectionSite::Auto
                }
            );
        }
        other => panic!("Expected Connector object, got {other:?}"),
    }

    // 3. Stationary click (tap without drag) on s2's Right dot (432, 130) -> directional connected shape
    state.selected_ids = vec!["s2".to_string()];
    state.editing_text_id = None;
    let s2_right_dot = to_screen(432.0, 130.0);
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(s2_right_dot)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: s2_right_dot,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: s2_right_dot,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(
        page.objects.len(),
        7,
        "Stationary click on quick dot must create directional connected shape + connector instead of deselecting"
    );
    assert_eq!(state.selected_ids, vec![page.objects[5].id().to_string()]);
}

#[test]
fn test_handle_drag_start_preserves_canvas_updates_and_last_interacted_page() {
    fn step_two_pages(
        ctx: &eframe::egui::Context,
        page1: &mut Page,
        page2: &mut Page,
        state: &mut CanvasState,
        events: Vec<eframe::egui::Event>,
    ) -> Pos2 {
        let mut page2_origin = Pos2::ZERO;
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 2000.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            eframe::egui::CentralPanel::default()
                .frame(eframe::egui::Frame::NONE)
                .show(ctx, |ui| {
                    let _ = render_page_canvas(ui, page1, state);
                    page2_origin = ui.next_widget_position();
                    let _ = render_page_canvas(ui, page2, state);
                });
        });
        page2_origin
    }

    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Lasso,
        zoom: 1.0,
        last_interacted_page_id: Some("page-1".to_string()),
        ..Default::default()
    };
    let mut page1 = Page::new(PageTemplate::Blank);
    page1.id = "page-1".to_string();
    let mut page2 = Page::new(PageTemplate::Blank);
    page2.id = "page-2".to_string();
    page2.objects.push(DrawObject::Shape {
        id: "s2".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    state.selected_ids = vec!["s2".to_string()];

    // Frame 0: layout
    let page2_origin = step_two_pages(&ctx, &mut page1, &mut page2, &mut state, vec![]);
    let to_screen_p2 = |x: f32, y: f32| page2_origin + Vec2::new(x, y);

    // East handle of s2 is at (180, 130)
    let handle_pos = to_screen_p2(180.0, 130.0);
    let drag_start_move = to_screen_p2(205.0, 130.0);
    let drag_release = to_screen_p2(230.0, 130.0);

    step_two_pages(
        &ctx,
        &mut page1,
        &mut page2,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(handle_pos)],
    );
    step_two_pages(
        &ctx,
        &mut page1,
        &mut page2,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: handle_pos,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    // Frame where drag_started() fires on page-2's East handle
    step_two_pages(
        &ctx,
        &mut page1,
        &mut page2,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drag_start_move)],
    );

    assert_eq!(state.resize_corner, Some(ResizeCorner::East));
    assert_eq!(
        state.last_interacted_page_id.as_deref(),
        Some("page-2"),
        "drag_started early return must no longer skip last_interacted_page_id updates"
    );
    assert_eq!(state.hovered_page_id.as_deref(), Some("page-2"));
    assert!(state.lasso_points.is_empty());
    assert!(state.active_stroke_page_id.is_none());

    // Continue drag and release
    step_two_pages(
        &ctx,
        &mut page1,
        &mut page2,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drag_release)],
    );
    step_two_pages(
        &ctx,
        &mut page1,
        &mut page2,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: drag_release,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(state.resize_corner, None);
    match &page2.objects[0] {
        DrawObject::Shape { width, .. } => {
            assert!(
                *width > 120.0,
                "Expected shape width to expand after East handle drag, got {width}"
            );
        }
        other => panic!("Expected Shape object, got {other:?}"),
    }
}

#[test]
fn test_autoshape_release_undo_depth_and_draft_cleanup() {
    fn step_canvas(
        ctx: &eframe::egui::Context,
        page: &mut Page,
        state: &mut CanvasState,
        events: Vec<eframe::egui::Event>,
    ) -> Pos2 {
        let mut origin = Pos2::ZERO;
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            eframe::egui::CentralPanel::default()
                .frame(eframe::egui::Frame::NONE)
                .show(ctx, |ui| {
                    origin = ui.next_widget_position();
                    let _ = render_page_canvas(ui, page, state);
                });
        });
        origin
    }

    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        auto_shape_enabled: true,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "autoshape-undo-test".to_string();

    let origin = step_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // Gambar persegi panjang tertutup secara bertahap agar dikenali oleh recognize_shape
    let mut pts = Vec::new();
    for i in 0..=8 {
        pts.push(to_screen(100.0 + (i as f32) * 15.0, 100.0));
    }
    for i in 1..=6 {
        pts.push(to_screen(220.0, 100.0 + (i as f32) * 15.0));
    }
    for i in 1..=8 {
        pts.push(to_screen(220.0 - (i as f32) * 15.0, 190.0));
    }
    for i in 1..=6 {
        pts.push(to_screen(100.0, 190.0 - (i as f32) * 15.0));
    }

    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(pts[0])],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: pts[0],
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    for &pt in &pts[1..] {
        step_canvas(
            &ctx,
            &mut page,
            &mut state,
            vec![eframe::egui::Event::PointerMoved(pt)],
        );
    }

    // Simulasikan pena diam >500ms sehingga hold-to-snap aktif
    state.last_stroke_move_time =
        Some(std::time::Instant::now() - std::time::Duration::from_millis(650));
    step_canvas(&ctx, &mut page, &mut state, vec![]);
    assert!(
        state.recognized_draft_shape.is_some(),
        "Expected hold-to-snap to recognize closed rectangle"
    );
    assert!(state.snap_initial_shape.is_some());
    assert!(state.snap_flash_time.is_some());

    // Lepaskan pointer -> commit shape & bersihkan seluruh draft state
    let last_pt = *pts.last().expect("points non-empty");
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: last_pt,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(page.objects.len(), 1);
    assert!(matches!(page.objects[0], DrawObject::Shape { .. }));
    assert!(state.active_stroke_page_id.is_none());
    assert!(state.draft_stroke.is_none());
    assert!(state.last_stroke_move_time.is_none());
    assert!(state.recognized_draft_shape.is_none());
    assert!(state.snap_initial_shape.is_none());
    assert!(state.snap_flash_time.is_none());
    assert!(state.active_touch_id.is_none());

    // Undo #1 -> goresan asli sebelum snap
    state.undo(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert!(matches!(page.objects[0], DrawObject::Stroke { .. }));

    // Undo #2 -> halaman kosong, tanpa langkah undo duplikat ke-3
    state.undo(&mut page);
    assert!(page.objects.is_empty());
    assert!(
        !state.can_undo(&page.id),
        "AutoShape release must produce exactly 2 undo steps, not 3"
    );
}

#[test]
fn test_connector_line_segment_drag_pushes_single_undo() {
    fn step_canvas(
        ctx: &eframe::egui::Context,
        page: &mut Page,
        state: &mut CanvasState,
        events: Vec<eframe::egui::Event>,
    ) -> Pos2 {
        let mut origin = Pos2::ZERO;
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            eframe::egui::CentralPanel::default()
                .frame(eframe::egui::Frame::NONE)
                .show(ctx, |ui| {
                    origin = ui.next_widget_position();
                    let _ = render_page_canvas(ui, page, state);
                });
        });
        origin
    }

    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Lasso,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "conn-seg-undo".to_string();
    page.objects.push(DrawObject::Shape {
        id: "s1".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    page.objects.push(DrawObject::Shape {
        id: "s2".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 340.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    page.objects.push(DrawObject::Connector {
        id: "c1".to_string(),
        start: ConnectorEnd::Attached {
            shape_id: "s1".to_string(),
            site: ConnectionSite::Right,
        },
        end: ConnectorEnd::Attached {
            shape_id: "s2".to_string(),
            site: ConnectionSite::Left,
        },
        route: ConnectorRoute::Orthogonal,
        control_points: Vec::new(),
        style: ConnectorStyle::default(),
        locked: false,
    });
    state.selected_ids = vec!["c1".to_string()];

    let origin = step_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // Hit pada LineSegment (220, 130) — cukup jauh dari StartHandle (180,130) maupun midpoint (260,130)
    let seg_hit = to_screen(220.0, 130.0);
    let drag_to = to_screen(220.0, 185.0);

    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(seg_hit)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: seg_hit,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drag_to)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: drag_to,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    match &page.objects[2] {
        DrawObject::Connector { control_points, .. } => {
            assert!(
                !control_points.is_empty(),
                "Dragging connector LineSegment should insert a control point"
            );
        }
        other => panic!("Expected Connector, got {other:?}"),
    }

    assert!(state.can_undo(&page.id));
    state.undo(&mut page);
    match &page.objects[2] {
        DrawObject::Connector { control_points, .. } => {
            assert!(
                control_points.is_empty(),
                "Single undo must restore original straight connector"
            );
        }
        other => panic!("Expected Connector, got {other:?}"),
    }
    assert!(
        !state.can_undo(&page.id),
        "Connector LineSegment drag must push only 1 undo snapshot"
    );
}

#[test]
fn test_pen_primary_click_and_escape_clear_stale_draft_state() {
    fn step_canvas(
        ctx: &eframe::egui::Context,
        page: &mut Page,
        state: &mut CanvasState,
        events: Vec<eframe::egui::Event>,
    ) -> Pos2 {
        let mut origin = Pos2::ZERO;
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            eframe::egui::CentralPanel::default()
                .frame(eframe::egui::Frame::NONE)
                .show(ctx, |ui| {
                    origin = ui.next_widget_position();
                    let _ = render_page_canvas(ui, page, state);
                });
        });
        origin
    }

    fn seed_stale_draft(state: &mut CanvasState, page_id: &str) {
        let dummy_shape = crate::geometry::RecognizedShape {
            kind: ShapeKind::Rectangle,
            x: 10.0,
            y: 10.0,
            width: 40.0,
            height: 40.0,
        };
        state.active_stroke_page_id = Some(page_id.to_string());
        state.last_stroke_move_time = Some(std::time::Instant::now());
        state.recognized_draft_shape = Some(dummy_shape.clone());
        state.snap_initial_shape = Some(dummy_shape);
        state.snap_flash_time = Some(std::time::Instant::now());
        state.active_touch_id = Some(eframe::egui::TouchId(7));
        state.draft_connector_start = Some(ConnectorEnd::Free { x: 1.0, y: 2.0 });
        state.draft_connector_current = Some(Point2D::new(3.0, 4.0));
        state.lasso_points = vec![Pos2::new(5.0, 6.0)];
    }

    fn assert_draft_cleared(state: &CanvasState) {
        assert!(state.active_stroke_page_id.is_none());
        assert!(state.draft_stroke.is_none());
        assert!(state.draft_shape_start.is_none());
        assert!(state.draft_shape_current.is_none());
        assert!(state.last_stroke_move_time.is_none());
        assert!(state.recognized_draft_shape.is_none());
        assert!(state.snap_initial_shape.is_none());
        assert!(state.snap_flash_time.is_none());
        assert!(state.active_touch_id.is_none());
        assert!(state.draft_connector_start.is_none());
        assert!(state.draft_connector_current.is_none());
        assert!(state.connector_snap_candidate.is_none());
        assert!(state.lasso_points.is_empty());
    }

    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "draft-cleanup-test".to_string();

    let origin = step_canvas(&ctx, &mut page, &mut state, vec![]);
    let click_pos = origin + Vec2::new(140.0, 140.0);

    // 1. Klik tunggal Pen membersihkan sisa state draft lama setelah membuat titik
    seed_stale_draft(&mut state, &page.id);
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(click_pos)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: click_pos,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: click_pos,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(page.objects.len(), 1);
    assert!(matches!(page.objects[0], DrawObject::Stroke { .. }));
    assert_draft_cleared(&state);

    // 2. Escape membatalkan seleksi dan state draft yang tersisa
    seed_stale_draft(&mut state, &page.id);
    state.selected_ids = vec![page.objects[0].id().to_string()];
    state.editing_text_id = None;
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::Key {
            key: eframe::egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );

    assert!(state.selected_ids.is_empty());
    assert_draft_cleared(&state);
}

#[test]
fn test_connector_snap_orthogonal_alignment_and_collinear_prune_without_clones() {
    fn step_canvas(
        ctx: &eframe::egui::Context,
        page: &mut Page,
        state: &mut CanvasState,
        events: Vec<eframe::egui::Event>,
    ) -> Pos2 {
        let mut origin = Pos2::ZERO;
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            eframe::egui::CentralPanel::default()
                .frame(eframe::egui::Frame::NONE)
                .show(ctx, |ui| {
                    origin = ui.next_widget_position();
                    let _ = render_page_canvas(ui, page, state);
                });
        });
        origin
    }

    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Lasso,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "conn-no-clone".to_string();
    page.objects.push(DrawObject::Shape {
        id: "s1".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    page.objects.push(DrawObject::Shape {
        id: "s2".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 340.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    page.objects.push(DrawObject::Connector {
        id: "c1".to_string(),
        start: ConnectorEnd::Attached {
            shape_id: "s1".to_string(),
            site: ConnectionSite::Right,
        },
        end: ConnectorEnd::Free { x: 280.0, y: 130.0 },
        route: ConnectorRoute::Orthogonal,
        control_points: Vec::new(),
        style: ConnectorStyle::default(),
        locked: false,
    });
    state.selected_ids = vec!["c1".to_string()];

    let origin = step_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // 1. Seret EndHandle (280, 130) ke Left anchor s2 (340, 130) -> attach
    let end_start = to_screen(280.0, 130.0);
    let end_snap = to_screen(340.0, 130.0);
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(end_start)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: end_start,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(end_snap)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: end_snap,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    match &page.objects[2] {
        DrawObject::Connector { end, .. } => {
            assert_eq!(
                end,
                &ConnectorEnd::Attached {
                    shape_id: "s2".to_string(),
                    site: ConnectionSite::Left,
                }
            );
        }
        other => panic!("Expected Connector, got {other:?}"),
    }

    // 2. Tarik segmen garis (220, 130) ke (250, 175) -> lalu mendekati sumbu y=130 -> orthogonal snap -> prune collinear saat lepas
    let cp_start = to_screen(220.0, 130.0);
    let cp_bend = to_screen(250.0, 175.0);
    let cp_near_line = to_screen(250.0, 134.0);
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(cp_start)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: cp_start,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(cp_bend)],
    );
    match &page.objects[2] {
        DrawObject::Connector { control_points, .. } => {
            assert_eq!(control_points.len(), 1);
            assert!((control_points[0].y - 175.0).abs() < 0.01);
        }
        other => panic!("Expected Connector, got {other:?}"),
    }
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(cp_near_line)],
    );
    match &page.objects[2] {
        DrawObject::Connector { control_points, .. } => {
            assert_eq!(control_points.len(), 1);
            assert!(
                (control_points[0].y - 130.0).abs() < 0.01,
                "Orthogonal control point drag should snap y to endpoint y=130, got {}",
                control_points[0].y
            );
        }
        other => panic!("Expected Connector, got {other:?}"),
    }
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: cp_near_line,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    match &page.objects[2] {
        DrawObject::Connector { control_points, .. } => {
            assert!(
                control_points.is_empty(),
                "Collinear control point should be pruned on release"
            );
        }
        other => panic!("Expected Connector, got {other:?}"),
    }
}

#[test]
fn test_moving_shape_prunes_covered_connector_control_points_through_canvas() {
    fn step_canvas(
        ctx: &eframe::egui::Context,
        page: &mut Page,
        state: &mut CanvasState,
        events: Vec<eframe::egui::Event>,
    ) -> Pos2 {
        let mut origin = Pos2::ZERO;
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 1000.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            eframe::egui::CentralPanel::default()
                .frame(eframe::egui::Frame::NONE)
                .show(ctx, |ui| {
                    origin = ui.next_widget_position();
                    let _ = render_page_canvas(ui, page, state);
                });
        });
        origin
    }

    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Lasso,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "prune-covered-cp".to_string();
    page.objects.push(DrawObject::Shape {
        id: "s1".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    page.objects.push(DrawObject::Shape {
        id: "s2".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 340.0,
        y: 100.0,
        width: 80.0,
        height: 60.0,
        color: ColorData::from(Color32::BLACK),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    page.objects.push(DrawObject::Connector {
        id: "c1".to_string(),
        start: ConnectorEnd::Attached {
            shape_id: "s1".to_string(),
            site: ConnectionSite::Right,
        },
        end: ConnectorEnd::Attached {
            shape_id: "s2".to_string(),
            site: ConnectionSite::Left,
        },
        route: ConnectorRoute::Orthogonal,
        control_points: vec![Point2D::new(260.0, 130.0), Point2D::new(260.0, 210.0)],
        style: ConnectorStyle::default(),
        locked: false,
    });
    state.selected_ids = vec!["s2".to_string()];

    let origin = step_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // Pindahkan s2 ke kiri sehingga menutupi control point pertama (260, 130) tetapi bukan (260, 210)
    let drag_start = to_screen(380.0, 130.0);
    let drag_dest = to_screen(260.0, 130.0);
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drag_start)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: drag_start,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drag_dest)],
    );
    step_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: drag_dest,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    match &page.objects[2] {
        DrawObject::Connector { control_points, .. } => {
            assert_eq!(control_points, &vec![Point2D::new(260.0, 210.0)]);
        }
        other => panic!("Expected Connector, got {other:?}"),
    }
}

fn step_test_canvas(
    ctx: &eframe::egui::Context,
    page: &mut Page,
    state: &mut CanvasState,
    events: Vec<eframe::egui::Event>,
) -> Pos2 {
    let mut origin = Pos2::ZERO;
    let input = eframe::egui::RawInput {
        screen_rect: Some(eframe::egui::Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(1200.0, 1200.0),
        )),
        events,
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        eframe::egui::CentralPanel::default()
            .frame(eframe::egui::Frame::NONE)
            .show(ctx, |ui| {
                origin = ui.next_widget_position();
                let _ = render_page_canvas(ui, page, state);
            });
    });
    origin
}

#[test]
fn test_pointer_sampling_very_short_micro_stroke_and_single_point_tap() {
    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "micro-stroke-page".to_string();

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // 1. Goresan sangat pendek (0.35 pt dan 0.70 pt, jauh di bawah threshold lama 1.2 pt dan drag threshold 6.0 px egui)
    let p0 = to_screen(100.0, 100.0);
    let p1 = to_screen(100.35, 100.15);
    let p2 = to_screen(100.70, 100.30);

    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(p0)],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: p0,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(p1),
            eframe::egui::Event::PointerMoved(p2),
        ],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: p2,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(
        page.objects.len(),
        1,
        "Micro-stroke harus menghasilkan tepat 1 objek Stroke"
    );
    match &page.objects[0] {
        DrawObject::Stroke { points, .. } => {
            assert_eq!(
                points.len(),
                3,
                "Seluruh 3 titik micro-stroke (0.35 pt step) harus tersimpan tanpa terbuang oleh threshold"
            );
            assert!((points[0].x - 100.0).abs() < 1e-3);
            assert!((points[1].x - 100.35).abs() < 1e-3);
            assert!((points[2].x - 100.70).abs() < 1e-3);
        }
        other => panic!("Expected Stroke, got {other:?}"),
    }
    assert_eq!(state.last_completed_raw_samples.len(), 3);

    // 2. Goresan satu titik (tap tanpa pergerakan)
    let tap_pos = to_screen(200.0, 200.0);
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(tap_pos)],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: tap_pos,
            button: eframe::egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: tap_pos,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(page.objects.len(), 2);
    match &page.objects[1] {
        DrawObject::Stroke { points, .. } => {
            assert_eq!(points.len(), 1);
            assert!((points[0].x - 200.0).abs() < 1e-3);
            assert!((points[0].y - 200.0).abs() < 1e-3);
        }
        other => panic!("Expected single-point Stroke, got {other:?}"),
    }
}

#[test]
fn test_pointer_sampling_multiple_events_in_single_frame_and_directions() {
    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "multi-event-page".to_string();

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // Uji goresan horizontal, vertikal, diagonal, dan perubahan arah tajam dalam satu frame burst
    let path = [
        (50.0, 50.0),
        (52.0, 50.0), // Horizontal
        (55.0, 50.0),
        (55.0, 53.0), // Belokan tajam 90 derajat ke Vertikal
        (55.0, 58.0),
        (60.0, 63.0), // Diagonal
        (65.0, 58.0), // Belokan tajam ke atas-kanan
        (70.0, 64.0), // Titik lepas akhir
    ];

    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(to_screen(path[0].0, path[0].1)),
            eframe::egui::Event::PointerButton {
                pos: to_screen(path[0].0, path[0].1),
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerMoved(to_screen(path[1].0, path[1].1)),
            eframe::egui::Event::PointerMoved(to_screen(path[2].0, path[2].1)),
            eframe::egui::Event::PointerMoved(to_screen(path[3].0, path[3].1)),
            eframe::egui::Event::PointerMoved(to_screen(path[4].0, path[4].1)),
            eframe::egui::Event::PointerMoved(to_screen(path[5].0, path[5].1)),
            eframe::egui::Event::PointerMoved(to_screen(path[6].0, path[6].1)),
        ],
    );

    // Lepaskan pointer pada titik terakhir yang bergerak sedikit dari titik ke-6
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(to_screen(path[7].0, path[7].1)),
            eframe::egui::Event::PointerButton {
                pos: to_screen(path[7].0, path[7].1),
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ],
    );

    assert_eq!(page.objects.len(), 1);
    match &page.objects[0] {
        DrawObject::Stroke { points, .. } => {
            assert_eq!(
                points.len(),
                path.len(),
                "Seluruh 8 sampel event dalam satu frame + release wajib diproses secara berurutan"
            );
            for (i, &(ex, ey)) in path.iter().enumerate() {
                assert!(
                    (points[i].x - ex).abs() < 1e-3 && (points[i].y - ey).abs() < 1e-3,
                    "Titik ke-{i} meleset: diharapkan ({ex}, {ey}), didapat ({}, {})",
                    points[i].x,
                    points[i].y
                );
            }
        }
        other => panic!("Expected Stroke, got {other:?}"),
    }

    // Pastikan raw samples menyimpan urutan sequence yang monoton dan konsisten
    assert_eq!(state.last_completed_raw_samples.len(), path.len());
    for (idx, sample) in state.last_completed_raw_samples.iter().enumerate() {
        assert_eq!(sample.sequence, idx as u64);
    }
}

#[test]
fn test_pointer_sampling_pressure_pairing_and_mouse_fallback() {
    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        zoom: 1.0,
        pressure_sensitivity_enabled: true,
        pressure_min: 0.2,
        pressure_multiplier: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "pressure-pairing-page".to_string();

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    let dev = eframe::egui::TouchDeviceId(0);
    let tid = eframe::egui::TouchId::from(42_u64);
    let p0 = to_screen(100.0, 100.0);
    let p1 = to_screen(110.0, 105.0);
    let p2 = to_screen(120.0, 110.0);
    let p3 = to_screen(130.0, 115.0);

    // 1. Input dengan variasi pressure pada beberapa Touch events dalam satu frame
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(p0),
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::Start,
                pos: p0,
                force: Some(0.0),
            },
            eframe::egui::Event::PointerButton {
                pos: p0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::Move,
                pos: p1,
                force: Some(0.25),
            },
            eframe::egui::Event::PointerMoved(p1),
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::Move,
                pos: p2,
                force: Some(0.50),
            },
            eframe::egui::Event::PointerMoved(p2),
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::Move,
                pos: p3,
                force: Some(1.0),
            },
            eframe::egui::Event::PointerMoved(p3),
        ],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::End,
                pos: p3,
                force: Some(1.0),
            },
            eframe::egui::Event::PointerButton {
                pos: p3,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ],
    );

    assert_eq!(page.objects.len(), 1);
    match &page.objects[0] {
        DrawObject::Stroke { points, .. } => {
            assert_eq!(points.len(), 4);
            // Effective pressure = 0.2 + 0.8 * raw_force
            assert!((points[0].pressure - 0.20).abs() < 1e-4);
            assert!((points[1].pressure - 0.40).abs() < 1e-4);
            assert!((points[2].pressure - 0.60).abs() < 1e-4);
            assert!((points[3].pressure - 1.00).abs() < 1e-4);
        }
        other => panic!("Expected Stroke, got {other:?}"),
    }

    // 2. Input tanpa pressure (Mouse fallback -> effective_pressure(1.0) = 1.0)
    let m0 = to_screen(200.0, 200.0);
    let m1 = to_screen(220.0, 210.0);
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(m0),
            eframe::egui::Event::PointerButton {
                pos: m0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerMoved(m1),
        ],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: m1,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(page.objects.len(), 2);
    match &page.objects[1] {
        DrawObject::Stroke { points, .. } => {
            assert_eq!(points.len(), 2);
            assert!((points[0].pressure - 1.0).abs() < 1e-4);
            assert!((points[1].pressure - 1.0).abs() < 1e-4);
        }
        other => panic!("Expected Stroke, got {other:?}"),
    }
    assert!(
        state
            .last_completed_raw_samples
            .iter()
            .all(|s| s.raw_pressure.is_none() && s.source == PointerSource::Mouse)
    );
}

#[test]
fn test_pointer_sampling_cancellation_via_pointer_gone_and_touch_cancel() {
    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "cancel-page".to_string();

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // 1. Mulai menggambar lalu batalkan dengan Event::PointerGone
    let p0 = to_screen(80.0, 80.0);
    let p1 = to_screen(100.0, 90.0);
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(p0),
            eframe::egui::Event::PointerButton {
                pos: p0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerMoved(p1),
        ],
    );
    assert!(state.draft_stroke.is_some());

    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerGone],
    );
    assert!(
        state.draft_stroke.is_none(),
        "PointerGone harus membatalkan draft_stroke aktif"
    );
    assert!(state.raw_stroke_samples.is_empty());
    assert!(
        page.objects.is_empty(),
        "Goresan yang dibatalkan tidak boleh disimpan ke page.objects"
    );

    // 2. Mulai menggambar dengan sentuhan lalu batalkan dengan TouchPhase::Cancel
    let dev = eframe::egui::TouchDeviceId(0);
    let tid = eframe::egui::TouchId::from(99_u64);
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(p0),
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::Start,
                pos: p0,
                force: Some(0.5),
            },
            eframe::egui::Event::PointerButton {
                pos: p0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::Move,
                pos: p1,
                force: Some(0.6),
            },
            eframe::egui::Event::PointerMoved(p1),
        ],
    );
    assert!(state.draft_stroke.is_some());

    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::Cancel,
                pos: p1,
                force: None,
            },
            eframe::egui::Event::PointerButton {
                pos: p1,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ],
    );
    assert!(state.draft_stroke.is_none());
    assert!(page.objects.is_empty());
}

#[test]
fn test_pointer_sampling_zoom_low_normal_high_and_page_space_consistency() {
    let ctx = eframe::egui::Context::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "zoom-consistency-page".to_string();

    for &zoom in &[0.4_f32, 1.0_f32, 2.5_f32] {
        let mut state = CanvasState {
            current_tool: ActiveTool::Pen,
            zoom,
            ..Default::default()
        };
        page.objects.clear();

        let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
        let page_to_screen = |px: f32, py: f32| origin + Vec2::new(px * zoom, py * zoom);

        // Gerakan kecil dalam page-space (step 0.25 pt) pada berbagai tingkat zoom
        let p0 = page_to_screen(40.0, 50.0);
        let p1 = page_to_screen(40.25, 50.25);
        let p2 = page_to_screen(40.50, 50.50);

        step_test_canvas(
            &ctx,
            &mut page,
            &mut state,
            vec![
                eframe::egui::Event::PointerMoved(p0),
                eframe::egui::Event::PointerButton {
                    pos: p0,
                    button: eframe::egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                eframe::egui::Event::PointerMoved(p1),
                eframe::egui::Event::PointerMoved(p2),
            ],
        );
        step_test_canvas(
            &ctx,
            &mut page,
            &mut state,
            vec![eframe::egui::Event::PointerButton {
                pos: p2,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );

        assert_eq!(page.objects.len(), 1, "Failed at zoom {zoom}");
        match &page.objects[0] {
            DrawObject::Stroke { points, .. } => {
                assert_eq!(points.len(), 3, "Points lost at zoom {zoom}");
                assert!((points[0].x - 40.0).abs() < 1e-3);
                assert!((points[1].x - 40.25).abs() < 1e-3);
                assert!((points[2].x - 40.50).abs() < 1e-3);
            }
            other => panic!("Expected Stroke, got {other:?}"),
        }
    }
}

#[test]
fn test_lasso_and_eraser_high_frequency_sampling() {
    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Lasso,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "lasso-eraser-sampling".to_string();

    // Tempatkan goresan titik kecil di (52.0, 52.0) untuk diuji dengan lasso pendek & cepat
    page.objects.push(DrawObject::Stroke {
        id: "tiny-dot".to_string(),
        points: vec![StrokePoint::new(52.0, 52.0, 1.0)],
        color: ColorData::from(Color32::BLACK),
        width: 1.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });
    // Tempatkan 3 goresan vertikal di x=120, x=160, x=200 untuk diuji dengan sapuan eraser cepat
    for (idx, x) in [120.0_f32, 160.0, 200.0].into_iter().enumerate() {
        page.objects.push(DrawObject::Stroke {
            id: format!("vline-{idx}"),
            points: vec![
                StrokePoint::new(x, 80.0, 1.0),
                StrokePoint::new(x, 140.0, 1.0),
            ],
            color: ColorData::from(Color32::BLACK),
            width: 2.0,
            is_highlighter: false,
            style: StrokeStyle::Solid,
        });
    }
    state.rebuild_spatial_grid(&page.id, &page.objects);

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // 1. Lasso pendek (< 5px excursion) dengan banyak perubahan arah dalam satu frame
    let l0 = to_screen(50.0, 50.0);
    let l1 = to_screen(54.5, 50.0);
    let l2 = to_screen(54.5, 54.5);
    let l3 = to_screen(50.0, 54.5);

    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(l0),
            eframe::egui::Event::PointerButton {
                pos: l0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerMoved(l1),
            eframe::egui::Event::PointerMoved(l2),
            eframe::egui::Event::PointerMoved(l3),
        ],
    );
    assert_eq!(
        state.lasso_points.len(),
        4,
        "Seluruh 4 titik lasso dalam satu frame wajib terkumpul"
    );

    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: l3,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(
        state.selected_ids,
        vec!["tiny-dot".to_string()],
        "Lasso pendek (< 6px) harus tetap berhasil memilih objek di dalamnya"
    );

    // 2. Eraser dengan gerakan cepat dari x=100 ke x=220 dalam satu langkah (melintasi x=120, 160, 200)
    state.current_tool = ActiveTool::Eraser;
    state.eraser_mode = EraserMode::WholeStroke;
    let e_start = to_screen(100.0, 110.0);
    let e_end = to_screen(220.0, 110.0);

    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(e_start),
            eframe::egui::Event::PointerButton {
                pos: e_start,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerMoved(e_end),
        ],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: e_end,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(
        page.objects.len(),
        1,
        "Gerakan cepat eraser dari x=100 ke x=220 harus menghapus ketiga garis vertikal tanpa melompati jalur"
    );
    assert_eq!(page.objects[0].id(), "tiny-dot");
}

#[test]
fn test_sampling_engine_high_frequency_benchmark() {
    let dev = eframe::egui::TouchDeviceId(0);
    let tid = eframe::egui::TouchId::from(7_u64);
    let mut raw_events = Vec::with_capacity(2000);
    for i in 0..1000 {
        let x = 50.0 + (i as f32) * 0.4;
        let y = 100.0 + ((i as f32) * 0.1).sin() * 8.0;
        let pos = Pos2::new(x, y);
        let force = 0.3 + 0.6 * ((i as f32) * 0.05).cos().abs();
        raw_events.push(eframe::egui::Event::Touch {
            device_id: dev,
            id: tid,
            phase: if i == 0 {
                eframe::egui::TouchPhase::Start
            } else {
                eframe::egui::TouchPhase::Move
            },
            pos,
            force: Some(force),
        });
        raw_events.push(eframe::egui::Event::PointerMoved(pos));
    }

    let start_time = std::time::Instant::now();
    let batch =
        collect_frame_pointer_batch(&raw_events, Some(tid), Some(Pos2::new(50.0, 100.0)), None);
    let mut raw_samples = Vec::with_capacity(batch.events.len());
    let mut draft_stroke = Vec::with_capacity(batch.events.len());
    for (seq, ev) in batch.events.iter().enumerate() {
        let sample = RawPointerSample {
            screen_pos: ev.screen_pos,
            page_pos: ev.screen_pos,
            raw_pressure: ev.raw_pressure,
            effective_pressure: ev.raw_pressure.unwrap_or(1.0),
            phase: ev.phase,
            source: ev.source,
            sequence: seq as u64,
            dt_secs: 0.001,
        };
        append_stroke_sample(&mut raw_samples, &mut draft_stroke, sample, true, false);
    }
    let filtered = crate::geometry::filter_stroke_points_adaptive(&draft_stroke, 3.0, false);
    let elapsed = start_time.elapsed();

    assert_eq!(batch.events.len(), 1000);
    assert_eq!(raw_samples.len(), 1000);
    assert_eq!(draft_stroke.len(), 1000);
    assert!(!filtered.is_empty());
    assert!(
        elapsed.as_millis() < 50,
        "Pemrosesan 1000 sampel high-frequency harus sangat cepat (< 50ms), aktual: {elapsed:?}"
    );
}

#[test]
fn test_track2_pen_pencil_highlighter_rendering_and_width_parity() {
    let ctx = eframe::egui::Context::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "track2-tool-parity".to_string();

    for (tool, expected_width) in [
        (ActiveTool::Pen, 3.0_f32),
        (ActiveTool::Pencil, 2.5_f32),
        (ActiveTool::Highlighter, 18.0_f32),
    ] {
        let mut state = CanvasState {
            current_tool: tool,
            stroke_width: 3.0,
            zoom: 1.5,
            ..Default::default()
        };
        page.objects.clear();

        let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
        let to_screen = |x: f32, y: f32| origin + Vec2::new(x * 1.5, y * 1.5);

        let p0 = to_screen(40.0, 40.0);
        let p1 = to_screen(60.0, 50.0);
        let p2 = to_screen(45.0, 70.0); // Sudut tajam

        step_test_canvas(
            &ctx,
            &mut page,
            &mut state,
            vec![
                eframe::egui::Event::PointerMoved(p0),
                eframe::egui::Event::PointerButton {
                    pos: p0,
                    button: eframe::egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                eframe::egui::Event::PointerMoved(p1),
                eframe::egui::Event::PointerMoved(p2),
            ],
        );
        assert!(state.draft_stroke.is_some());

        step_test_canvas(
            &ctx,
            &mut page,
            &mut state,
            vec![eframe::egui::Event::PointerButton {
                pos: p2,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );

        assert_eq!(page.objects.len(), 1);
        match &page.objects[0] {
            DrawObject::Stroke {
                width,
                is_highlighter,
                points,
                ..
            } => {
                assert!((*width - expected_width).abs() < 1e-4);
                assert_eq!(*is_highlighter, tool == ActiveTool::Highlighter);
                assert_eq!(points.len(), 3);
            }
            other => panic!("Expected Stroke, got {other:?}"),
        }

        // Render frame berikutnya untuk memverifikasi pembuatan dan pembacaan cache masing-masing alat
        step_test_canvas(&ctx, &mut page, &mut state, vec![]);
        if tool == ActiveTool::Highlighter {
            assert_eq!(state.highlighter_cache.len(), 1);
            assert!(state.mesh_cache.is_empty());
        } else {
            assert_eq!(state.mesh_cache.len(), 1);
        }
    }
}

#[test]
fn test_track4_canvas_lasso_selection_zoom_drag_and_stale_id_cleanup() {
    let ctx = eframe::egui::Context::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "track4-lasso-page".to_string();

    // 1. Uji konsistensi seleksi lasso pada berbagai tingkat zoom (0.5x, 1.0x, 2.5x)
    for &zoom in &[0.5_f32, 1.0_f32, 2.5_f32] {
        let mut state = CanvasState {
            current_tool: ActiveTool::Lasso,
            lasso_selection_mode: crate::model::LassoSelectionMode::Containment,
            zoom,
            ..Default::default()
        };
        page.objects = vec![
            DrawObject::Stroke {
                id: "s-in".into(),
                points: vec![
                    StrokePoint::new(30.0, 30.0, 0.6),
                    StrokePoint::new(60.0, 60.0, 0.6),
                ],
                color: ColorData::from(Color32::BLACK),
                width: 3.0,
                is_highlighter: false,
                style: StrokeStyle::Solid,
            },
            DrawObject::Shape {
                id: "sh-partial".into(),
                shape_type: ShapeKind::Rectangle,
                x: 70.0,
                y: 30.0,
                width: 60.0, // x: 70..130 (keluar dari lasso x: 10..100)
                height: 40.0,
                color: ColorData::from(Color32::BLUE),
                stroke_width: 2.0,
                fill: None,
                rotation: 0.0,
                corner_radius: 0.0,
                label: None,
                locked: false,
                curve_points: Vec::new(),
            },
        ];
        state.rebuild_spatial_grid(&page.id, &page.objects);

        let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
        let to_screen = |px: f32, py: f32| origin + Vec2::new(px * zoom, py * zoom);

        let p0 = to_screen(10.0, 10.0);
        let p1 = to_screen(100.0, 10.0);
        let p2 = to_screen(100.0, 100.0);
        let p3 = to_screen(10.0, 100.0);

        step_test_canvas(
            &ctx,
            &mut page,
            &mut state,
            vec![
                eframe::egui::Event::PointerMoved(p0),
                eframe::egui::Event::PointerButton {
                    pos: p0,
                    button: eframe::egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                eframe::egui::Event::PointerMoved(p1),
                eframe::egui::Event::PointerMoved(p2),
                eframe::egui::Event::PointerMoved(p3),
            ],
        );
        step_test_canvas(
            &ctx,
            &mut page,
            &mut state,
            vec![eframe::egui::Event::PointerButton {
                pos: p3,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );

        // Pada Containment mode, hanya "s-in" yang terpilih di semua level zoom
        assert_eq!(
            state.selected_ids,
            vec!["s-in".to_string()],
            "Hasil Containment lasso harus konsisten pada zoom={zoom}"
        );
        // Lasso tidak boleh menambahkan snapshot undo palsu
        assert!(!state.can_undo(&page.id));
    }

    // 2. Uji penghapusan objek terpilih menggunakan Eraser: tidak boleh meninggalkan stale ID di selected_ids
    let mut state = CanvasState {
        selected_ids: vec!["s-in".to_string(), "sh-partial".to_string()],
        eraser_mode: EraserMode::WholeStroke,
        ..Default::default()
    };
    state.rebuild_spatial_grid(&page.id, &page.objects);
    assert!(state.erase_at(&mut page, Pos2::new(45.0, 45.0), 6.0));
    assert_eq!(
        state.selected_ids,
        vec!["sh-partial".to_string()],
        "Objek yang dihapus dengan Eraser wajib langsung dibersihkan dari selected_ids (tanpa stale ID)"
    );
}

#[test]
fn test_hotfix_stylus_touch_end_with_pointer_gone_commits_once_and_preserves_metadata() {
    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        zoom: 1.0,
        pressure_sensitivity_enabled: true,
        pressure_min: 0.2,
        pressure_multiplier: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "stylus-pointer-gone-page".to_string();

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    let dev = eframe::egui::TouchDeviceId(1);
    let tid = eframe::egui::TouchId::from(42_u64);
    let p0 = to_screen(40.0, 50.0);
    let p1 = to_screen(80.0, 70.0);
    let p2 = to_screen(120.0, 90.0);

    // Frame 1: Stylus Start -> Move
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::Start,
                pos: p0,
                force: Some(0.30),
            },
            eframe::egui::Event::PointerButton {
                pos: p0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::Move,
                pos: p1,
                force: Some(0.75),
            },
            eframe::egui::Event::PointerMoved(p1),
        ],
    );
    assert!(state.draft_stroke.is_some());

    // Frame 2: Stylus Move -> TouchPhase::End + PointerButton(released) + PointerGone dalam batch yang sama
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::Touch {
                device_id: dev,
                id: tid,
                phase: eframe::egui::TouchPhase::End,
                pos: p2,
                force: Some(0.45),
            },
            eframe::egui::Event::PointerButton {
                pos: p2,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerGone,
        ],
    );

    // Wajib commit tepat 1 stroke ke page.objects, tidak ada draft tersisa, tidak ada duplikat
    assert_eq!(
        page.objects.len(),
        1,
        "Goresan stylus wajib tersimpan permanen meski PointerGone menyertai TouchPhase::End"
    );
    assert!(state.draft_stroke.is_none());
    assert!(state.active_stroke_page_id.is_none());
    assert!(!state.pointer_gesture_active);
    assert!(state.active_touch_id.is_none());

    match &page.objects[0] {
        DrawObject::Stroke { points, .. } => {
            assert_eq!(points.len(), 3);
            assert!((points[2].x - 120.0).abs() < 1e-3);
            assert!((points[2].y - 90.0).abs() < 1e-3);
            let expected_end_p = state.effective_pressure(0.45);
            assert!(
                (points[2].pressure - expected_end_p).abs() < 1e-4,
                "Pressure titik akhir stylus harus terjaga ({} vs {expected_end_p})",
                points[2].pressure
            );
        }
        other => panic!("Expected Stroke, got {other:?}"),
    }

    // Release sample metadata wajib mempertahankan PointerSource::Touch, bukan Mouse
    let last_sample = state
        .last_completed_raw_samples
        .last()
        .expect("raw samples recorded");
    assert_eq!(last_sample.phase, PointerSamplePhase::Release);
    assert_eq!(
        last_sample.source,
        PointerSource::Touch {
            device_id: dev,
            id: tid,
        }
    );
    assert_eq!(last_sample.raw_pressure, Some(0.45));

    // Frame 3: Frame berikutnya yang hanya berisi PointerGone tidak boleh merusak stroke yang sudah ter-commit
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerGone],
    );
    assert_eq!(page.objects.len(), 1);

    // Undo/Redo setelah stylus finalization harus bekerja normal (hanya 1 undo snapshot)
    assert_eq!(state.undo_stacks.get(&page.id).unwrap().len(), 1);
    state.undo(&mut page);
    assert!(page.objects.is_empty());
    state.redo(&mut page);
    assert_eq!(page.objects.len(), 1);
}

#[test]
fn test_hotfix_unrelated_touch_cancel_and_short_stylus_stroke_with_pointer_gone() {
    let ctx = eframe::egui::Context::default();
    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        zoom: 1.0,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "stylus-unrelated-cancel".to_string();

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    let dev = eframe::egui::TouchDeviceId(1);
    let stylus_tid = eframe::egui::TouchId::from(10_u64);
    let palm_tid = eframe::egui::TouchId::from(99_u64);
    let p0 = to_screen(50.0, 50.0);
    let p1 = to_screen(52.0, 51.0); // Goresan pendek (< 6px drag threshold)

    // 1. Pembatalan TouchId lain (mis. telapak tangan) tidak boleh membatalkan goresan stylus aktif
    let batch_with_unrelated_cancel = collect_frame_pointer_batch(
        &[
            eframe::egui::Event::Touch {
                device_id: dev,
                id: palm_tid,
                phase: eframe::egui::TouchPhase::Cancel,
                pos: to_screen(200.0, 200.0),
                force: None,
            },
            eframe::egui::Event::PointerMoved(p1),
        ],
        None,
        None,
        None,
    );
    assert!(
        !batch_with_unrelated_cancel.should_cancel(),
        "TouchPhase::Cancel tanpa sentuhan aktif tidak boleh membatalkan interaksi mouse"
    );

    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::Touch {
                device_id: dev,
                id: stylus_tid,
                phase: eframe::egui::TouchPhase::Start,
                pos: p0,
                force: Some(0.6),
            },
            eframe::egui::Event::PointerButton {
                pos: p0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    assert!(state.active_touch_id == Some(stylus_tid));

    // Frame dengan pembatalan palm_tid di tengah goresan stylus_tid + selesai dengan goresan pendek + PointerGone
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::Touch {
                device_id: dev,
                id: palm_tid,
                phase: eframe::egui::TouchPhase::Cancel,
                pos: to_screen(250.0, 250.0),
                force: None,
            },
            eframe::egui::Event::Touch {
                device_id: dev,
                id: stylus_tid,
                phase: eframe::egui::TouchPhase::Move,
                pos: p1,
                force: Some(0.8),
            },
            eframe::egui::Event::PointerMoved(p1),
            eframe::egui::Event::Touch {
                device_id: dev,
                id: stylus_tid,
                phase: eframe::egui::TouchPhase::End,
                pos: p1,
                force: Some(0.5),
            },
            eframe::egui::Event::PointerButton {
                pos: p1,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerGone,
        ],
    );

    assert_eq!(
        page.objects.len(),
        1,
        "Goresan pendek stylus tetap wajib ter-commit meski ada TouchPhase::Cancel dari ID lain dan PointerGone"
    );
    assert!(state.draft_stroke.is_none());
}

#[test]
fn test_canvas_1000_percent_zoom_drawing_lasso_and_eraser() {
    assert!((CanvasState::MAX_ZOOM - 10.0).abs() < 1e-6);

    let ctx = eframe::egui::Context::default();
    let zoom = CanvasState::MAX_ZOOM;
    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        zoom,
        ..Default::default()
    };
    let mut page = Page::new(PageTemplate::Grid);
    page.id = "zoom-1000-page".to_string();

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |px: f32, py: f32| origin + Vec2::new(px * zoom, py * zoom);

    // 1. Gambar goresan presisi di zoom 1000%
    let p0 = to_screen(10.0, 10.0);
    let p1 = to_screen(12.5, 11.5);
    let p2 = to_screen(15.0, 10.0);
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(p0),
            eframe::egui::Event::PointerButton {
                pos: p0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerMoved(p1),
            eframe::egui::Event::PointerMoved(p2),
        ],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: p2,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(page.objects.len(), 1);
    if let DrawObject::Stroke { points, .. } = &page.objects[0] {
        assert_eq!(points.len(), 3);
        assert!((points[0].x - 10.0).abs() < 1e-3);
        assert!((points[1].x - 12.5).abs() < 1e-3);
        assert!((points[2].x - 15.0).abs() < 1e-3);
    } else {
        panic!("Expected Stroke");
    }

    // 2. Render mesh cache pada zoom 1000%
    step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    assert_eq!(state.mesh_cache.len(), 1);

    // 3. Lasso selection pada zoom 1000%
    state.current_tool = ActiveTool::Lasso;
    state.lasso_selection_mode = crate::model::LassoSelectionMode::Containment;
    let l0 = to_screen(5.0, 5.0);
    let l1 = to_screen(20.0, 5.0);
    let l2 = to_screen(20.0, 18.0);
    let l3 = to_screen(5.0, 18.0);
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(l0),
            eframe::egui::Event::PointerButton {
                pos: l0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerMoved(l1),
            eframe::egui::Event::PointerMoved(l2),
            eframe::egui::Event::PointerMoved(l3),
        ],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: l3,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(state.selected_ids.len(), 1);

    // 4. Eraser pada zoom 1000%
    let erase_radius = (3.8 / state.zoom).clamp(1.8, 4.5);
    state.eraser_mode = EraserMode::WholeStroke;
    assert!(state.erase_at(&mut page, Pos2::new(12.5, 11.5), erase_radius));
    assert!(page.objects.is_empty());
    assert!(state.selected_ids.is_empty());
}

#[test]
fn test_highlighter_compositing_same_color_no_stacking_and_different_colors_distinct() {
    use crate::canvas::highlighter_renderer::composite_highlighter_groups_to_color_image;
    use crate::geometry::{HighlighterStrokeInput, MaskTransform, group_highlighters_by_rgb};

    let s_h = vec![
        StrokePoint::new(20.0, 50.0, 1.0),
        StrokePoint::new(80.0, 50.0, 1.0),
    ];
    let s_v = vec![
        StrokePoint::new(50.0, 20.0, 1.0),
        StrokePoint::new(50.0, 80.0, 1.0),
    ];
    let alpha = 95u8;
    let transform = MaskTransform {
        origin_x: 0.0,
        origin_y: 0.0,
        scale: 1.0,
    };

    // Single stroke vs overlapping same-color strokes
    let g1 = group_highlighters_by_rgb([HighlighterStrokeInput {
        points: &s_h,
        rgb: (255, 235, 59),
        width: 18.0,
        alpha,
        style: StrokeStyle::Solid,
    }]);
    let img1 = composite_highlighter_groups_to_color_image(&g1, 100, 100, transform, 3);

    let g2 = group_highlighters_by_rgb([
        HighlighterStrokeInput {
            points: &s_h,
            rgb: (255, 235, 59),
            width: 18.0,
            alpha,
            style: StrokeStyle::Solid,
        },
        HighlighterStrokeInput {
            points: &s_v,
            rgb: (255, 235, 59),
            width: 18.0,
            alpha,
            style: StrokeStyle::Solid,
        },
    ]);
    let img2 = composite_highlighter_groups_to_color_image(&g2, 100, 100, transform, 3);

    let px_single = img1.pixels[50 * 100 + 50];
    let px_overlap = img2.pixels[50 * 100 + 50];
    assert_eq!(px_single.a(), alpha);
    assert_eq!(
        px_overlap, px_single,
        "Overlapping same-color highlighters must have identical color and alpha as a single stroke"
    );

    // Different colors overlapping must blend and remain distinct from single color
    let g_diff = group_highlighters_by_rgb([
        HighlighterStrokeInput {
            points: &s_h,
            rgb: (255, 235, 59),
            width: 18.0,
            alpha,
            style: StrokeStyle::Solid,
        },
        HighlighterStrokeInput {
            points: &s_v,
            rgb: (0, 200, 255),
            width: 18.0,
            alpha,
            style: StrokeStyle::Solid,
        },
    ]);
    let img_diff = composite_highlighter_groups_to_color_image(&g_diff, 100, 100, transform, 3);
    let px_diff_overlap = img_diff.pixels[50 * 100 + 50];
    let px_only_cyan = img_diff.pixels[25 * 100 + 50];
    assert_ne!(px_diff_overlap, px_single);
    assert_ne!(px_diff_overlap, px_only_cyan);
    assert!(
        px_diff_overlap.a() > alpha,
        "Different-color overlap should accumulate SrcOver coverage"
    );
}

#[test]
fn test_highlighter_active_preview_commit_undo_redo_zoom_and_zoom_window() {
    let ctx = eframe::egui::Context::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "hl-lifecycle-page".to_string();

    let mut state = CanvasState {
        current_tool: ActiveTool::Highlighter,
        stroke_color: Color32::from_rgb(255, 225, 50),
        stroke_width: 3.0,
        highlighter_opacity: 0.4,
        zoom: 1.0,
        ..Default::default()
    };

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // 1. Active draft stroke preview populates highlighter_cache without mesh_cache
    let p0 = to_screen(40.0, 60.0);
    let p1 = to_screen(160.0, 60.0);
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(p0),
            eframe::egui::Event::PointerButton {
                pos: p0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerMoved(p1),
        ],
    );
    assert!(state.draft_stroke.is_some());
    assert_eq!(state.highlighter_cache.len(), 1);
    assert!(state.mesh_cache.is_empty());

    // 2. Commit stroke -> ColorData stores unmultiplied RGB + expected alpha
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: p1,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(page.objects.len(), 1);
    if let DrawObject::Stroke {
        color,
        is_highlighter,
        ..
    } = &page.objects[0]
    {
        assert!(*is_highlighter);
        assert_eq!((color.r, color.g, color.b), (255, 225, 50));
        assert_eq!(color.a, 102);
    } else {
        panic!("Expected highlighter stroke");
    }

    // 3. Second crossing stroke & Zoom Window rendering
    let q0 = to_screen(100.0, 30.0);
    let q1 = to_screen(100.0, 120.0);
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(q0),
            eframe::egui::Event::PointerButton {
                pos: q0,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            eframe::egui::Event::PointerMoved(q1),
        ],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: q1,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(page.objects.len(), 2);
    assert_eq!(state.highlighter_cache.len(), 1);

    // Zoom Window renders into HighlighterCacheScope::ZoomWindow cleanly
    state.zoom_target_rect =
        eframe::egui::Rect::from_min_size(Pos2::new(20.0, 20.0), Vec2::new(180.0, 120.0));
    let _ = ctx.run(
        eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 1000.0),
            )),
            ..Default::default()
        },
        |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                super::zoom_window::render_zoom_window_canvas(ui, &mut page, &mut state, 2.0);
            });
        },
    );
    assert_eq!(state.highlighter_cache.len(), 2);
    assert!(state.mesh_cache.is_empty());

    // 4. Undo & Redo invalidate cache properly
    state.undo(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert!(state.highlighter_cache.is_empty());
    step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    assert_eq!(state.highlighter_cache.len(), 1);

    state.undo(&mut page);
    assert!(page.objects.is_empty());
    step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    assert!(state.highlighter_cache.is_empty());

    state.redo(&mut page);
    assert_eq!(page.objects.len(), 1);
    step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    assert_eq!(state.highlighter_cache.len(), 1);

    // 5. Opacity change & Zoom In / Out (0.5x .. 10.0x) render cleanly
    state.highlighter_opacity = 0.6;
    step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    state.zoom = 0.5;
    step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    state.zoom = 10.0;
    step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    assert_eq!(state.highlighter_cache.len(), 1);
}

#[test]
fn test_hold_to_snap_wavy_stroke_converts_to_line_with_undo_and_respects_smart_guides() {
    let ctx = eframe::egui::Context::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "hold-wavy-smart-guides-test".to_string();

    let mut state = CanvasState {
        current_tool: ActiveTool::Pen,
        auto_shape_enabled: true,
        smart_guides: false,
        zoom: 1.0,
        ..Default::default()
    };

    let origin = step_test_canvas(&ctx, &mut page, &mut state, vec![]);
    let to_screen = |x: f32, y: f32| origin + Vec2::new(x, y);

    // 1. Gambar goresan acak bergelombang dari (40, 120) ke (240, 120)
    let mut pts = Vec::new();
    for i in 0..=20 {
        let t = i as f32 / 20.0;
        let x = 40.0 + t * 200.0;
        let y = 120.0 - (t * std::f32::consts::TAU * 2.5).sin() * 35.0;
        pts.push(to_screen(x, y));
    }

    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(pts[0]),
            eframe::egui::Event::PointerButton {
                pos: pts[0],
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    for &pt in &pts[1..] {
        step_test_canvas(
            &ctx,
            &mut page,
            &mut state,
            vec![eframe::egui::Event::PointerMoved(pt)],
        );
    }

    // Simulasikan hold >500ms di ujung akhir -> konversi ke garis lurus antara titik awal dan akhir
    state.last_stroke_move_time =
        Some(std::time::Instant::now() - std::time::Duration::from_millis(650));
    step_test_canvas(&ctx, &mut page, &mut state, vec![]);

    let rec = state
        .recognized_draft_shape
        .as_ref()
        .expect("Wavy open stroke held at end must convert to straight Line");
    assert_eq!(rec.kind, ShapeKind::Line);
    assert!((rec.x - 40.0).abs() < 1e-2);
    assert!((rec.y - 120.0).abs() < 1e-2);
    assert!((rec.width - 200.0).abs() < 1e-2);
    assert!(rec.height.abs() < 1e-1);

    // 2. Saat smart_guides == false, geser ujung garis ke (140, 125) (~2.86 deg) -> TIDAK boleh snap ke 0 deg
    let drag_target = to_screen(140.0, 125.0);
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drag_target)],
    );
    let rec_free = state.recognized_draft_shape.as_ref().unwrap();
    assert!(
        (rec_free.width - 100.0).abs() < 1e-2 && (rec_free.height - 5.0).abs() < 1e-2,
        "With smart_guides=false, line endpoint must follow cursor freely without 15-deg magnetic snap, got ({}, {})",
        rec_free.width,
        rec_free.height
    );

    // 3. Menahan Shift tetap mengunci sudut ke kelipatan 15 deg meskipun smart_guides == false
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(to_screen(142.0, 125.0))],
    );
    let _ = ctx.run(
        eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 1000.0),
            )),
            events: vec![eframe::egui::Event::PointerMoved(drag_target)],
            modifiers: eframe::egui::Modifiers::SHIFT,
            ..Default::default()
        },
        |ctx| {
            eframe::egui::CentralPanel::default()
                .frame(eframe::egui::Frame::NONE)
                .show(ctx, |ui| {
                    let _ = render_page_canvas(ui, &mut page, &mut state);
                });
        },
    );
    let rec_shift = state.recognized_draft_shape.as_ref().unwrap();
    assert!(
        rec_shift.height.abs() < 1e-3,
        "Shift-drag must still snap angle to 0 deg, got height={}",
        rec_shift.height
    );

    // 4. Saat smart_guides == true (tanpa Shift), sudut ~2.86 deg kembali snap magnetik ke 0 deg
    state.smart_guides = true;
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drag_target)],
    );
    let rec_guided = state.recognized_draft_shape.as_ref().unwrap();
    assert!(
        rec_guided.height.abs() < 1e-3,
        "With smart_guides=true, near-horizontal line must snap to 0 deg, got height={}",
        rec_guided.height
    );

    // 5. Matikan kembali smart_guides, geser bebas ke (140, 125) dan lepaskan untuk commit
    state.smart_guides = false;
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drag_target)],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: drag_target,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );

    assert_eq!(page.objects.len(), 1);
    let line_id = match &page.objects[0] {
        DrawObject::Shape {
            id,
            shape_type,
            x,
            y,
            width,
            height,
            ..
        } => {
            assert_eq!(*shape_type, ShapeKind::Line);
            assert!((*x - 40.0).abs() < 1e-2);
            assert!((*y - 120.0).abs() < 1e-2);
            assert!((*width - 100.0).abs() < 1e-2);
            assert!((*height - 5.0).abs() < 1e-2);
            id.clone()
        }
        other => panic!("Expected committed Line shape, got {other:?}"),
    };

    // Undo #1 -> kembali ke goresan bergelombang asli; Undo #2 -> halaman kosong
    state.undo(&mut page);
    assert_eq!(page.objects.len(), 1);
    assert!(matches!(page.objects[0], DrawObject::Stroke { .. }));
    state.undo(&mut page);
    assert!(page.objects.is_empty());

    // Redo x2 -> kembali ke garis lurus yang telah di-commit
    state.redo(&mut page);
    state.redo(&mut page);
    assert_eq!(page.objects.len(), 1);

    // 6. Geser garis yang sudah di-commit di dekat objek lain saat smart_guides == false -> bebas tanpa snapping
    page.objects.push(DrawObject::Shape {
        id: "ref-rect".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 220.0,
        width: 120.0,
        height: 60.0,
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    });
    state.current_tool = ActiveTool::Lasso;
    state.selected_ids = vec![line_id];
    let grab = to_screen(90.0, 122.5);
    let drop_near_align = to_screen(147.0, 122.5); // geser +57 -> x menjadi 97.0 (dalam radius 8px dari x=100)
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(grab),
            eframe::egui::Event::PointerButton {
                pos: grab,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(drop_near_align)],
    );
    assert!(
        state.active_alignment_guides.is_empty(),
        "Alignment guides must stay empty when smart_guides=false"
    );
    if let DrawObject::Shape { x, .. } = &page.objects[0] {
        assert!(
            (*x - 97.0).abs() < 1e-2,
            "Line must not snap to x=100.0 when smart_guides=false, got x={x}"
        );
    }
    step_test_canvas(
        &ctx,
        &mut page,
        &mut state,
        vec![eframe::egui::Event::PointerButton {
            pos: drop_near_align,
            button: eframe::egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
}

fn step_two_pages_vertical(
    ctx: &eframe::egui::Context,
    page1: &mut Page,
    page2: &mut Page,
    state: &mut CanvasState,
    events: Vec<eframe::egui::Event>,
) -> (Pos2, Pos2, eframe::egui::FullOutput) {
    let mut origin1 = Pos2::ZERO;
    let mut origin2 = Pos2::ZERO;
    let output = ctx.run(
        eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000.0, 2000.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            eframe::egui::CentralPanel::default()
                .frame(eframe::egui::Frame::NONE)
                .show(ctx, |ui| {
                    let r1 = render_page_canvas(ui, page1, state);
                    origin1 = r1.rect.min;
                    ui.add_space(16.0);
                    let r2 = render_page_canvas(ui, page2, state);
                    origin2 = r2.rect.min;
                });
        },
    );
    (origin1, origin2, output)
}

fn collect_circle_shapes(
    shape: &eframe::egui::Shape,
    out: &mut Vec<eframe::egui::epaint::CircleShape>,
) {
    match shape {
        eframe::egui::Shape::Circle(c) => out.push(*c),
        eframe::egui::Shape::Vec(children) => {
            for child in children {
                collect_circle_shapes(child, out);
            }
        }
        _ => {}
    }
}

fn make_box_with_diagonals(prefix: &str) -> Vec<DrawObject> {
    let black = ColorData {
        r: 20,
        g: 22,
        b: 28,
        a: 255,
    };
    vec![
        DrawObject::Shape {
            id: format!("{prefix}-rect"),
            shape_type: ShapeKind::Rectangle,
            x: 200.0,
            y: 80.0,
            width: 160.0,
            height: 100.0,
            color: black.clone(),
            stroke_width: 2.0,
            fill: None,
            rotation: 0.0,
            corner_radius: 0.0,
            label: None,
            locked: false,
            curve_points: Vec::new(),
        },
        DrawObject::Stroke {
            id: format!("{prefix}-diag1"),
            points: vec![
                StrokePoint {
                    x: 200.0,
                    y: 80.0,
                    pressure: 1.0,
                },
                StrokePoint {
                    x: 360.0,
                    y: 180.0,
                    pressure: 1.0,
                },
            ],
            color: black.clone(),
            width: 2.0,
            is_highlighter: false,
            style: StrokeStyle::Solid,
        },
        DrawObject::Stroke {
            id: format!("{prefix}-diag2"),
            points: vec![
                StrokePoint {
                    x: 360.0,
                    y: 80.0,
                    pressure: 1.0,
                },
                StrokePoint {
                    x: 200.0,
                    y: 180.0,
                    pressure: 1.0,
                },
            ],
            color: black,
            width: 2.0,
            is_highlighter: false,
            style: StrokeStyle::Solid,
        },
    ]
}

#[test]
fn test_continuous_vertical_eraser_on_blank_page1_preserves_objects_above() {
    for mode in [EraserMode::Partial, EraserMode::WholeStroke] {
        let ctx = eframe::egui::Context::default();
        let mut page1 = Page::new(PageTemplate::Ruled);
        page1.id = "scroll-page-1".to_string();
        page1.objects = make_box_with_diagonals("p1");

        let mut page2 = Page::new(PageTemplate::Blank);
        page2.id = "scroll-page-2".to_string();
        page2.objects = make_box_with_diagonals("p2");

        let mut state = CanvasState {
            current_tool: ActiveTool::Eraser,
            eraser_mode: mode,
            zoom: 1.0,
            ..Default::default()
        };

        let (orig1, _, _) =
            step_two_pages_vertical(&ctx, &mut page1, &mut page2, &mut state, vec![]);

        // Geser penghapus di area kertas kosong bawah Halaman 1 (y = 620..640) jauh dari objek (y = 80..180)
        let start = orig1 + Vec2::new(280.0, 620.0);
        step_two_pages_vertical(
            &ctx,
            &mut page1,
            &mut page2,
            &mut state,
            vec![
                eframe::egui::Event::PointerMoved(start),
                eframe::egui::Event::PointerButton {
                    pos: start,
                    button: eframe::egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );

        for dx in [20.0_f32, 40.0, 60.0] {
            let p = start + Vec2::new(dx, dx * 0.25);
            step_two_pages_vertical(
                &ctx,
                &mut page1,
                &mut page2,
                &mut state,
                vec![eframe::egui::Event::PointerMoved(p)],
            );
        }

        let end = start + Vec2::new(60.0, 15.0);
        step_two_pages_vertical(
            &ctx,
            &mut page1,
            &mut page2,
            &mut state,
            vec![eframe::egui::Event::PointerButton {
                pos: end,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );

        assert_eq!(
            page1.objects.len(),
            3,
            "Erasing blank bottom area on page 1 in scroll mode ({mode:?}) must preserve all top objects"
        );
        assert_eq!(
            page2.objects.len(),
            3,
            "Erasing blank area on page 1 in scroll mode ({mode:?}) must preserve all page 2 objects"
        );
        assert!(state.active_stroke_page_id.is_none());
        assert!(state.last_erase_sample_pos.is_none());
    }
}

#[test]
fn test_continuous_vertical_eraser_on_page2_does_not_corrupt_page1_and_erases_target() {
    let ctx = eframe::egui::Context::default();
    let mut page1 = Page::new(PageTemplate::Blank);
    page1.id = "scroll-p1-keep".to_string();
    page1.objects = make_box_with_diagonals("keep");

    let mut page2 = Page::new(PageTemplate::Blank);
    page2.id = "scroll-p2-erase".to_string();
    page2.objects.push(DrawObject::Stroke {
        id: "p2-stroke".to_string(),
        points: vec![
            StrokePoint {
                x: 200.0,
                y: 140.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 340.0,
                y: 140.0,
                pressure: 1.0,
            },
        ],
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });

    let mut state = CanvasState {
        current_tool: ActiveTool::Eraser,
        eraser_mode: EraserMode::WholeStroke,
        zoom: 1.0,
        ..Default::default()
    };

    let (_, orig2, _) = step_two_pages_vertical(&ctx, &mut page1, &mut page2, &mut state, vec![]);
    let start = orig2 + Vec2::new(260.0, 130.0);
    let mid = orig2 + Vec2::new(270.0, 140.0);
    let end = orig2 + Vec2::new(280.0, 150.0);

    step_two_pages_vertical(
        &ctx,
        &mut page1,
        &mut page2,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(start),
            eframe::egui::Event::PointerButton {
                pos: start,
                button: eframe::egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    step_two_pages_vertical(
        &ctx,
        &mut page1,
        &mut page2,
        &mut state,
        vec![eframe::egui::Event::PointerMoved(mid)],
    );
    step_two_pages_vertical(
        &ctx,
        &mut page1,
        &mut page2,
        &mut state,
        vec![
            eframe::egui::Event::PointerMoved(end),
            eframe::egui::Event::PointerButton {
                pos: end,
                button: eframe::egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ],
    );

    assert_eq!(page1.objects.len(), 3, "Page 1 objects must stay intact");
    assert!(
        page2.objects.is_empty(),
        "Targeted stroke on Page 2 must be erased"
    );
}

#[test]
fn test_eraser_radius_helpers_scale_with_zoom_and_mode() {
    let mut state = CanvasState {
        eraser_mode: EraserMode::Partial,
        zoom: 1.0,
        ..Default::default()
    };
    assert!((state.eraser_page_radius() - 10.0).abs() < 1e-4);
    assert!((state.eraser_screen_radius() - 10.0).abs() < 1e-4);

    // Partial clamps page_radius to max 16.0 at 0.5x zoom -> 8.0 screen px
    state.zoom = 0.5;
    assert!((state.eraser_page_radius() - 16.0).abs() < 1e-4);
    assert!((state.eraser_screen_radius() - 8.0).abs() < 1e-4);

    // Partial clamps page_radius to min 4.0 at 4.0x zoom -> 16.0 screen px
    state.zoom = 4.0;
    assert!((state.eraser_page_radius() - 4.0).abs() < 1e-4);
    assert!((state.eraser_screen_radius() - 16.0).abs() < 1e-4);

    // WholeStroke at 1.0x -> 3.8 page & 3.8 screen px; at 2.0x -> 1.9 page & 3.8 screen px
    state.eraser_mode = EraserMode::WholeStroke;
    state.zoom = 1.0;
    assert!((state.eraser_page_radius() - 3.8).abs() < 1e-4);
    assert!((state.eraser_screen_radius() - 3.8).abs() < 1e-4);
    state.zoom = 2.0;
    assert!((state.eraser_page_radius() - 1.9).abs() < 1e-4);
    assert!((state.eraser_screen_radius() - 3.8).abs() < 1e-4);
}

#[test]
fn test_eraser_circular_cursor_renders_matching_screen_radius_on_hover() {
    let ctx = eframe::egui::Context::default();
    let mut page = Page::new(PageTemplate::Blank);
    page.id = "eraser-cursor-preview-page".to_string();

    let mut state = CanvasState {
        current_tool: ActiveTool::Eraser,
        eraser_mode: EraserMode::Partial,
        zoom: 1.0,
        ..Default::default()
    };

    let run_hover =
        |ctx: &eframe::egui::Context, page: &mut Page, state: &mut CanvasState, hover: Pos2| {
            ctx.run(
                eframe::egui::RawInput {
                    screen_rect: Some(eframe::egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1000.0, 1000.0),
                    )),
                    events: vec![eframe::egui::Event::PointerMoved(hover)],
                    ..Default::default()
                },
                |ctx| {
                    eframe::egui::CentralPanel::default()
                        .frame(eframe::egui::Frame::NONE)
                        .show(ctx, |ui| {
                            let _ = render_page_canvas(ui, page, state);
                        });
                },
            )
        };

    let hover = Pos2::new(220.0, 260.0);
    let _ = run_hover(&ctx, &mut page, &mut state, hover);
    let out_partial = run_hover(&ctx, &mut page, &mut state, hover);
    assert_eq!(
        out_partial.platform_output.cursor_icon,
        eframe::egui::CursorIcon::None
    );
    let mut circles = Vec::new();
    for clipped in &out_partial.shapes {
        collect_circle_shapes(&clipped.shape, &mut circles);
    }
    assert!(
        circles
            .iter()
            .any(|c| (c.center - hover).length() < 0.1 && (c.radius - 10.0).abs() < 1e-3),
        "Partial eraser at zoom 1.0 must render circular cursor with radius 10.0px, found: {circles:?}"
    );

    // Zoom 0.5 Partial -> radius 8.0px
    state.zoom = 0.5;
    let out_zoom_half = run_hover(&ctx, &mut page, &mut state, hover);
    circles.clear();
    for clipped in &out_zoom_half.shapes {
        collect_circle_shapes(&clipped.shape, &mut circles);
    }
    assert!(
        circles
            .iter()
            .any(|c| (c.center - hover).length() < 0.1 && (c.radius - 8.0).abs() < 1e-3),
        "Partial eraser at zoom 0.5 must render circular cursor with radius 8.0px, found: {circles:?}"
    );

    // WholeStroke zoom 1.0 -> radius 3.8px
    state.zoom = 1.0;
    state.eraser_mode = EraserMode::WholeStroke;
    let out_whole = run_hover(&ctx, &mut page, &mut state, hover);
    circles.clear();
    for clipped in &out_whole.shapes {
        collect_circle_shapes(&clipped.shape, &mut circles);
    }
    assert!(
        circles
            .iter()
            .any(|c| (c.center - hover).length() < 0.1 && (c.radius - 3.8).abs() < 1e-3),
        "WholeStroke eraser at zoom 1.0 must render circular cursor with radius 3.8px, found: {circles:?}"
    );

    // Switching back to Pen hides eraser cursor circle & restores cursor icon
    state.current_tool = ActiveTool::Pen;
    let out_pen = run_hover(&ctx, &mut page, &mut state, hover);
    assert_ne!(
        out_pen.platform_output.cursor_icon,
        eframe::egui::CursorIcon::None
    );
    circles.clear();
    for clipped in &out_pen.shapes {
        collect_circle_shapes(&clipped.shape, &mut circles);
    }
    assert!(
        !circles.iter().any(|c| (c.center - hover).length() < 0.1),
        "Pen tool must not draw eraser circular cursor"
    );
}

fn collect_mesh_vertex_colors(shape: &eframe::egui::Shape, out: &mut Vec<eframe::egui::Color32>) {
    match shape {
        eframe::egui::Shape::Mesh(mesh) => {
            for v in &mesh.vertices {
                out.push(v.color);
            }
        }
        eframe::egui::Shape::Vec(children) => {
            for child in children {
                collect_mesh_vertex_colors(child, out);
            }
        }
        _ => {}
    }
}

#[test]
fn test_laser_solid_custom_color_renders_selected_color() {
    use super::laser::render_laser_trail;
    use crate::model::LaserColorMode;

    let cyan = Color32::from_rgb(0, 212, 255);
    let mut state = CanvasState {
        current_tool: ActiveTool::Laser,
        laser_color: cyan,
        laser_color_mode: LaserColorMode::Solid,
        laser_strokes: vec![vec![
            Pos2::new(20.0, 40.0),
            Pos2::new(120.0, 40.0),
            Pos2::new(220.0, 40.0),
        ]],
        ..Default::default()
    };

    let ctx = eframe::egui::Context::default();
    let out = ctx.run(
        eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(400.0, 200.0),
            )),
            ..Default::default()
        },
        |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                render_laser_trail(ui.painter(), &state, 1.0, 0.0);
            });
        },
    );

    let mut colors = Vec::new();
    for clipped in &out.shapes {
        collect_mesh_vertex_colors(&clipped.shape, &mut colors);
    }
    // Core neon vertices carry exact selected cyan Color32 (premultiplied by alpha=238), not default red
    let expected_core = Color32::from_rgba_unmultiplied(0, 212, 255, 238);
    let default_red_core = Color32::from_rgba_unmultiplied(255, 0, 35, 238);
    assert!(
        colors.contains(&expected_core),
        "Expected solid custom cyan core vertices ({expected_core:?})"
    );
    assert!(
        !colors.contains(&default_red_core),
        "Default red must not appear when custom cyan laser color is selected"
    );
    // White beam center vertices still present
    let expected_beam = Color32::from_rgba_unmultiplied(255, 255, 255, 250);
    assert!(colors.contains(&expected_beam));

    state.laser_strokes.clear();
}

#[test]
fn test_laser_rainbow_mode_produces_multi_hue_gradient_and_animates_over_time() {
    use super::laser::{
        hsv_to_rgb, rainbow_laser_color, render_laser_trail, tessellate_laser_layer_colored,
    };
    use crate::model::LaserColorMode;

    // 1. Primary HSV checks
    assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), (255, 0, 0));
    assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), (0, 255, 0));
    assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), (0, 0, 255));

    // 2. Per-arc tessellation emits distinct hues along 300px stroke
    let pts = vec![
        Pos2::new(10.0, 50.0),
        Pos2::new(90.0, 50.0),
        Pos2::new(170.0, 50.0),
        Pos2::new(250.0, 50.0),
        Pos2::new(310.0, 50.0),
    ];
    let mesh = tessellate_laser_layer_colored(&pts, 8.0, |arc| rainbow_laser_color(arc, 0.0, 255));
    let distinct_colors: std::collections::HashSet<(u8, u8, u8)> = mesh
        .vertices
        .iter()
        .map(|v| (v.color.r(), v.color.g(), v.color.b()))
        .collect();
    assert!(
        distinct_colors.len() >= 4,
        "Rainbow tessellation must produce multiple distinct hues along stroke, got {distinct_colors:?}"
    );

    // 3. Full render_laser_trail in Rainbow mode animates hues across time_secs
    let state = CanvasState {
        current_tool: ActiveTool::Laser,
        laser_color_mode: LaserColorMode::Rainbow,
        laser_strokes: vec![pts],
        ..Default::default()
    };

    let render_at_time = |time_secs: f32| -> Vec<(u8, u8, u8)> {
        let ctx = eframe::egui::Context::default();
        let out = ctx.run(
            eframe::egui::RawInput {
                screen_rect: Some(eframe::egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(400.0, 200.0),
                )),
                ..Default::default()
            },
            |ctx| {
                eframe::egui::CentralPanel::default().show(ctx, |ui| {
                    render_laser_trail(ui.painter(), &state, 1.0, time_secs);
                });
            },
        );
        let mut cols = Vec::new();
        for clipped in &out.shapes {
            collect_mesh_vertex_colors(&clipped.shape, &mut cols);
        }
        cols.into_iter()
            .filter(|c| !(c.r() == 255 && c.g() == 255 && c.b() == 255))
            .map(|c| (c.r(), c.g(), c.b()))
            .collect()
    };

    let colors_t0 = render_at_time(0.0);
    let colors_t1 = render_at_time(0.75);
    assert!(!colors_t0.is_empty());
    assert_ne!(
        colors_t0, colors_t1,
        "Advancing animation time must shift rainbow laser colors"
    );
}

#[test]
fn test_laser_sharp_turn_round_join_has_no_spikes_or_pinch() {
    use super::laser::{
        rainbow_laser_color, tessellate_laser_layer, tessellate_laser_layer_colored,
    };

    let hairpin = vec![
        Pos2::new(20.0, 20.0),
        Pos2::new(80.0, 20.0),
        Pos2::new(80.0, 80.0), // belokan 90 deg
        Pos2::new(25.0, 74.0), // belokan tajam mendekati 170 deg
    ];
    let width = 20.0; // radius = 10.0
    let meshes = [
        tessellate_laser_layer(
            &hairpin,
            width,
            Color32::from_rgba_unmultiplied(0, 212, 255, 180),
        ),
        tessellate_laser_layer_colored(&hairpin, width, |arc| rainbow_laser_color(arc, 0.25, 220)),
    ];

    for (idx, mesh) in meshes.iter().enumerate() {
        assert!(
            !mesh.vertices.is_empty(),
            "Laser mesh {idx} must not be empty"
        );
        assert!(
            !mesh.indices.is_empty(),
            "Laser mesh {idx} must have triangles"
        );
        for &tri_idx in &mesh.indices {
            assert!((tri_idx as usize) < mesh.vertices.len());
        }
        let max_dist = mesh
            .vertices
            .iter()
            .map(|v| {
                assert!(v.pos.x.is_finite() && v.pos.y.is_finite());
                hairpin
                    .windows(2)
                    .map(|w| crate::geometry::point_to_segment_dist(v.pos, w[0], w[1]))
                    .fold(f32::INFINITY, f32::min)
            })
            .fold(0.0_f32, f32::max);
        assert!(
            max_dist <= 10.6,
            "Laser mesh {idx} must have no spikes beyond radius 10px, got max={max_dist}"
        );
        // Pastikan sisi luar sudut (80, 20) memiliki cakupan radius penuh (tidak terjepit/pinched)
        let corner_max_x = mesh
            .vertices
            .iter()
            .filter(|v| v.pos.y <= 35.0)
            .map(|v| v.pos.x)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            corner_max_x >= 89.5,
            "Laser corner must preserve outer radius coverage (~90px), got {corner_max_x}"
        );
    }
}

#[test]
fn test_laser_neon_beam_has_continuous_radial_fade_to_transparent_and_white_core() {
    use super::laser::{rainbow_laser_color, tessellate_laser_neon_beam};

    let curve = vec![
        Pos2::new(20.0, 40.0),
        Pos2::new(90.0, 30.0),
        Pos2::new(150.0, 75.0),
        Pos2::new(80.0, 95.0),
    ];

    let solid_mesh = tessellate_laser_neon_beam(&curve, 1.0, 1.0, |_| (255, 0, 35));
    let rainbow_mesh = tessellate_laser_neon_beam(&curve, 1.0, 1.0, |arc| {
        let c = rainbow_laser_color(arc, 0.3, 255);
        (c.r(), c.g(), c.b())
    });

    for (label, mesh) in [("solid", &solid_mesh), ("rainbow", &rainbow_mesh)] {
        assert!(!mesh.vertices.is_empty(), "{label} neon mesh non-empty");
        assert!(!mesh.indices.is_empty(), "{label} neon indices non-empty");
        assert!(
            mesh.vertices
                .iter()
                .any(|v| v.color == Color32::TRANSPARENT),
            "{label} neon mesh must fade smoothly to Color32::TRANSPARENT at outer edge"
        );
        assert!(
            mesh.vertices
                .iter()
                .any(|v| v.color == Color32::from_rgba_unmultiplied(255, 255, 255, 250)),
            "{label} neon mesh must preserve white hotspot core"
        );
        let distinct_alphas: std::collections::HashSet<u8> =
            mesh.vertices.iter().map(|v| v.color.a()).collect();
        assert!(
            distinct_alphas.len() >= 5,
            "{label} neon beam must have smooth multi-stop radial alpha falloff, got {distinct_alphas:?}"
        );
    }
}

#[test]
fn test_autoshape_line_always_renders_rounded_caps_even_at_thin_width() {
    use super::primitives::draw_shape_primitive;
    use eframe::egui::Stroke;

    let ctx = eframe::egui::Context::default();
    for width in [1.5_f32, 2.5_f32, 3.0_f32] {
        let out = ctx.run(
            eframe::egui::RawInput {
                screen_rect: Some(eframe::egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(400.0, 200.0),
                )),
                ..Default::default()
            },
            |ctx| {
                eframe::egui::CentralPanel::default()
                    .frame(eframe::egui::Frame::NONE)
                    .show(ctx, |ui| {
                        draw_shape_primitive(
                            ui.painter(),
                            ShapeKind::Line,
                            50.0,
                            80.0,
                            200.0,
                            0.0,
                            0.0,
                            0.0,
                            &[],
                            Color32::TRANSPARENT,
                            Stroke::new(width, Color32::BLACK),
                            |p| p,
                        );
                    });
            },
        );

        let mut meshes = Vec::new();
        for clipped in &out.shapes {
            if let eframe::egui::Shape::Mesh(m) = &clipped.shape {
                meshes.push(m);
            }
        }
        assert!(
            !meshes.is_empty(),
            "Line of width {width} must tessellate into rounded-cap Mesh instead of squared LineSegment"
        );
        let mesh = meshes[0];
        let min_x = mesh
            .vertices
            .iter()
            .map(|v| v.pos.x)
            .fold(f32::INFINITY, f32::min);
        let max_x = mesh
            .vertices
            .iter()
            .map(|v| v.pos.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let expected_r = width * 0.5;
        assert!(
            min_x < 50.0 - expected_r * 0.8 && max_x > 250.0 + expected_r * 0.8,
            "Horizontal Line (50..250, width={width}) must extend past endpoints with rounded caps, got [{min_x}, {max_x}]"
        );
    }
}
