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
