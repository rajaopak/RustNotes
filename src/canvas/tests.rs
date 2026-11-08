use super::*;
use crate::geometry::{smooth_stroke_points, tessellate_stroke};
use crate::model::{ColorData, DrawObject, Page, PageTemplate, ShapeKind, StrokePoint};
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
    });
    page.objects.push(DrawObject::Text {
        id: "t1".to_string(),
        x: 200.0,
        y: 200.0,
        text: "Hello".to_string(),
        font_size: 16.0,
        color: ColorData::from(Color32::BLACK),
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
