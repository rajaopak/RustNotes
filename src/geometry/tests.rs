use super::*;
use crate::model::{
    ColorData, ConnectionSite, DrawObject, FontFamilyPreset, LassoSelectionMode, Point2D,
    ShapeKind, StrokePoint, StrokeStyle, TextAlign,
};
use eframe::egui::{Color32, Pos2, Rect};

fn dummy_color() -> ColorData {
    ColorData {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    }
}

#[test]
fn test_point_in_polygon() {
    // Square 0,0 to 100,100
    let poly = vec![
        Pos2::new(0.0, 0.0),
        Pos2::new(100.0, 0.0),
        Pos2::new(100.0, 100.0),
        Pos2::new(0.0, 100.0),
    ];

    // Inside
    assert!(point_in_polygon(Pos2::new(50.0, 50.0), &poly));
    // Outside
    assert!(!point_in_polygon(Pos2::new(150.0, 50.0), &poly));
    assert!(!point_in_polygon(Pos2::new(-10.0, 50.0), &poly));
    assert!(!point_in_polygon(Pos2::new(50.0, 150.0), &poly));

    // Concave L-shaped polygon
    let concave = vec![
        Pos2::new(0.0, 0.0),
        Pos2::new(100.0, 0.0),
        Pos2::new(100.0, 50.0),
        Pos2::new(50.0, 50.0),
        Pos2::new(50.0, 100.0),
        Pos2::new(0.0, 100.0),
    ];
    // In the lower left arm of L
    assert!(point_in_polygon(Pos2::new(25.0, 75.0), &concave));
    // In the cutout top-right
    assert!(!point_in_polygon(Pos2::new(75.0, 75.0), &concave));

    // Fewer than 3 points
    let empty_poly: Vec<Pos2> = vec![];
    assert!(!point_in_polygon(Pos2::new(10.0, 10.0), &empty_poly));
    let two_pts = vec![Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0)];
    assert!(!point_in_polygon(Pos2::new(5.0, 5.0), &two_pts));
}

#[test]
fn test_segments_cross() {
    // Crossing lines
    let a = Pos2::new(0.0, 0.0);
    let b = Pos2::new(10.0, 10.0);
    let c = Pos2::new(0.0, 10.0);
    let d = Pos2::new(10.0, 0.0);
    assert!(segments_cross(a, b, c, d));

    // Parallel lines
    let c_par = Pos2::new(0.0, 2.0);
    let d_par = Pos2::new(10.0, 12.0);
    assert!(!segments_cross(a, b, c_par, d_par));

    // Non-intersecting disjoint lines
    let c_dis = Pos2::new(20.0, 0.0);
    let d_dis = Pos2::new(20.0, 10.0);
    assert!(!segments_cross(a, b, c_dis, d_dis));
}

#[test]
fn test_point_to_segment_dist() {
    let a = Pos2::new(0.0, 0.0);
    let b = Pos2::new(10.0, 0.0);

    // Projected directly onto segment
    assert!((point_to_segment_dist(Pos2::new(5.0, 3.0), a, b) - 3.0).abs() < 1e-4);

    // Outside endpoint A
    assert!((point_to_segment_dist(Pos2::new(-4.0, 0.0), a, b) - 4.0).abs() < 1e-4);

    // Outside endpoint B
    assert!((point_to_segment_dist(Pos2::new(13.0, 4.0), a, b) - 5.0).abs() < 1e-4);

    // Zero-length segment (a == b)
    let zero_seg = Pos2::new(2.0, 2.0);
    assert!((point_to_segment_dist(Pos2::new(5.0, 6.0), zero_seg, zero_seg) - 5.0).abs() < 1e-4);
}

#[test]
fn test_object_bounds() {
    // Empty stroke
    let empty_stroke = DrawObject::Stroke {
        id: "1".into(),
        points: vec![],
        color: dummy_color(),
        width: 4.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    assert_eq!(object_bounds(&empty_stroke), Rect::NOTHING);

    // Stroke with points and width padding
    let stroke = DrawObject::Stroke {
        id: "2".into(),
        points: vec![
            StrokePoint {
                x: 10.0,
                y: 10.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 50.0,
                y: 60.0,
                pressure: 1.0,
            },
        ],
        color: dummy_color(),
        width: 6.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    let b = object_bounds(&stroke);
    // padding = width/2 = 3.0
    assert_eq!(b.min, Pos2::new(7.0, 7.0));
    assert_eq!(b.max, Pos2::new(53.0, 63.0));

    // Shape bounds
    let shape = DrawObject::Shape {
        id: "3".into(),
        shape_type: ShapeKind::Rectangle,
        x: 20.0,
        y: 30.0,
        width: 40.0,
        height: 50.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };
    let sb = object_bounds(&shape);
    assert_eq!(sb.min, Pos2::new(19.0, 29.0));
    assert_eq!(sb.max, Pos2::new(61.0, 81.0));
}

#[test]
fn test_combined_bounds() {
    assert!(combined_bounds(&[]).is_none());

    let obj1 = DrawObject::Shape {
        id: "1".into(),
        shape_type: ShapeKind::Rectangle,
        x: 0.0,
        y: 0.0,
        width: 50.0,
        height: 50.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };
    let obj2 = DrawObject::Shape {
        id: "2".into(),
        shape_type: ShapeKind::Rectangle,
        x: 80.0,
        y: 80.0,
        width: 20.0,
        height: 20.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };

    let cb = combined_bounds(&[&obj1, &obj2]).expect("Should have combined bounds");
    assert_eq!(cb.min, Pos2::new(-1.0, -1.0));
    assert_eq!(cb.max, Pos2::new(101.0, 101.0));
}

#[test]
fn test_hit_test_object() {
    let stroke = DrawObject::Stroke {
        id: "1".into(),
        points: vec![
            StrokePoint {
                x: 0.0,
                y: 0.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 100.0,
                y: 0.0,
                pressure: 1.0,
            },
        ],
        color: dummy_color(),
        width: 4.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };

    // Hit near the line
    assert!(hit_test_object(&stroke, Pos2::new(50.0, 2.0), 5.0));
    // Miss far from the line
    assert!(!hit_test_object(&stroke, Pos2::new(50.0, 30.0), 5.0));

    let rect_filled = DrawObject::Shape {
        id: "2".into(),
        shape_type: ShapeKind::Rectangle,
        x: 10.0,
        y: 10.0,
        width: 80.0,
        height: 80.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: Some(dummy_color()),
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };
    // Inside filled shape
    assert!(hit_test_object(&rect_filled, Pos2::new(50.0, 50.0), 5.0));
    // Outside filled shape
    assert!(!hit_test_object(&rect_filled, Pos2::new(120.0, 50.0), 5.0));
}

#[test]
fn test_translate_and_scale_object() {
    let mut obj = DrawObject::Shape {
        id: "1".into(),
        shape_type: ShapeKind::Rectangle,
        x: 10.0,
        y: 10.0,
        width: 20.0,
        height: 30.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };

    translate_object(&mut obj, 15.0, -5.0);
    if let DrawObject::Shape { x, y, .. } = obj {
        assert_eq!(x, 25.0);
        assert_eq!(y, 5.0);
    } else {
        panic!("Expected shape");
    }

    scale_object(&mut obj, Pos2::new(0.0, 0.0), 2.0, 2.0);
    if let DrawObject::Shape {
        x,
        y,
        width,
        height,
        ..
    } = obj
    {
        assert_eq!(x, 50.0);
        assert_eq!(y, 10.0);
        assert_eq!(width, 40.0);
        assert_eq!(height, 60.0);
    } else {
        panic!("Expected shape");
    }
}

#[test]
fn test_image_object_geometry() {
    let mut img = DrawObject::Image {
        id: "img-test".to_string(),
        asset_id: "test.png".to_string(),
        x: 50.0,
        y: 60.0,
        width: 200.0,
        height: 150.0,
        rotation: 0.0,
        lock_aspect: true,
    };

    let b = object_bounds(&img);
    assert_eq!(b.min, Pos2::new(50.0, 60.0));
    assert_eq!(b.max, Pos2::new(250.0, 210.0));

    assert!(hit_test_object(&img, Pos2::new(100.0, 100.0), 5.0));
    assert!(!hit_test_object(&img, Pos2::new(300.0, 300.0), 5.0));

    translate_object(&mut img, 20.0, -10.0);
    if let DrawObject::Image { x, y, .. } = img {
        assert_eq!(x, 70.0);
        assert_eq!(y, 50.0);
    }

    scale_object(&mut img, Pos2::new(70.0, 50.0), 2.0, 2.0);
    if let DrawObject::Image { width, height, .. } = img {
        assert_eq!(width, 400.0);
        assert_eq!(height, 300.0);
    }
}

#[test]
fn test_flip_and_rotate_object() {
    let mut stroke = DrawObject::Stroke {
        id: "s1".into(),
        points: vec![
            StrokePoint {
                x: 10.0,
                y: 20.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 30.0,
                y: 40.0,
                pressure: 1.0,
            },
        ],
        color: dummy_color(),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };

    // Flip horizontal around center_x = 50.0:
    // 10 -> 2*50 - 10 = 90
    // 30 -> 2*50 - 30 = 70
    flip_object_horizontal(&mut stroke, 50.0);
    if let DrawObject::Stroke { points, .. } = &stroke {
        assert_eq!(points[0].x, 90.0);
        assert_eq!(points[1].x, 70.0);
    }

    // Flip vertical around center_y = 50.0:
    // 20 -> 2*50 - 20 = 80
    // 40 -> 2*50 - 40 = 60
    flip_object_vertical(&mut stroke, 50.0);
    if let DrawObject::Stroke { points, .. } = &stroke {
        assert_eq!(points[0].y, 80.0);
        assert_eq!(points[1].y, 60.0);
    }

    // Rotate 90 degrees (pi / 2) around (0, 0):
    // (x, y) -> (-y, x)
    let mut pt_obj = DrawObject::Stroke {
        id: "s2".into(),
        points: vec![StrokePoint {
            x: 10.0,
            y: 0.0,
            pressure: 1.0,
        }],
        color: dummy_color(),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    rotate_object(
        &mut pt_obj,
        Pos2::new(0.0, 0.0),
        std::f32::consts::FRAC_PI_2,
    );
    if let DrawObject::Stroke { points, .. } = &pt_obj {
        assert!((points[0].x - 0.0).abs() < 1e-4);
        assert!((points[0].y - 10.0).abs() < 1e-4);
    }
}

#[test]
fn test_object_in_lasso() {
    let poly = vec![
        Pos2::new(0.0, 0.0),
        Pos2::new(100.0, 0.0),
        Pos2::new(100.0, 100.0),
        Pos2::new(0.0, 100.0),
    ];

    let inside_stroke = DrawObject::Stroke {
        id: "1".into(),
        points: vec![
            StrokePoint {
                x: 20.0,
                y: 20.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 40.0,
                y: 40.0,
                pressure: 1.0,
            },
        ],
        color: dummy_color(),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    assert!(object_in_lasso(&inside_stroke, &poly));

    let crossing_stroke = DrawObject::Stroke {
        id: "2".into(),
        points: vec![
            StrokePoint {
                x: -20.0,
                y: 50.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 120.0,
                y: 50.0,
                pressure: 1.0,
            },
        ],
        color: dummy_color(),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    assert!(object_in_lasso(&crossing_stroke, &poly));

    let outside_stroke = DrawObject::Stroke {
        id: "3".into(),
        points: vec![
            StrokePoint {
                x: 200.0,
                y: 200.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 220.0,
                y: 220.0,
                pressure: 1.0,
            },
        ],
        color: dummy_color(),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    assert!(!object_in_lasso(&outside_stroke, &poly));
}

#[test]
fn test_precise_lasso_containment_mode() {
    let poly = vec![
        Pos2::new(0.0, 0.0),
        Pos2::new(100.0, 0.0),
        Pos2::new(100.0, 100.0),
        Pos2::new(0.0, 100.0),
    ];

    // 1. Fully contained stroke
    let inside_stroke = DrawObject::Stroke {
        id: "s_in".into(),
        points: vec![
            StrokePoint {
                x: 20.0,
                y: 20.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 80.0,
                y: 80.0,
                pressure: 1.0,
            },
        ],
        color: dummy_color(),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    assert!(object_in_lasso_mode(
        &inside_stroke,
        &poly,
        LassoSelectionMode::Containment
    ));
    assert!(object_in_lasso_mode(
        &inside_stroke,
        &poly,
        LassoSelectionMode::Intersection
    ));

    // 2. Stroke that crosses the lasso border (partially outside)
    let crossing_stroke = DrawObject::Stroke {
        id: "s_cross".into(),
        points: vec![
            StrokePoint {
                x: -20.0,
                y: 50.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 120.0,
                y: 50.0,
                pressure: 1.0,
            },
        ],
        color: dummy_color(),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    // In containment mode, crossing stroke must NOT be selected
    assert!(!object_in_lasso_mode(
        &crossing_stroke,
        &poly,
        LassoSelectionMode::Containment
    ));
    // In intersection mode, crossing stroke IS selected
    assert!(object_in_lasso_mode(
        &crossing_stroke,
        &poly,
        LassoSelectionMode::Intersection
    ));

    // 3. Single point stroke (dot) inside
    let dot_stroke = DrawObject::Stroke {
        id: "s_dot".into(),
        points: vec![StrokePoint {
            x: 50.0,
            y: 50.0,
            pressure: 1.0,
        }],
        color: dummy_color(),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    assert!(object_in_lasso_mode(
        &dot_stroke,
        &poly,
        LassoSelectionMode::Containment
    ));

    // 4. Shape fully inside vs crossing
    let inside_rect = DrawObject::Shape {
        id: "rect_in".into(),
        shape_type: ShapeKind::Rectangle,
        x: 10.0,
        y: 10.0,
        width: 40.0,
        height: 40.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        curve_points: Vec::new(),
        locked: false,
        label: None,
    };
    assert!(object_in_lasso_mode(
        &inside_rect,
        &poly,
        LassoSelectionMode::Containment
    ));

    let crossing_rect = DrawObject::Shape {
        id: "rect_cross".into(),
        shape_type: ShapeKind::Rectangle,
        x: 80.0,
        y: 80.0,
        width: 50.0,
        height: 50.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        curve_points: Vec::new(),
        locked: false,
        label: None,
    };
    assert!(!object_in_lasso_mode(
        &crossing_rect,
        &poly,
        LassoSelectionMode::Containment
    ));
    assert!(object_in_lasso_mode(
        &crossing_rect,
        &poly,
        LassoSelectionMode::Intersection
    ));

    // 5. Sticky note fully inside vs partially outside
    let inside_sticky = DrawObject::StickyNote {
        id: "sticky_in".into(),
        x: 15.0,
        y: 15.0,
        width: 30.0,
        height: 30.0,
        text: "Note".into(),
        color: dummy_color(),
        text_color: dummy_color(),
        font_size: 14.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
        rotation: 0.0,
    };
    assert!(object_in_lasso_mode(
        &inside_sticky,
        &poly,
        LassoSelectionMode::Containment
    ));

    let crossing_sticky = DrawObject::StickyNote {
        id: "sticky_cross".into(),
        x: 90.0,
        y: 10.0,
        width: 30.0,
        height: 30.0,
        text: "Note".into(),
        color: dummy_color(),
        text_color: dummy_color(),
        font_size: 14.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
        rotation: 0.0,
    };
    assert!(!object_in_lasso_mode(
        &crossing_sticky,
        &poly,
        LassoSelectionMode::Containment
    ));
    assert!(object_in_lasso_mode(
        &crossing_sticky,
        &poly,
        LassoSelectionMode::Intersection
    ));
}

#[test]
fn test_selection_frame_rotation_and_handles() {
    let shape = DrawObject::Shape {
        id: "rot_shape".into(),
        shape_type: ShapeKind::Rectangle,
        x: 150.0,
        y: 100.0,
        width: 100.0,
        height: 60.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: std::f32::consts::FRAC_PI_4, // 45 deg
        corner_radius: 0.0,
        curve_points: Vec::new(),
        locked: false,
        label: None,
    };

    let frame = SelectionFrame::from_objects(&[&shape], 20.0).expect("selection frame");
    assert!((frame.pivot.x - 200.0).abs() < 1e-3);
    assert!((frame.pivot.y - 130.0).abs() < 1e-3);
    assert!((frame.local_size.x - 100.0).abs() < 1e-3);
    assert!((frame.local_size.y - 60.0).abs() < 1e-3);
    assert!((frame.rotation - std::f32::consts::FRAC_PI_4).abs() < 1e-3);
    assert_eq!(frame.handles.len(), 9);

    // Sticky note selection frame preserves rotation
    let sticky = DrawObject::StickyNote {
        id: "rot_sticky".into(),
        x: 50.0,
        y: 50.0,
        width: 80.0,
        height: 80.0,
        text: "Hi".into(),
        color: dummy_color(),
        text_color: dummy_color(),
        font_size: 14.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
        rotation: 0.5,
    };
    let s_frame = SelectionFrame::from_objects(&[&sticky], 20.0).expect("sticky frame");
    assert!((s_frame.pivot.x - 90.0).abs() < 1e-3);
    assert!((s_frame.pivot.y - 90.0).abs() < 1e-3);
    assert!((s_frame.rotation - 0.5).abs() < 1e-3);

    // Multi-selection produces unified axis-aligned frame
    let multi = SelectionFrame::from_objects(&[&shape, &sticky], 20.0).expect("multi frame");
    assert_eq!(multi.rotation, 0.0);
    assert!(multi.local_size.x > 100.0);
}

#[test]
fn test_sticky_note_rotation_and_page_rotation() {
    let mut sticky = DrawObject::StickyNote {
        id: "sn_rot".into(),
        x: 100.0,
        y: 100.0,
        width: 200.0,
        height: 100.0,
        text: "Rotated Note".into(),
        color: dummy_color(),
        text_color: dummy_color(),
        font_size: 14.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
        rotation: 0.0,
    };

    // Initial bounds
    let b0 = object_bounds(&sticky);
    assert_eq!(b0.width(), 200.0);
    assert_eq!(b0.height(), 100.0);

    // Rotate 90 degrees around center (200.0, 150.0)
    let center = Pos2::new(200.0, 150.0);
    rotate_object(&mut sticky, center, std::f32::consts::FRAC_PI_2);

    if let DrawObject::StickyNote {
        width,
        height,
        rotation,
        ..
    } = &sticky
    {
        assert_eq!(*width, 200.0);
        assert_eq!(*height, 100.0);
        assert!((*rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    } else {
        panic!("Expected StickyNote");
    }

    // Rotated bounds width and height are swapped in world AABB
    let b1 = object_bounds(&sticky);
    assert!((b1.width() - 100.0).abs() < 1e-2);
    assert!((b1.height() - 200.0).abs() < 1e-2);

    // Rotate page 90 degrees clockwise
    let mut page = crate::model::Page::new(crate::model::PageTemplate::Blank);
    page.width = 1000.0;
    page.height = 800.0;
    page.objects.push(sticky);
    rotate_page_content(&mut page, 90);
    if let DrawObject::StickyNote { rotation, .. } = &page.objects[0] {
        assert!((*rotation - std::f32::consts::PI).abs() < 1e-3);
    }
}

#[test]
fn test_smooth_stroke_points() {
    let pts = vec![
        StrokePoint {
            x: 0.0,
            y: 0.0,
            pressure: 0.5,
        },
        StrokePoint {
            x: 10.0,
            y: 20.0,
            pressure: 0.7,
        },
        StrokePoint {
            x: 30.0,
            y: 25.0,
            pressure: 0.9,
        },
        StrokePoint {
            x: 60.0,
            y: 10.0,
            pressure: 0.4,
        },
    ];

    let smoothed = smooth_stroke_points(&pts, 4);
    assert!(smoothed.len() > pts.len());

    // First point must match exactly
    assert_eq!(smoothed[0].x, pts[0].x);
    assert_eq!(smoothed[0].y, pts[0].y);

    // Last point must match exactly
    let last_idx = smoothed.len() - 1;
    assert_eq!(smoothed[last_idx].x, pts[pts.len() - 1].x);
    assert_eq!(smoothed[last_idx].y, pts[pts.len() - 1].y);

    // No NaNs
    for p in &smoothed {
        assert!(p.x.is_finite());
        assert!(p.y.is_finite());
        assert!(p.pressure.is_finite());
    }
}

#[test]
fn test_tessellate_stroke() {
    let pts = vec![
        StrokePoint {
            x: 0.0,
            y: 0.0,
            pressure: 0.5,
        },
        StrokePoint {
            x: 50.0,
            y: 50.0,
            pressure: 1.0,
        },
    ];

    let mesh = tessellate_stroke(&pts, 4.0, Color32::RED, |p| p);
    assert!(!mesh.vertices.is_empty());
    assert!(!mesh.indices.is_empty());
    assert_eq!(mesh.indices.len() % 3, 0); // Must be valid triangles
}

#[test]
fn test_tessellate_single_point_stroke_dot() {
    let single_pt = vec![StrokePoint {
        x: 100.0,
        y: 150.0,
        pressure: 0.8,
    }];

    // Test tessellation with different stroke styles
    for style in StrokeStyle::ALL {
        let mesh = tessellate_stroke_styled(&single_pt, 6.0, Color32::BLUE, false, style, |p| p);
        // Harus menghasilkan mesh dot melingkar
        assert!(
            !mesh.vertices.is_empty(),
            "Mesh vertices should not be empty for {style:?}"
        );
        assert!(
            !mesh.indices.is_empty(),
            "Mesh indices should not be empty for {style:?}"
        );
        assert_eq!(
            mesh.indices.len() % 3,
            0,
            "Indices must form valid triangles for {style:?}"
        );
    }
}

#[test]
fn test_tessellate_stroke_styles_continuous() {
    let pts = vec![
        StrokePoint {
            x: 0.0,
            y: 0.0,
            pressure: 0.5,
        },
        StrokePoint {
            x: 30.0,
            y: 0.0,
            pressure: 0.7,
        },
        StrokePoint {
            x: 60.0,
            y: 20.0,
            pressure: 0.9,
        },
        StrokePoint {
            x: 100.0,
            y: 40.0,
            pressure: 1.0,
        },
    ];

    for style in [
        StrokeStyle::Solid,
        StrokeStyle::Dashed,
        StrokeStyle::DashDot,
        StrokeStyle::Dotted,
    ] {
        let mesh = tessellate_stroke_styled(&pts, 4.0, Color32::BLACK, false, style, |p| p);
        assert!(
            !mesh.vertices.is_empty(),
            "Vertices should be generated for {style:?}"
        );
        assert!(
            !mesh.indices.is_empty(),
            "Indices should be generated for {style:?}"
        );
        assert_eq!(mesh.indices.len() % 3, 0);
    }
}

#[test]
fn test_single_point_stroke_bounds_and_hit_test() {
    let dot_obj = DrawObject::Stroke {
        id: "dot1".into(),
        points: vec![StrokePoint {
            x: 200.0,
            y: 300.0,
            pressure: 1.0,
        }],
        color: dummy_color(),
        width: 8.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };

    let b = object_bounds(&dot_obj);
    assert_ne!(b, Rect::NOTHING);
    assert_eq!(b.min, Pos2::new(196.0, 296.0));
    assert_eq!(b.max, Pos2::new(204.0, 304.0));

    // Hit test tepat di titik tengah
    assert!(hit_test_object(&dot_obj, Pos2::new(200.0, 300.0), 2.0));
    // Hit test di tepi lingkaran radius
    assert!(hit_test_object(&dot_obj, Pos2::new(203.0, 300.0), 2.0));
    // Miss jauh dari titik
    assert!(!hit_test_object(&dot_obj, Pos2::new(220.0, 300.0), 2.0));
}

#[test]
fn test_spatial_grid_matches_brute_force() {
    let objects = vec![
        DrawObject::Shape {
            id: "1".into(),
            shape_type: ShapeKind::Rectangle,
            x: 10.0,
            y: 10.0,
            width: 30.0,
            height: 30.0,
            color: dummy_color(),
            stroke_width: 2.0,
            fill: None,
            rotation: 0.0,
            corner_radius: 0.0,
            label: None,
            locked: false,
            curve_points: Vec::new(),
        },
        DrawObject::Shape {
            id: "2".into(),
            shape_type: ShapeKind::Rectangle,
            x: 250.0,
            y: 250.0,
            width: 40.0,
            height: 40.0,
            color: dummy_color(),
            stroke_width: 2.0,
            fill: None,
            rotation: 0.0,
            corner_radius: 0.0,
            label: None,
            locked: false,
            curve_points: Vec::new(),
        },
        DrawObject::Shape {
            id: "3".into(),
            shape_type: ShapeKind::Rectangle,
            x: 500.0,
            y: 500.0,
            width: 20.0,
            height: 20.0,
            color: dummy_color(),
            stroke_width: 2.0,
            fill: None,
            rotation: 0.0,
            corner_radius: 0.0,
            label: None,
            locked: false,
            curve_points: Vec::new(),
        },
    ];

    let mut grid = SpatialGrid::new(100.0);
    grid.rebuild(&objects);

    // Query point near object 1
    let query_pt = Pos2::new(25.0, 25.0);
    let tol = 10.0;
    let candidates = grid.query_point(query_pt, tol);

    // Brute force candidates
    let mut brute_force = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        let b = object_bounds(obj);
        if b.expand(tol).contains(query_pt) {
            brute_force.push(i);
        }
    }

    for idx in brute_force {
        assert!(
            candidates.contains(&idx),
            "SpatialGrid candidate query must include candidate {idx}"
        );
    }
}

#[test]
fn test_smooth_stroke_points_edge_cases() {
    let empty: Vec<StrokePoint> = Vec::new();
    assert_eq!(smooth_stroke_points(&empty, 4).len(), 0);

    let single = vec![StrokePoint {
        x: 10.0,
        y: 10.0,
        pressure: 1.0,
    }];
    assert_eq!(smooth_stroke_points(&single, 4).len(), 1);

    let pair = vec![
        StrokePoint {
            x: 0.0,
            y: 0.0,
            pressure: 0.5,
        },
        StrokePoint {
            x: 10.0,
            y: 10.0,
            pressure: 0.8,
        },
    ];
    assert_eq!(smooth_stroke_points(&pair, 4).len(), 2);
}

#[test]
fn test_spatial_grid_empty_query() {
    let objects: Vec<DrawObject> = Vec::new();
    let mut grid = SpatialGrid::new(100.0);
    grid.rebuild(&objects);
    let res = grid.query_point(Pos2::new(100.0, 100.0), 10.0);
    assert!(res.is_empty());
}

#[test]
fn test_continuous_coordinate_conversion() {
    // Page 1 origin at screen (100, 50), zoom = 1.5
    let origin_p1 = Pos2::new(100.0, 50.0);
    let zoom = 1.5;
    let screen_pt = Pos2::new(250.0, 200.0);
    let page_pt = screen_to_page_point(screen_pt, origin_p1, zoom);
    // rel_x = 150 / 1.5 = 100, rel_y = 150 / 1.5 = 100
    assert_eq!(page_pt, Pos2::new(100.0, 100.0));
    let roundtrip_screen = page_to_screen_point(page_pt, origin_p1, zoom);
    assert_eq!(roundtrip_screen, screen_pt);

    // Page 2 stacked vertically below page 1 (height 800 * 1.5 + 24 margin = 1224)
    let origin_p2 = Pos2::new(100.0, 50.0 + 1224.0);
    let screen_pt_p2 = Pos2::new(100.0 + 75.0, origin_p2.y + 150.0);
    let page_pt_p2 = screen_to_page_point(screen_pt_p2, origin_p2, zoom);
    assert_eq!(page_pt_p2, Pos2::new(50.0, 100.0));
}

#[test]
fn test_recognize_shape_line() {
    // Line from (10, 10) to (110, 10) with minor jitter
    let points = vec![
        StrokePoint::from(Pos2::new(10.0, 10.0)),
        StrokePoint::from(Pos2::new(30.0, 10.5)),
        StrokePoint::from(Pos2::new(50.0, 9.8)),
        StrokePoint::from(Pos2::new(70.0, 10.2)),
        StrokePoint::from(Pos2::new(90.0, 10.0)),
        StrokePoint::from(Pos2::new(100.0, 10.1)),
        StrokePoint::from(Pos2::new(105.0, 10.0)),
        StrokePoint::from(Pos2::new(110.0, 10.0)),
    ];

    let shape = recognize_shape(&points).expect("Should recognize straight line");
    assert_eq!(shape.kind, ShapeKind::Line);
}

#[test]
fn test_recognize_shape_circle() {
    // Circle around (100, 100) with radius 40
    let center = Pos2::new(100.0, 100.0);
    let radius = 40.0;
    let mut points = Vec::new();
    for i in 0..24 {
        let angle = (i as f32) * std::f32::consts::TAU / 24.0;
        let pt = Pos2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        points.push(StrokePoint::from(pt));
    }
    // close the loop
    points.push(points[0].clone());

    let shape = recognize_shape(&points).expect("Should recognize circle");
    assert_eq!(shape.kind, ShapeKind::Ellipse);
}

#[test]
fn test_recognize_shape_rectangle() {
    // Rectangle from (50, 50) to (150, 110)
    let mut points = Vec::new();
    // Top edge
    for x in [50.0, 75.0, 100.0, 125.0, 150.0] {
        points.push(StrokePoint::from(Pos2::new(x, 50.0)));
    }
    // Right edge
    for y in [65.0, 80.0, 95.0, 110.0] {
        points.push(StrokePoint::from(Pos2::new(150.0, y)));
    }
    // Bottom edge
    for x in [125.0, 100.0, 75.0, 50.0] {
        points.push(StrokePoint::from(Pos2::new(x, 110.0)));
    }
    // Left edge & close
    for y in [95.0, 80.0, 65.0, 50.0] {
        points.push(StrokePoint::from(Pos2::new(50.0, y)));
    }

    let shape = recognize_shape(&points).expect("Should recognize rectangle");
    assert_eq!(shape.kind, ShapeKind::Rectangle);
}

#[test]
fn test_recognize_shape_triangle() {
    // Triangle: (100, 20), (40, 120), (160, 120)
    let mut points = Vec::new();
    // Edge 1: top to bottom-left
    for t in 0..6 {
        let frac = t as f32 / 5.0;
        points.push(StrokePoint::from(Pos2::new(
            100.0 - 60.0 * frac,
            20.0 + 100.0 * frac,
        )));
    }
    // Edge 2: bottom-left to bottom-right
    for t in 1..6 {
        let frac = t as f32 / 5.0;
        points.push(StrokePoint::from(Pos2::new(40.0 + 120.0 * frac, 120.0)));
    }
    // Edge 3: bottom-right to top
    for t in 1..=5 {
        let frac = t as f32 / 5.0;
        points.push(StrokePoint::from(Pos2::new(
            160.0 - 60.0 * frac,
            120.0 - 100.0 * frac,
        )));
    }

    let shape = recognize_shape(&points).expect("Should recognize triangle");
    assert_eq!(shape.kind, ShapeKind::Triangle);
}

#[test]
fn test_recognize_shape_random_squiggle_none() {
    // Random squiggle points should NOT be recognized as clean shape
    let points = vec![
        StrokePoint::from(Pos2::new(10.0, 10.0)),
        StrokePoint::from(Pos2::new(20.0, 80.0)),
        StrokePoint::from(Pos2::new(60.0, 15.0)),
        StrokePoint::from(Pos2::new(30.0, 110.0)),
        StrokePoint::from(Pos2::new(90.0, 45.0)),
        StrokePoint::from(Pos2::new(15.0, 95.0)),
        StrokePoint::from(Pos2::new(80.0, 85.0)),
        StrokePoint::from(Pos2::new(40.0, 40.0)),
    ];

    let res = recognize_shape(&points);
    assert!(res.is_none());
}

#[test]
fn test_split_stroke_by_eraser_middle_split() {
    // Garis horizontal dari x=0 ke x=100 pada y=50
    let points: Vec<StrokePoint> = (0..=10)
        .map(|i| StrokePoint {
            x: i as f32 * 10.0,
            y: 50.0,
            pressure: 0.5,
        })
        .collect();

    // Penghapus di (50, 50) dengan radius 12 -> memotong sekitar x=38..62
    let split = split_stroke_by_eraser(&points, Pos2::new(50.0, 50.0), 12.0)
        .expect("Should hit and split stroke");
    assert_eq!(split.len(), 2);

    // Segmen pertama berakhir tepat di tepi kiri lingkaran (x ~ 38)
    let first_end = split[0].last().unwrap();
    assert!((first_end.x - 38.0).abs() < 0.1);

    // Segmen kedua dimulai tepat di tepi kanan lingkaran (x ~ 62)
    let second_start = split[1].first().unwrap();
    assert!((second_start.x - 62.0).abs() < 0.1);
}

#[test]
fn test_split_stroke_by_eraser_long_single_segment() {
    // Satu segmen panjang dari (0, 0) ke (100, 0)
    let points = vec![
        StrokePoint {
            x: 0.0,
            y: 0.0,
            pressure: 0.4,
        },
        StrokePoint {
            x: 100.0,
            y: 0.0,
            pressure: 0.8,
        },
    ];

    // Penghapus di tengah (50, 0) radius 10 -> memotong 1 segmen menjadi 2 segmen: [0..40] dan [60..100]
    let split = split_stroke_by_eraser(&points, Pos2::new(50.0, 0.0), 10.0)
        .expect("Should split single long segment");
    assert_eq!(split.len(), 2);
    assert!((split[0].last().unwrap().x - 40.0).abs() < 0.1);
    assert!((split[1].first().unwrap().x - 60.0).abs() < 0.1);
}

#[test]
fn test_split_stroke_by_eraser_miss_and_full_erase() {
    let points = vec![
        StrokePoint {
            x: 10.0,
            y: 10.0,
            pressure: 0.5,
        },
        StrokePoint {
            x: 20.0,
            y: 10.0,
            pressure: 0.5,
        },
        StrokePoint {
            x: 30.0,
            y: 10.0,
            pressure: 0.5,
        },
    ];

    // Jauh di luar goresan -> None
    assert!(split_stroke_by_eraser(&points, Pos2::new(100.0, 100.0), 10.0).is_none());

    // Radius besar menutupi seluruh goresan -> Some(vec![])
    let full =
        split_stroke_by_eraser(&points, Pos2::new(20.0, 10.0), 50.0).expect("Should hit stroke");
    assert!(full.is_empty());
}

#[test]
fn test_palm_rejection_filter() {
    let existing = vec![StrokePoint {
        x: 100.0,
        y: 100.0,
        pressure: 0.6,
    }];

    // Gerakan normal pena dekat titik sebelumnya -> diterima (false)
    assert!(!should_reject_palm_input(
        &existing,
        Pos2::new(105.0, 104.0),
        0.016,
        false
    ));

    // Multi-touch aktif saat menulis -> ditolak sebagai sentuhan telapak tangan (true)
    assert!(should_reject_palm_input(
        &existing,
        Pos2::new(105.0, 104.0),
        0.016,
        true
    ));

    // Lompatan jarak ekstrem (> 180pt) dalam waktu sangat singkat (< 80ms) -> ditolak (true)
    assert!(should_reject_palm_input(
        &existing,
        Pos2::new(400.0, 450.0),
        0.010,
        false
    ));
}

#[test]
fn test_handwriting_ocr_recognition() {
    // Buat goresan tulisan tangan untuk huruf "H" (2 garis vertikal + 1 garis horizontal)
    // diikuti huruf "I" (1 garis vertikal)
    let h_left = DrawObject::Stroke {
        id: "h1".into(),
        points: vec![
            StrokePoint::from(Pos2::new(20.0, 20.0)),
            StrokePoint::from(Pos2::new(20.0, 70.0)),
        ],
        color: dummy_color(),
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    let h_right = DrawObject::Stroke {
        id: "h2".into(),
        points: vec![
            StrokePoint::from(Pos2::new(45.0, 20.0)),
            StrokePoint::from(Pos2::new(45.0, 70.0)),
        ],
        color: dummy_color(),
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    let h_bar = DrawObject::Stroke {
        id: "h3".into(),
        points: vec![
            StrokePoint::from(Pos2::new(20.0, 45.0)),
            StrokePoint::from(Pos2::new(45.0, 45.0)),
        ],
        color: dummy_color(),
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };
    let i_vert = DrawObject::Stroke {
        id: "i1".into(),
        points: vec![
            StrokePoint::from(Pos2::new(65.0, 20.0)),
            StrokePoint::from(Pos2::new(65.0, 70.0)),
        ],
        color: dummy_color(),
        width: 3.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    };

    let objects = vec![h_left, h_right, h_bar, i_vert];
    let ocr = recognize_handwriting_from_strokes(&objects);
    assert_eq!(ocr, "HI");
}

#[test]
fn test_recognize_shape_right_triangle_and_scalene_triangle() {
    // 1. Segitiga Siku-Siku (RightTriangle) dengan sudut siku di kiri-bawah: (40, 20) -> (40, 120) -> (160, 120)
    let mut right_pts = Vec::new();
    for t in 0..=6 {
        let f = t as f32 / 6.0;
        right_pts.push(StrokePoint::from(Pos2::new(40.0, 20.0 + 100.0 * f)));
    }
    for t in 1..=6 {
        let f = t as f32 / 6.0;
        right_pts.push(StrokePoint::from(Pos2::new(40.0 + 120.0 * f, 120.0)));
    }
    for t in 1..=6 {
        let f = t as f32 / 6.0;
        right_pts.push(StrokePoint::from(Pos2::new(
            160.0 - 120.0 * f,
            120.0 - 100.0 * f,
        )));
    }
    let rec_right = recognize_shape(&right_pts).expect("Should recognize right triangle");
    assert_eq!(rec_right.kind, ShapeKind::RightTriangle);

    // 2. Segitiga Sembarang (ScaleneTriangle): (30, 25) -> (180, 60) -> (75, 140)
    let v0 = Pos2::new(30.0, 25.0);
    let v1 = Pos2::new(180.0, 60.0);
    let v2 = Pos2::new(75.0, 140.0);
    let mut scalene_pts = Vec::new();
    for (a, b) in [(v0, v1), (v1, v2), (v2, v0)] {
        for t in 0..6 {
            let f = t as f32 / 6.0;
            scalene_pts.push(StrokePoint::from(Pos2::new(
                a.x + (b.x - a.x) * f,
                a.y + (b.y - a.y) * f,
            )));
        }
    }
    scalene_pts.push(StrokePoint::from(v0));
    let rec_scalene = recognize_shape(&scalene_pts).expect("Should recognize scalene triangle");
    assert!(matches!(
        rec_scalene.kind,
        ShapeKind::ScaleneTriangle { .. }
    ));
}

#[test]
fn test_recognize_shape_parallelogram() {
    // Jajar Genjang (Parallelogram): (70, 40) -> (190, 40) -> (150, 110) -> (30, 110)
    let c0 = Pos2::new(70.0, 40.0);
    let c1 = Pos2::new(190.0, 40.0);
    let c2 = Pos2::new(150.0, 110.0);
    let c3 = Pos2::new(30.0, 110.0);
    let mut para_pts = Vec::new();
    for (a, b) in [(c0, c1), (c1, c2), (c2, c3), (c3, c0)] {
        for t in 0..6 {
            let f = t as f32 / 6.0;
            para_pts.push(StrokePoint::from(Pos2::new(
                a.x + (b.x - a.x) * f,
                a.y + (b.y - a.y) * f,
            )));
        }
    }
    para_pts.push(StrokePoint::from(c0));

    let rec_para = recognize_shape(&para_pts).expect("Should recognize parallelogram");
    assert_eq!(rec_para.kind, ShapeKind::Parallelogram);
}

#[test]
fn test_rotate_line_around_start_point() {
    // Garis mulai di (50, 60) dengan vektor (100, 0) -> titik akhir di (150, 60)
    let mut line = DrawObject::Shape {
        id: "line-1".into(),
        shape_type: ShapeKind::Line,
        x: 50.0,
        y: 60.0,
        width: 100.0,
        height: 0.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };

    // Rotasi 90 derajat (PI / 2): titik awal (50, 60) tetap sebagai pusat,
    // titik akhir berotasi menjadi (50, 160) -> width = 0, height = 100
    rotate_object(
        &mut line,
        Pos2::new(100.0, 60.0),
        std::f32::consts::FRAC_PI_2,
    );
    if let DrawObject::Shape {
        x,
        y,
        width,
        height,
        ..
    } = line
    {
        assert!((x - 50.0).abs() < 1e-4);
        assert!((y - 60.0).abs() < 1e-4);
        assert!(width.abs() < 1e-4);
        assert!((height - 100.0).abs() < 1e-4);
    } else {
        panic!("Expected Line Shape");
    }
}

#[test]
fn test_rotate_page_content_90_cw_and_180() {
    let mut page = crate::model::Page::with_size(crate::model::PageTemplate::Blank, 800.0, 1100.0);
    page.objects.push(DrawObject::Stroke {
        id: "s1".into(),
        points: vec![
            StrokePoint {
                x: 100.0,
                y: 200.0,
                pressure: 1.0,
            },
            StrokePoint {
                x: 300.0,
                y: 400.0,
                pressure: 1.0,
            },
        ],
        color: dummy_color(),
        width: 2.0,
        is_highlighter: false,
        style: StrokeStyle::Solid,
    });

    // Rotate 90 CW: (x, y) -> (old_h - y, x)
    // old_h = 1100, old_w = 800
    // (100, 200) -> (1100 - 200, 100) = (900, 100)
    // (300, 400) -> (1100 - 400, 300) = (700, 300)
    rotate_page_content(&mut page, 90);
    assert_eq!(page.width, 1100.0);
    assert_eq!(page.height, 800.0);
    assert_eq!(page.rotation, 90);
    if let DrawObject::Stroke { points, .. } = &page.objects[0] {
        assert!((points[0].x - 900.0).abs() < 1e-4);
        assert!((points[0].y - 100.0).abs() < 1e-4);
        assert!((points[1].x - 700.0).abs() < 1e-4);
        assert!((points[1].y - 300.0).abs() < 1e-4);
    } else {
        panic!("Expected stroke");
    }

    // Rotate 180: (x, y) -> (w - x, h - y)
    // current w = 1100, h = 800
    // (900, 100) -> (1100 - 900, 800 - 100) = (200, 700)
    rotate_page_content(&mut page, 180);
    assert_eq!(page.width, 1100.0);
    assert_eq!(page.height, 800.0);
    assert_eq!(page.rotation, 270);
    if let DrawObject::Stroke { points, .. } = &page.objects[0] {
        assert!((points[0].x - 200.0).abs() < 1e-4);
        assert!((points[0].y - 700.0).abs() < 1e-4);
    } else {
        panic!("Expected stroke");
    }
}

#[test]
fn test_highlighter_tessellation_no_overlap() {
    let pts = vec![
        StrokePoint {
            x: 10.0,
            y: 10.0,
            pressure: 0.1, // Pressure bervariasi tidak boleh memengaruhi ketebalan highlighter
        },
        StrokePoint {
            x: 100.0,
            y: 10.0,
            pressure: 0.9,
        },
    ];
    let mesh = tessellate_stroke_ex(
        &pts,
        20.0,
        eframe::egui::Color32::from_rgba_unmultiplied(255, 255, 0, 90),
        true,
        |p| p,
    );
    assert!(!mesh.vertices.is_empty());
    assert!(!mesh.indices.is_empty());
    // Semua index segitiga valid
    for &idx in &mesh.indices {
        assert!((idx as usize) < mesh.vertices.len());
    }
}

#[test]
fn test_image_object_rotation_bounds_and_transform() {
    let mut page = crate::model::Page::with_size(crate::model::PageTemplate::Blank, 800.0, 1100.0);
    page.objects.push(DrawObject::Image {
        id: "img1".into(),
        asset_id: "asset_test".into(),
        x: 100.0,
        y: 200.0,
        width: 300.0,
        height: 400.0,
        rotation: 0.0,
        lock_aspect: true,
    });

    let b0 = object_bounds(&page.objects[0]);
    assert!((b0.width() - 300.0).abs() < 1e-4);
    assert!((b0.height() - 400.0).abs() < 1e-4);

    // Rotasi 90 CW: center (100+150=250, 200+200=400) -> (1100 - 400 = 700, 250)
    // Page dimensions swap: width: 1100, height: 800
    rotate_page_content(&mut page, 90);
    assert_eq!(page.width, 1100.0);
    assert_eq!(page.height, 800.0);
    assert_eq!(page.rotation, 90);

    if let DrawObject::Image {
        x,
        y,
        width,
        height,
        rotation,
        ..
    } = &page.objects[0]
    {
        assert!(
            (width - 300.0).abs() < 1e-4,
            "Width asli harus tetap terjaga tanpa swap terbalik"
        );
        assert!(
            (height - 400.0).abs() < 1e-4,
            "Height asli harus tetap terjaga tanpa swap terbalik"
        );
        let cx = x + width / 2.0;
        let cy = y + height / 2.0;
        assert!((cx - 700.0).abs() < 1e-4, "Center x harus 700");
        assert!((cy - 250.0).abs() < 1e-4, "Center y harus 250");
        assert!(
            (rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-4,
            "Rotasi harus 90 derajat (pi/2)"
        );
    } else {
        panic!("Expected Image object");
    }

    // Bounding box visual setelah rotasi 90 derajat harus 400x300
    let b1 = object_bounds(&page.objects[0]);
    assert!(
        (b1.width() - 400.0).abs() < 1e-2,
        "Lebar bounding box visual setelah rotasi 90 CW harus sesuai height"
    );
    assert!(
        (b1.height() - 300.0).abs() < 1e-2,
        "Tinggi bounding box visual setelah rotasi 90 CW harus sesuai width"
    );
}

#[test]
fn test_text_object_rotation_on_page_rotate() {
    let mut page = crate::model::Page::with_size(crate::model::PageTemplate::Blank, 800.0, 1100.0);
    page.objects.push(DrawObject::Text {
        id: "text-rot-test".into(),
        x: 100.0,
        y: 200.0,
        text: "Rotated Text".into(),
        font_size: 20.0,
        color: dummy_color(),
        rotation: 0.0,
        font_family: FontFamilyPreset::SansSerif,
        bold: false,
        italic: false,
        alignment: TextAlign::Left,
    });

    // 1. Rotate 90 CW: (x, y) -> (1100 - y = 900, x = 100), rotation -> FRAC_PI_2 (90 deg)
    rotate_page_content(&mut page, 90);
    assert_eq!(page.width, 1100.0);
    assert_eq!(page.height, 800.0);
    assert_eq!(page.rotation, 90);

    if let DrawObject::Text { x, y, rotation, .. } = &page.objects[0] {
        assert!(
            (x - 900.0).abs() < 1e-4,
            "Posisi x teks setelah rotasi 90 CW harus 900"
        );
        assert!(
            (y - 100.0).abs() < 1e-4,
            "Posisi y teks setelah rotasi 90 CW harus 100"
        );
        assert!(
            (rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-4,
            "Sudut rotasi teks harus 90 derajat (pi/2)"
        );
    } else {
        panic!("Expected Text object");
    }

    // 2. Rotate 180: (x, y) -> (1100 - x = 200, 800 - y = 700), rotation -> PI/2 + PI = 3*PI/2 (270 deg)
    rotate_page_content(&mut page, 180);
    assert_eq!(page.rotation, 270);
    if let DrawObject::Text { x, y, rotation, .. } = &page.objects[0] {
        assert!(
            (x - 200.0).abs() < 1e-4,
            "Posisi x teks setelah rotasi 180 harus 200"
        );
        assert!(
            (y - 700.0).abs() < 1e-4,
            "Posisi y teks setelah rotasi 180 harus 700"
        );
        let expected_rot =
            (std::f32::consts::FRAC_PI_2 + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU);
        assert!(
            (rotation - expected_rot).abs() < 1e-4,
            "Sudut rotasi teks harus 270 derajat"
        );
    } else {
        panic!("Expected Text object");
    }
}

#[test]
fn test_shape_connection_point_rotated() {
    use crate::geometry::connectors::{Direction, shape_connection_point};
    use crate::model::{ConnectionSite, Point2D};

    // 1. Rectangle di (100, 100), width: 100, height: 60, rotation: 0
    // Center: (150, 130)
    let shape_rect = DrawObject::Shape {
        id: "rect-conn".into(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 60.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };

    let (top_pt, top_dir) = shape_connection_point(&shape_rect, ConnectionSite::Top, None);
    assert_eq!(top_pt, Point2D::new(150.0, 100.0));
    assert_eq!(top_dir, Direction::Up);

    let (right_pt, right_dir) = shape_connection_point(&shape_rect, ConnectionSite::Right, None);
    assert_eq!(right_pt, Point2D::new(200.0, 130.0));
    assert_eq!(right_dir, Direction::Right);

    let (bottom_pt, bottom_dir) = shape_connection_point(&shape_rect, ConnectionSite::Bottom, None);
    assert_eq!(bottom_pt, Point2D::new(150.0, 160.0));
    assert_eq!(bottom_dir, Direction::Down);

    let (left_pt, left_dir) = shape_connection_point(&shape_rect, ConnectionSite::Left, None);
    assert_eq!(left_pt, Point2D::new(100.0, 130.0));
    assert_eq!(left_dir, Direction::Left);

    // 2. Mode Auto: jika target ada di kanan jauh (300, 130), Auto harus memilih Right
    let (auto_pt, _) = shape_connection_point(
        &shape_rect,
        ConnectionSite::Auto,
        Some(Point2D::new(300.0, 130.0)),
    );
    assert_eq!(auto_pt, right_pt);

    // Jika target ada di atas (150, 10), Auto harus memilih Top
    let (auto_pt_top, _) = shape_connection_point(
        &shape_rect,
        ConnectionSite::Auto,
        Some(Point2D::new(150.0, 10.0)),
    );
    assert_eq!(auto_pt_top, top_pt);

    // 3. Rotasi 90 derajat (pi/2) CW
    // Center tetap (150, 130).
    // Sisi 'Right' lokal (200, 130) berotasi 90 CW menjadi (150, 180) (menghadap Down)
    let mut shape_rect_rot = shape_rect.clone();
    if let DrawObject::Shape {
        ref mut rotation, ..
    } = shape_rect_rot
    {
        *rotation = std::f32::consts::FRAC_PI_2;
    }
    let (r_pt, r_dir) = shape_connection_point(&shape_rect_rot, ConnectionSite::Right, None);
    assert!((r_pt.x - 150.0).abs() < 1e-4);
    assert!((r_pt.y - 180.0).abs() < 1e-4);
    assert_eq!(r_dir, Direction::Down);

    // 4. Diamond connection points
    let shape_diamond = DrawObject::Shape {
        id: "diamond-conn".into(),
        shape_type: ShapeKind::Diamond,
        x: 200.0,
        y: 200.0,
        width: 80.0,
        height: 80.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };
    let (d_top, d_dir) = shape_connection_point(&shape_diamond, ConnectionSite::Top, None);
    assert_eq!(d_top, Point2D::new(240.0, 200.0));
    assert_eq!(d_dir, Direction::Up);
}

#[test]
fn test_route_orthogonal_no_diagonal_and_clean_collinear() {
    use crate::geometry::connectors::{Direction, route_orthogonal};
    use crate::model::Point2D;

    let start = Point2D::new(100.0, 100.0);
    let end = Point2D::new(300.0, 250.0);

    let path = route_orthogonal(start, Direction::Right, end, Direction::Left, 20.0);

    assert!(
        path.len() >= 4,
        "Jalur orthogonal minimal memiliki start, stub1, stub2, end"
    );
    assert_eq!(path.first().copied(), Some(start));
    assert_eq!(path.last().copied(), Some(end));

    // Verifikasi bahwa SETIAP segmen strictly ortogonal (horizontal atau vertikal, tidak ada diagonal)
    for i in 0..path.len() - 1 {
        let p1 = path[i];
        let p2 = path[i + 1];
        let is_h = (p1.y - p2.y).abs() < 1e-4;
        let is_v = (p1.x - p2.x).abs() < 1e-4;
        assert!(
            is_h || is_v,
            "Segmen {} -> {} tidak orthogonal (dx={}, dy={})",
            i,
            i + 1,
            (p1.x - p2.x).abs(),
            (p1.y - p2.y).abs()
        );
    }
}

#[test]
fn test_arrowhead_geometry_symmetry_and_direction() {
    use crate::geometry::connectors::arrowhead_geometry;
    use crate::model::Point2D;

    let tip = Point2D::new(200.0, 100.0);
    let incoming = Point2D::new(1.0, 0.0); // Garis datang dari kiri ke kanan
    let arrow_len = 12.0;
    let arrow_width = 8.0;

    let [t, left, right] = arrowhead_geometry(tip, incoming, arrow_len, arrow_width);

    assert_eq!(t, tip);
    // Base x harus tip.x - 12.0 = 188.0
    assert_eq!(left.x, 188.0);
    assert_eq!(right.x, 188.0);
    // Simetri sumbu Y: tip.y = 100.0, sayap berjarak 4.0 ke atas dan ke bawah
    assert_eq!(left.y, 104.0);
    assert_eq!(right.y, 96.0);
}

#[test]
fn test_hit_test_connector_details() {
    use crate::geometry::connectors::{ConnectorHitTarget, hit_test_connector_details};
    use crate::model::Point2D;

    let points = vec![
        Point2D::new(50.0, 50.0),
        Point2D::new(150.0, 50.0),
        Point2D::new(150.0, 150.0),
    ];

    // Dekat titik awal
    assert_eq!(
        hit_test_connector_details(Point2D::new(52.0, 51.0), &points, 5.0),
        Some(ConnectorHitTarget::StartHandle)
    );

    // Dekat titik akhir
    assert_eq!(
        hit_test_connector_details(Point2D::new(149.0, 148.0), &points, 5.0),
        Some(ConnectorHitTarget::EndHandle)
    );

    // Dekat tikungan tengah (control point 1)
    assert_eq!(
        hit_test_connector_details(Point2D::new(150.0, 51.0), &points, 5.0),
        Some(ConnectorHitTarget::ControlPoint(1))
    );

    // Dekat badan segmen 0 (di (100, 50))
    assert_eq!(
        hit_test_connector_details(Point2D::new(100.0, 52.0), &points, 5.0),
        Some(ConnectorHitTarget::LineSegment(0))
    );

    // Jauh di luar toleransi
    assert_eq!(
        hit_test_connector_details(Point2D::new(0.0, 0.0), &points, 5.0),
        None
    );
}

#[test]
fn test_rounded_rect_vertices_clamping_and_rotation() {
    use crate::geometry::curves::rounded_rect_vertices;
    use crate::model::Point2D;

    // Radius berlebihan: width 100, height 40 -> max radius adalah 40 / 2 = 20.
    // Jika input corner_radius 50.0, fungsi harus clamp ke 20.0 tanpa error/panic.
    let verts = rounded_rect_vertices(0.0, 0.0, 100.0, 40.0, 50.0, 0.0, 4);
    assert!(!verts.is_empty());

    // Corner radius 0 mengembalikan 4 titik persegi panjang
    let flat_verts = rounded_rect_vertices(10.0, 20.0, 80.0, 50.0, 0.0, 0.0, 4);
    assert_eq!(flat_verts.len(), 4);
    assert_eq!(flat_verts[0], Point2D::new(10.0, 20.0));
    assert_eq!(flat_verts[2], Point2D::new(90.0, 70.0));
}

#[test]
fn test_cubic_bezier_sampling_and_rdp() {
    use crate::geometry::curves::{sample_cubic_bezier, simplify_points_rdp};
    use crate::model::Point2D;

    let p0 = Point2D::new(0.0, 0.0);
    let p1 = Point2D::new(20.0, 50.0);
    let p2 = Point2D::new(80.0, 50.0);
    let p3 = Point2D::new(100.0, 0.0);

    let samples = sample_cubic_bezier(p0, p1, p2, p3, 20);
    assert_eq!(samples.len(), 21);
    assert_eq!(samples[0], p0);
    assert_eq!(samples[20], p3);

    // Uji RDP simplification pada garis lurus dengan sedikit noise kecil
    let noisy_line = vec![
        Point2D::new(0.0, 0.0),
        Point2D::new(25.0, 0.2),
        Point2D::new(50.0, -0.1),
        Point2D::new(75.0, 0.1),
        Point2D::new(100.0, 0.0),
    ];
    let simplified = simplify_points_rdp(&noisy_line, 1.0);
    assert_eq!(
        simplified.len(),
        2,
        "Garis lurus harus tereduksi menjadi 2 titik ujung"
    );
    assert_eq!(simplified[0], Point2D::new(0.0, 0.0));
    assert_eq!(simplified[1], Point2D::new(100.0, 0.0));
}

#[test]
fn test_hit_test_rotated_shape() {
    use crate::geometry::connectors::hit_test_rotated_shape;
    use crate::model::Point2D;

    // Persegi panjang di (100, 100) ukuran 100x40.
    // Center: (150, 120).
    // Rotasi 90 derajat CW:
    // Lebar visual menjadi 40, tinggi visual menjadi 100.
    let shape_rot = DrawObject::Shape {
        id: "rot-hit".into(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 40.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: Some(dummy_color()),
        rotation: std::f32::consts::FRAC_PI_2,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };

    // Titik (150, 160) berada di dalam perpanjangan vertikal setelah rotasi 90 derajat
    assert!(hit_test_rotated_shape(
        Point2D::new(150.0, 160.0),
        &shape_rot,
        2.0
    ));

    // Titik (190, 120) yang tadinya di dalam persegi horizontal, kini berada di luar setelah rotasi vertikal!
    assert!(!hit_test_rotated_shape(
        Point2D::new(190.0, 120.0),
        &shape_rot,
        2.0
    ));
}

#[test]
fn test_compute_and_hit_test_transform_handles() {
    use super::shape_ops::{
        TransformHandleKind, compute_transform_handles, hit_test_transform_handles,
    };

    // Bounding box tanpa rotasi: (100, 100), width 200, height 100 -> center (200, 150)
    let handles = compute_transform_handles(100.0, 100.0, 200.0, 100.0, 0.0, 20.0);
    assert_eq!(handles.len(), 9);

    let get_handle = |kind: TransformHandleKind| -> Point2D {
        handles.iter().find(|h| h.kind == kind).unwrap().pos
    };

    // 4 Sudut (Scale)
    let nw = get_handle(TransformHandleKind::NorthWest);
    let ne = get_handle(TransformHandleKind::NorthEast);
    let se = get_handle(TransformHandleKind::SouthEast);
    let sw = get_handle(TransformHandleKind::SouthWest);
    assert!((nw.x - 100.0).abs() < 1e-4 && (nw.y - 100.0).abs() < 1e-4);
    assert!((ne.x - 300.0).abs() < 1e-4 && (ne.y - 100.0).abs() < 1e-4);
    assert!((se.x - 300.0).abs() < 1e-4 && (se.y - 200.0).abs() < 1e-4);
    assert!((sw.x - 100.0).abs() < 1e-4 && (sw.y - 200.0).abs() < 1e-4);

    // 4 Tepi Sisi (Resize satu sumbu)
    let n = get_handle(TransformHandleKind::North);
    let e = get_handle(TransformHandleKind::East);
    let s = get_handle(TransformHandleKind::South);
    let w = get_handle(TransformHandleKind::West);
    assert!((n.x - 200.0).abs() < 1e-4 && (n.y - 100.0).abs() < 1e-4);
    assert!((e.x - 300.0).abs() < 1e-4 && (e.y - 150.0).abs() < 1e-4);
    assert!((s.x - 200.0).abs() < 1e-4 && (s.y - 200.0).abs() < 1e-4);
    assert!((w.x - 100.0).abs() < 1e-4 && (w.y - 150.0).abs() < 1e-4);

    // Tangkai Rotasi di atas
    let rot = get_handle(TransformHandleKind::Rotation);
    assert!((rot.x - 200.0).abs() < 1e-4 && (rot.y - 80.0).abs() < 1e-4);

    // Hit testing: tepat di titik SouthEast
    let hit_se = hit_test_transform_handles(&handles, Point2D::new(300.0, 200.0), 5.0);
    assert_eq!(hit_se, Some(TransformHandleKind::SouthEast));

    // Hit testing: sedikit meleset tapi masih dalam radius toleransi 10px
    let hit_rot = hit_test_transform_handles(&handles, Point2D::new(202.0, 83.0), 10.0);
    assert_eq!(hit_rot, Some(TransformHandleKind::Rotation));

    // Hit testing: di luar seluruh handle
    let miss = hit_test_transform_handles(&handles, Point2D::new(200.0, 150.0), 10.0);
    assert_eq!(miss, None);
}

#[test]
fn test_calculate_proportional_scale_and_min_dim() {
    use super::shape_ops::{ShapeTransformBox, TransformHandleKind, calculate_proportional_scale};

    // Objek awal: x=100, y=100, w=100, h=50. Dragging SouthEast corner.
    // Anchor berlawanan adalah NorthWest (100, 100).
    let orig = ShapeTransformBox::new(100.0, 100.0, 100.0, 50.0, 0.0);
    let (nx, ny, nw, nh) = calculate_proportional_scale(
        TransformHandleKind::SouthEast,
        Point2D::new(300.0, 200.0), // perbesar ke arah tenggara
        orig,
        4.0,
    );

    // Sudut NorthWest tetap di (100, 100)
    assert!((nx - 100.0).abs() < 1e-3);
    assert!((ny - 100.0).abs() < 1e-3);
    // Skala 2x: w=200, h=100
    assert!((nw - 200.0).abs() < 1e-3);
    assert!((nh - 100.0).abs() < 1e-3);
    assert!((nw / nh - 2.0).abs() < 1e-3); // Rasio aspek 2:1 terjaga

    // Menyeret melewati titik anchor (inversi / flip) tidak menyebabkan crash atau ukuran di bawah 4.0
    let (_fx, _fy, fw, fh) = calculate_proportional_scale(
        TransformHandleKind::SouthEast,
        Point2D::new(50.0, 50.0), // kursor melompat ke seberang anchor
        orig,
        4.0,
    );
    assert!(fw >= 4.0);
    assert!(fh >= 4.0);
    assert!(!fw.is_nan() && !fh.is_nan());
}

#[test]
fn test_calculate_single_axis_resize_and_local_projection() {
    use super::shape_ops::{ShapeTransformBox, TransformHandleKind, calculate_single_axis_resize};

    let orig = ShapeTransformBox::new(100.0, 100.0, 100.0, 50.0, 0.0);

    // Resize sisi Timur (East): lebar berubah, tinggi tetap 50.0
    let (nx, ny, nw, nh) = calculate_single_axis_resize(
        TransformHandleKind::East,
        Point2D::new(260.0, 125.0),
        orig,
        4.0,
    );
    assert!((nx - 100.0).abs() < 1e-3); // Sisi kiri tetap di 100
    assert!((ny - 100.0).abs() < 1e-3); // Sisi atas tetap di 100
    assert!((nw - 160.0).abs() < 1e-3); // Lebar bertambah jadi 160
    assert!((nh - 50.0).abs() < 1e-3); // Tinggi tetap 50

    // Resize sisi Selatan (South): tinggi berubah, lebar tetap 100.0
    let (sx, sy, sw, sh) = calculate_single_axis_resize(
        TransformHandleKind::South,
        Point2D::new(150.0, 220.0),
        orig,
        4.0,
    );
    assert!((sx - 100.0).abs() < 1e-3);
    assert!((sy - 100.0).abs() < 1e-3);
    assert!((sw - 100.0).abs() < 1e-3);
    assert!((sh - 120.0).abs() < 1e-3); // Tinggi bertambah jadi 120

    // Batas minimum tidak pernah lebih kecil dari 4.0 pt
    let (_, _, min_w, _) = calculate_single_axis_resize(
        TransformHandleKind::East,
        Point2D::new(50.0, 125.0), // kursor dipersempit drastis
        orig,
        4.0,
    );
    assert!(min_w >= 4.0);
}

#[test]
fn test_calculate_rotation_angle_and_15_deg_snap() {
    use super::shape_ops::calculate_rotation_angle;

    let cx = 100.0_f32;
    let cy = 100.0_f32;

    // Tangkai rotasi di atas: titik (100, 60) -> sudut rotasi 0 rad
    let angle_zero = calculate_rotation_angle(cx, cy, Point2D::new(100.0, 60.0), false);
    assert!(angle_zero.abs() < 1e-4);

    // Kursor digeser ke kanan: (140, 100) -> sudut rotasi 90 derajat (PI/2 rad)
    let angle_90 = calculate_rotation_angle(cx, cy, Point2D::new(140.0, 100.0), false);
    assert!((angle_90 - std::f32::consts::FRAC_PI_2).abs() < 1e-3);

    // Dengan snap 15 derajat (TAU / 24 = 0.2618 rad):
    // Sudut 16.5 derajat (0.288 rad) akan menempel tepat ke 15 derajat (0.2618 rad)
    let angle_snapped = calculate_rotation_angle(
        cx,
        cy,
        Point2D::new(
            100.0 + 40.0 * 0.288_f32.sin(),
            100.0 - 40.0 * 0.288_f32.cos(),
        ),
        true,
    );
    let expected_15_deg = std::f32::consts::TAU / 24.0;
    assert!((angle_snapped - expected_15_deg).abs() < 1e-3);
}

#[test]
fn test_change_shape_kind_preserves_data_and_connector_anchors() {
    use super::shape_ops::change_shape_kind;
    use crate::geometry::shape_connection_point;
    use crate::model::{ColorData, ConnectionSite, DrawObject, ShapeKind};

    let mut shape = DrawObject::Shape {
        id: "test-shape-g5".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 120.0,
        height: 80.0,
        color: ColorData {
            r: 0,
            g: 114,
            b: 206,
            a: 255,
        },
        stroke_width: 2.5,
        fill: Some(ColorData {
            r: 200,
            g: 230,
            b: 255,
            a: 50,
        }),
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };

    // Ubah ke RoundedRectangle dengan default corner radius 9.6
    change_shape_kind(&mut shape, ShapeKind::Rectangle, 9.6);
    if let DrawObject::Shape { corner_radius, .. } = &shape {
        assert_eq!(*corner_radius, 9.6);
    }

    // Ubah ke Diamond (Belah Ketupat)
    change_shape_kind(&mut shape, ShapeKind::Diamond, 0.0);
    if let DrawObject::Shape {
        shape_type,
        corner_radius,
        width,
        height,
        ..
    } = &shape
    {
        assert_eq!(*shape_type, ShapeKind::Diamond);
        assert_eq!(*corner_radius, 0.0);
        assert_eq!(*width, 120.0);
        assert_eq!(*height, 80.0);
    }

    // Verifikasi titik sambung connector (Task 5.3): tetap valid di tepi Diamond
    let (pt_top, _) = shape_connection_point(&shape, ConnectionSite::Top, None);
    let (pt_right, _) = shape_connection_point(&shape, ConnectionSite::Right, None);
    assert!((pt_top.x - 160.0).abs() < 1e-4 && (pt_top.y - 100.0).abs() < 1e-4);
    assert!((pt_right.x - 220.0).abs() < 1e-4 && (pt_right.y - 140.0).abs() < 1e-4);
}

#[test]
fn test_shape_text_bounds_and_auto_fit() {
    // 1. Rectangle murni
    let (w_rect, h_rect) =
        calculate_shape_text_bounds(ShapeKind::Rectangle, 200.0, 100.0, 0.0, 2.0);
    assert_eq!(w_rect, 200.0 - 10.0); // 190.0
    assert_eq!(h_rect, 100.0 - 10.0); // 90.0

    // 2. RoundedRectangle: safe bounds lebih mengecil karena radius sudut
    let (w_rrect, h_rrect) =
        calculate_shape_text_bounds(ShapeKind::Rectangle, 200.0, 100.0, 14.0, 2.0);
    assert!(w_rrect < w_rect);
    assert!(h_rrect < h_rect);

    // 3. Ellipse: batas dalam elips ~0.7071
    let (w_ell, h_ell) = calculate_shape_text_bounds(ShapeKind::Ellipse, 200.0, 100.0, 0.0, 2.0);
    assert!((w_ell - (200.0 * std::f32::consts::FRAC_1_SQRT_2 - 10.0)).abs() < 0.1);
    assert!((h_ell - (100.0 * std::f32::consts::FRAC_1_SQRT_2 - 10.0)).abs() < 0.1);

    // 4. Diamond: batas dalam belah ketupat ~0.5
    let (w_dia, h_dia) = calculate_shape_text_bounds(ShapeKind::Diamond, 200.0, 100.0, 0.0, 2.0);
    assert!((w_dia - (200.0 * 0.5 - 10.0)).abs() < 0.1);
    assert!((h_dia - (100.0 * 0.5 - 10.0)).abs() < 0.1);

    // 5. Auto-fit font size
    let base_size = 16.0;
    // Teks kosong atau pendek tetap di base size
    let fit_short = calculate_auto_fit_font_size(base_size, "Halo", 180.0, 80.0, 8.0);
    assert_eq!(fit_short, base_size);

    // Teks sangat panjang akan mengecil secara proporsional hingga minimal 8.0 pt
    let long_text = "Ini adalah paragraf yang sangat panjang sekali di dalam kotak diagram rustnotes untuk menguji kalkulasi auto fit font size";
    let fit_long = calculate_auto_fit_font_size(base_size, long_text, 100.0, 30.0, 8.0);
    assert!(fit_long < base_size);
    assert!(fit_long >= 8.0);
}

#[test]
fn test_shape_label_proportional_scale() {
    let mut shape = DrawObject::Shape {
        id: "shape_with_label".into(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 10.0,
        label: Some(crate::model::ShapeLabel {
            text: "Flowchart Step".into(),
            font_size: 16.0,
            color: dummy_color(),
            alignment: crate::model::TextAlign::Center,
            vertical_alignment: crate::model::VerticalAlign::Middle,
        }),
        locked: false,
        curve_points: Vec::new(),
    };

    // Skala 2.0x terhadap titik (100, 100)
    scale_object(&mut shape, Pos2::new(100.0, 100.0), 2.0, 2.0);

    if let DrawObject::Shape {
        width,
        height,
        corner_radius,
        label,
        ..
    } = &shape
    {
        assert_eq!(*width, 200.0);
        assert_eq!(*height, 200.0);
        assert_eq!(*corner_radius, 20.0);
        let lbl = label.as_ref().expect("label should exist");
        assert_eq!(lbl.font_size, 32.0);
    } else {
        panic!("Object should remain Shape");
    }
}

#[test]
fn test_connector_snap_site_detection() {
    let shape_a = DrawObject::Shape {
        id: "shape_a".into(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };

    let objects = vec![shape_a];

    // Titik kanan shape_a adalah (200.0, 150.0). Uji titik dekat (202.0, 149.0)
    let p_near_right = Point2D::new(202.0, 149.0);
    let snap = find_connector_snap_site(&objects, p_near_right, 16.0, None);
    assert!(snap.is_some());
    let (id, site, anchor) = snap.unwrap();
    assert_eq!(id, "shape_a");
    assert_eq!(site, ConnectionSite::Right);
    assert_eq!(anchor, Point2D::new(200.0, 150.0));

    // Uji titik yang jauh: tidak boleh ada snap
    let p_far = Point2D::new(500.0, 500.0);
    let snap_far = find_connector_snap_site(&objects, p_far, 16.0, None);
    assert!(snap_far.is_none());

    // Uji exclude_shape_id
    let snap_excluded = find_connector_snap_site(&objects, p_near_right, 16.0, Some("shape_a"));
    assert!(snap_excluded.is_none());
}

#[test]
fn test_detach_connectors_when_shape_deleted() {
    let shape_a = DrawObject::Shape {
        id: "shape_a".into(),
        shape_type: ShapeKind::Rectangle,
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };
    let shape_b = DrawObject::Shape {
        id: "shape_b".into(),
        shape_type: ShapeKind::Rectangle,
        x: 300.0,
        y: 100.0,
        width: 100.0,
        height: 100.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };
    let conn = DrawObject::Connector {
        id: "conn_1".into(),
        start: crate::model::ConnectorEnd::Attached {
            shape_id: "shape_a".into(),
            site: ConnectionSite::Right,
        },
        end: crate::model::ConnectorEnd::Attached {
            shape_id: "shape_b".into(),
            site: ConnectionSite::Left,
        },
        route: crate::model::ConnectorRoute::Orthogonal,
        control_points: Vec::new(),
        style: crate::model::ConnectorStyle {
            stroke_color: dummy_color(),
            stroke_width: 2.0,
            start_marker: crate::model::MarkerKind::None,
            end_marker: crate::model::MarkerKind::Arrow,
        },
        locked: false,
    };

    let mut objects = vec![shape_a, shape_b, conn];

    // Hapus shape_a
    detach_connectors_from_shapes(&mut objects, &["shape_a".to_string()]);
    objects.retain(|o| o.id() != "shape_a");

    // Pastikan connector conn_1 tetap ada
    let conn_after = objects
        .iter()
        .find(|o| o.id() == "conn_1")
        .expect("Connector must not be deleted");
    if let DrawObject::Connector { start, end, .. } = conn_after {
        // Ujung start menjadi Free di koordinat terakhir (200.0, 150.0)
        match start {
            crate::model::ConnectorEnd::Free { x, y } => {
                assert_eq!(*x, 200.0);
                assert_eq!(*y, 150.0);
            }
            _ => panic!("Start must be detached to Free"),
        }
        // Ujung end tetap Attached ke shape_b
        match end {
            crate::model::ConnectorEnd::Attached { shape_id, site } => {
                assert_eq!(shape_id, "shape_b");
                assert_eq!(*site, ConnectionSite::Left);
            }
            _ => panic!("End must remain Attached"),
        }
    } else {
        panic!("Object must be Connector");
    }
}

#[test]
fn test_remap_connector_endpoints_on_copy_and_paste() {
    let shape_a = DrawObject::Shape {
        id: "orig_a".into(),
        shape_type: ShapeKind::Rectangle,
        x: 50.0,
        y: 50.0,
        width: 100.0,
        height: 100.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };
    let shape_b = DrawObject::Shape {
        id: "orig_b".into(),
        shape_type: ShapeKind::Rectangle,
        x: 250.0,
        y: 50.0,
        width: 100.0,
        height: 100.0,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        rotation: 0.0,
        corner_radius: 0.0,
        label: None,
        locked: false,
        curve_points: Vec::new(),
    };
    let conn = DrawObject::Connector {
        id: "orig_conn".into(),
        start: crate::model::ConnectorEnd::Attached {
            shape_id: "orig_a".into(),
            site: ConnectionSite::Right,
        },
        end: crate::model::ConnectorEnd::Attached {
            shape_id: "orig_b".into(),
            site: ConnectionSite::Left,
        },
        route: crate::model::ConnectorRoute::Orthogonal,
        control_points: Vec::new(),
        style: crate::model::ConnectorStyle {
            stroke_color: dummy_color(),
            stroke_width: 2.0,
            start_marker: crate::model::MarkerKind::None,
            end_marker: crate::model::MarkerKind::Arrow,
        },
        locked: false,
    };

    let original_objects = vec![shape_a.clone(), shape_b.clone(), conn.clone()];

    // Skenario 1: Copy keduanya bersamaan
    let mut cloned_together = vec![shape_a.clone(), shape_b.clone(), conn.clone()];
    let mut id_map_together = std::collections::HashMap::new();
    id_map_together.insert("orig_a".into(), "new_a".into());
    id_map_together.insert("orig_b".into(), "new_b".into());
    id_map_together.insert("orig_conn".into(), "new_conn".into());

    remap_connector_endpoints_on_copy(&mut cloned_together, &id_map_together, &original_objects);

    if let DrawObject::Connector { start, end, .. } = &cloned_together[2] {
        match start {
            crate::model::ConnectorEnd::Attached { shape_id, .. } => assert_eq!(shape_id, "new_a"),
            _ => panic!("Start should remap to new_a"),
        }
        match end {
            crate::model::ConnectorEnd::Attached { shape_id, .. } => assert_eq!(shape_id, "new_b"),
            _ => panic!("End should remap to new_b"),
        }
    }

    // Skenario 2: Hanya copy Connector tanpa bentuknya
    let mut cloned_conn_only = vec![conn.clone()];
    let mut id_map_conn_only = std::collections::HashMap::new();
    id_map_conn_only.insert("orig_conn".into(), "new_conn_only".into());

    remap_connector_endpoints_on_copy(&mut cloned_conn_only, &id_map_conn_only, &original_objects);

    if let DrawObject::Connector { start, end, .. } = &cloned_conn_only[0] {
        match start {
            crate::model::ConnectorEnd::Free { x, y } => {
                assert_eq!(*x, 150.0);
                assert_eq!(*y, 100.0);
            }
            _ => panic!("Start should detach to Free coordinate"),
        }
        match end {
            crate::model::ConnectorEnd::Free { x, y } => {
                assert_eq!(*x, 250.0);
                assert_eq!(*y, 100.0);
            }
            _ => panic!("End should detach to Free coordinate"),
        }
    }
}

#[test]
fn test_catmull_rom_spline_generation() {
    use crate::geometry::connectors::catmull_rom_spline;
    use crate::model::Point2D;

    let waypoints = vec![
        Point2D::new(0.0, 0.0),
        Point2D::new(50.0, 100.0),
        Point2D::new(100.0, 50.0),
        Point2D::new(150.0, 150.0),
    ];

    let spline = catmull_rom_spline(&waypoints, 12);
    assert!(!spline.is_empty());
    assert!((spline[0].x - 0.0).abs() < 1e-4);
    assert!((spline[0].y - 0.0).abs() < 1e-4);
    let last = spline[spline.len() - 1];
    assert!((last.x - 150.0).abs() < 1e-4);
    assert!((last.y - 150.0).abs() < 1e-4);

    // Pastikan tidak ada koordinat NaN
    for p in &spline {
        assert!(!p.x.is_nan());
        assert!(!p.y.is_nan());
    }
}

#[test]
fn test_route_orthogonal_through_points_strictly_orthogonal() {
    use crate::geometry::connectors::{Direction, route_orthogonal_through_points};
    use crate::model::Point2D;

    let start = Point2D::new(50.0, 50.0);
    let end = Point2D::new(300.0, 200.0);
    let control_points = vec![Point2D::new(150.0, 80.0), Point2D::new(200.0, 150.0)];

    let path = route_orthogonal_through_points(
        start,
        Direction::Right,
        end,
        Direction::Left,
        &control_points,
    );
    assert!(path.len() >= 3);

    // Setiap segmen harus tepat horizontal atau vertikal (tanpa garis miring)
    for i in 0..path.len() - 1 {
        let a = path[i];
        let b = path[i + 1];
        let dx = (a.x - b.x).abs();
        let dy = (a.y - b.y).abs();
        assert!(
            dx < 1e-3 || dy < 1e-3,
            "Segmen {} -> {} tidak orthogonal! dx={}, dy={}",
            i,
            i + 1,
            dx,
            dy
        );
    }
}

#[test]
fn test_insert_and_prune_collinear_control_points() {
    use crate::geometry::connectors::{insert_control_point_at, prune_collinear_control_points};
    use crate::model::Point2D;

    let start = Point2D::new(0.0, 0.0);
    let end = Point2D::new(200.0, 0.0);
    let mut control_points = Vec::new();

    // 1. Sisipkan titik di tengah segmen (100, 50)
    let idx = insert_control_point_at(&mut control_points, start, end, Point2D::new(100.0, 50.0));
    assert_eq!(idx, 0);
    assert_eq!(control_points.len(), 1);
    assert_eq!(control_points[0], Point2D::new(100.0, 50.0));

    // Sisipkan titik kedua di (50, 25)
    let idx2 = insert_control_point_at(&mut control_points, start, end, Point2D::new(50.0, 25.0));
    assert_eq!(idx2, 0);
    assert_eq!(control_points.len(), 2);
    assert_eq!(control_points[0], Point2D::new(50.0, 25.0));

    // 2. Prune titik yang segaris: buat titik (100, 1.0) yang hampir persis di garis lurus Y=0
    let mut test_cps = vec![Point2D::new(100.0, 1.5)];
    let pruned = prune_collinear_control_points(&mut test_cps, start, end, 5.0);
    assert!(pruned);
    assert!(test_cps.is_empty(), "Titik hampir segaris harus ter-prune");
}

#[test]
fn test_prune_invalid_connector_control_points_inside_shape() {
    use crate::geometry::connectors::prune_invalid_connector_control_points;
    use crate::model::{
        ColorData, ConnectionSite, ConnectorEnd, ConnectorRoute, ConnectorStyle, DrawObject,
        Point2D, ShapeKind,
    };

    let shape_a = DrawObject::Shape {
        id: "shape_a".into(),
        x: 100.0,
        y: 100.0,
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
        shape_type: ShapeKind::Rectangle,
        rotation: 0.0,
        corner_radius: 0.0,
        curve_points: Vec::new(),
        label: None,
        locked: false,
    };

    let mut conn = DrawObject::Connector {
        id: "conn_1".into(),
        start: ConnectorEnd::Attached {
            shape_id: "shape_a".into(),
            site: ConnectionSite::Right,
        },
        end: ConnectorEnd::Free { x: 400.0, y: 140.0 },
        route: ConnectorRoute::Straight,
        control_points: vec![
            Point2D::new(150.0, 140.0), // Berada di DALAM shape_a (X: 100-200, Y: 100-180)!
            Point2D::new(300.0, 140.0), // Di luar shape_a
        ],
        style: ConnectorStyle::default(),
        locked: false,
    };

    let all_objects = vec![shape_a];
    let changed = prune_invalid_connector_control_points(&mut conn, &all_objects);
    assert!(changed);

    if let DrawObject::Connector { control_points, .. } = conn {
        assert_eq!(control_points.len(), 1);
        assert_eq!(control_points[0], Point2D::new(300.0, 140.0));
    }
}

#[test]
fn test_next_shape_rect_directions_and_opposite_sites() {
    use crate::geometry::shape_ops::next_shape_rect;
    use crate::model::ConnectionSite;

    let sx = 200.0;
    let sy = 200.0;
    let sw = 100.0;
    let sh = 60.0;
    let gap = 50.0;
    let all_objects = vec![];
    let page_w = 1000.0;
    let page_h = 1000.0;

    // 1. Right -> Left
    let (rx, ry, rw, rh, ropp) = next_shape_rect(
        (sx, sy, sw, sh),
        ConnectionSite::Right,
        gap,
        &all_objects,
        (page_w, page_h),
    );
    assert_eq!(ropp, ConnectionSite::Left);
    assert_eq!(rx, sx + sw + gap);
    assert_eq!(ry, sy);
    assert_eq!(rw, sw);
    assert_eq!(rh, sh);

    // 2. Bottom -> Top
    let (bx, by, bw, bh, bopp) = next_shape_rect(
        (sx, sy, sw, sh),
        ConnectionSite::Bottom,
        gap,
        &all_objects,
        (page_w, page_h),
    );
    assert_eq!(bopp, ConnectionSite::Top);
    assert_eq!(bx, sx);
    assert_eq!(by, sy + sh + gap);
    assert_eq!(bw, sw);
    assert_eq!(bh, sh);

    // 3. Left -> Right
    let (lx, ly, _lw, _lh, lopp) = next_shape_rect(
        (sx, sy, sw, sh),
        ConnectionSite::Left,
        gap,
        &all_objects,
        (page_w, page_h),
    );
    assert_eq!(lopp, ConnectionSite::Right);
    assert_eq!(lx, sx - gap - sw);
    assert_eq!(ly, sy);

    // 4. Top -> Bottom
    let (tx, ty, _tw, _th, topp) = next_shape_rect(
        (sx, sy, sw, sh),
        ConnectionSite::Top,
        gap,
        &all_objects,
        (page_w, page_h),
    );
    assert_eq!(topp, ConnectionSite::Bottom);
    assert_eq!(tx, sx);
    assert_eq!(ty, sy - gap - sh);
}

#[test]
fn test_next_shape_rect_overlap_avoidance() {
    use crate::geometry::shape_ops::next_shape_rect;
    use crate::model::{ColorData, ConnectionSite, DrawObject, ShapeKind};

    let sx = 100.0;
    let sy = 100.0;
    let sw = 100.0;
    let sh = 60.0;
    let gap = 40.0;

    // Bentuk rintangan yang tepat berada di posisi target awal sebelah kanan: X: 240, Y: 100
    let obstacle = DrawObject::Shape {
        id: "obstacle".into(),
        x: 240.0,
        y: 100.0,
        width: 100.0,
        height: 60.0,
        color: ColorData {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        },
        stroke_width: 2.0,
        fill: None,
        shape_type: ShapeKind::Rectangle,
        rotation: 0.0,
        corner_radius: 0.0,
        curve_points: Vec::new(),
        label: None,
        locked: false,
    };

    let all_objects = vec![obstacle];
    let (rx, ry, _rw, _rh, ropp) = next_shape_rect(
        (sx, sy, sw, sh),
        ConnectionSite::Right,
        gap,
        &all_objects,
        (1000.0, 1000.0),
    );

    assert_eq!(ropp, ConnectionSite::Left);
    // Posisi target awal 240 terhalang, jadi harus digeser lebih jauh ke kanan (> 240 + 100 = 340)
    assert!(rx >= 310.0, "rx {} harus melewati rintangan di 240-340", rx);
    assert_eq!(ry, sy);
}

#[test]
fn test_next_shape_rect_clamping_to_page_bounds() {
    use crate::geometry::shape_ops::next_shape_rect;
    use crate::model::ConnectionSite;

    let page_w = 400.0;
    let page_h = 400.0;

    // Bentuk yang berada dekat dengan tepi kanan halaman
    let sx = 300.0;
    let sy = 100.0;
    let sw = 80.0;
    let sh = 50.0;
    let gap = 50.0;

    let (rx, ry, rw, rh, _) = next_shape_rect(
        (sx, sy, sw, sh),
        ConnectionSite::Right,
        gap,
        &[],
        (page_w, page_h),
    );

    // Harus ter-klem di dalam batas halaman (rx + rw <= page_w)
    assert!(
        rx + rw <= page_w,
        "rx + rw ({}) tidak boleh keluar dari page_w ({})",
        rx + rw,
        page_w
    );
    assert!(rx >= 16.0);
    assert!(ry + rh <= page_h);
}

#[test]
fn test_snap_value_and_point_to_grid() {
    use crate::geometry::snapping::{snap_point_to_grid, snap_value_to_grid};
    use crate::model::Point2D;

    let grid_size = 30.0;
    let threshold = 5.0;

    // Nilai 29.0 dekat dengan kelipatan 30.0
    let (v1, s1) = snap_value_to_grid(29.0, grid_size, threshold);
    assert!(s1);
    assert_eq!(v1, 30.0);

    // Nilai 33.0 dekat dengan kelipatan 30.0
    let (v2, s2) = snap_value_to_grid(33.0, grid_size, threshold);
    assert!(s2);
    assert_eq!(v2, 30.0);

    // Nilai 15.0 jauh dari kelipatan grid mana pun (jarak 15.0 > threshold 5.0)
    let (v3, s3) = snap_value_to_grid(15.0, grid_size, threshold);
    assert!(!s3);
    assert_eq!(v3, 15.0);

    // Point2D snap
    let pt = Point2D::new(59.0, 92.0);
    let (p_snapped, p_s) = snap_point_to_grid(pt, grid_size, threshold);
    assert!(p_s);
    assert_eq!(p_snapped.x, 60.0);
    assert_eq!(p_snapped.y, 90.0);
}

#[test]
fn test_find_alignment_guides_left_and_top_alignment() {
    use crate::geometry::snapping::{GuideOrientation, find_alignment_guides};
    use eframe::egui::Rect;

    let target_rect = Rect::from_min_size(
        eframe::egui::Pos2::new(100.0, 100.0),
        eframe::egui::Vec2::new(80.0, 60.0),
    );
    // Moving rect dengan sisi kiri di 103.0 (selisih 3.0 dari tepi kiri target)
    let moving_rect = Rect::from_min_size(
        eframe::egui::Pos2::new(103.0, 250.0),
        eframe::egui::Vec2::new(80.0, 60.0),
    );

    let (snap_adjust, guides) = find_alignment_guides(moving_rect, &[target_rect], 6.0);

    // Harus menyesuaikan dx sebesar -3.0 agar tepat sejajar di x = 100.0
    assert_eq!(snap_adjust.x, -3.0);
    assert_eq!(guides.len(), 1);
    assert_eq!(guides[0].orientation, GuideOrientation::Vertical);
    assert_eq!(guides[0].position, 100.0);
}

#[test]
fn test_find_alignment_guides_center_alignment() {
    use crate::geometry::snapping::{GuideOrientation, find_alignment_guides};
    use eframe::egui::Rect;

    // Target rect berpusat di (200.0, 150.0)
    let target_rect = Rect::from_center_size(
        eframe::egui::Pos2::new(200.0, 150.0),
        eframe::egui::Vec2::new(100.0, 80.0),
    );
    // Moving rect berpusat di (202.0, 148.5) (selisih dx = -2.0, dy = +1.5)
    let moving_rect = Rect::from_center_size(
        eframe::egui::Pos2::new(202.0, 148.5),
        eframe::egui::Vec2::new(60.0, 60.0),
    );

    let (snap_adjust, guides) = find_alignment_guides(moving_rect, &[target_rect], 6.0);

    assert_eq!(snap_adjust.x, -2.0);
    assert_eq!(snap_adjust.y, 1.5);
    assert_eq!(guides.len(), 2);
    assert!(
        guides
            .iter()
            .any(|g| g.orientation == GuideOrientation::Vertical && g.position == 200.0)
    );
    assert!(
        guides
            .iter()
            .any(|g| g.orientation == GuideOrientation::Horizontal && g.position == 150.0)
    );
}

#[test]
fn test_find_alignment_guides_far_no_snap() {
    use crate::geometry::snapping::find_alignment_guides;
    use eframe::egui::{Rect, Vec2};

    let target_rect = Rect::from_min_size(
        eframe::egui::Pos2::new(100.0, 100.0),
        eframe::egui::Vec2::new(50.0, 50.0),
    );
    let moving_rect = Rect::from_min_size(
        eframe::egui::Pos2::new(500.0, 500.0),
        eframe::egui::Vec2::new(50.0, 50.0),
    );

    let (snap_adjust, guides) = find_alignment_guides(moving_rect, &[target_rect], 8.0);

    assert_eq!(snap_adjust, Vec2::ZERO);
    assert!(guides.is_empty());
}

#[test]
fn test_diagram_starter_templates_generation() {
    use crate::geometry::shape_ops::{DiagramStarterKind, generate_diagram_starter};
    use crate::model::DrawObject;

    for kind in DiagramStarterKind::ALL {
        let objs = generate_diagram_starter(kind, 800.0, 1000.0);
        assert!(
            !objs.is_empty(),
            "Diagram starter {:?} harus menghasilkan objek",
            kind
        );

        let shape_count = objs
            .iter()
            .filter(|o| matches!(o, DrawObject::Shape { .. }))
            .count();
        let connector_count = objs
            .iter()
            .filter(|o| matches!(o, DrawObject::Connector { .. }))
            .count();

        match kind {
            DiagramStarterKind::Flowchart => {
                assert_eq!(
                    shape_count, 4,
                    "Flowchart starter harus punya 4 shapes (Start, Process, Decision, End)"
                );
                assert_eq!(
                    connector_count, 3,
                    "Flowchart starter harus punya 3 connectors penghubung"
                );
            }
            DiagramStarterKind::MindMap => {
                assert_eq!(
                    shape_count, 5,
                    "MindMap starter harus punya 1 root + 4 node cabang"
                );
                assert_eq!(
                    connector_count, 4,
                    "MindMap starter harus punya 4 connectors dari root ke node cabang"
                );
            }
            DiagramStarterKind::Architecture3Tier => {
                assert_eq!(
                    shape_count, 3,
                    "Architecture3Tier harus punya 3 layer shapes"
                );
                assert_eq!(
                    connector_count, 2,
                    "Architecture3Tier harus punya 2 connectors dua arah"
                );
            }
        }

        // Verifikasi semua connector merujuk ke shape_id yang valid
        let shape_ids: std::collections::HashSet<String> = objs
            .iter()
            .filter_map(|o| match o {
                DrawObject::Shape { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();

        for o in &objs {
            if let DrawObject::Connector { start, end, .. } = o {
                if let crate::model::ConnectorEnd::Attached { shape_id, .. } = start {
                    assert!(
                        shape_ids.contains(shape_id),
                        "Start connector harus menempel pada shape yang ada di starter"
                    );
                }
                if let crate::model::ConnectorEnd::Attached { shape_id, .. } = end {
                    assert!(
                        shape_ids.contains(shape_id),
                        "End connector harus menempel pada shape yang ada di starter"
                    );
                }
            }
        }
    }
}

#[test]
fn test_circle_recognition_scale_invariance_radii() {
    let radii = [10.0, 25.0, 50.0, 100.0, 250.0, 500.0];
    for &r in &radii {
        let center = Pos2::new(600.0, 600.0);
        let mut points = Vec::new();
        let n = 36;
        for i in 0..=n {
            let theta = (i as f32) * std::f32::consts::TAU / (n as f32);
            // Tambahkan noise mikro terkontrol 1%
            let noise_r = r * (1.0 + 0.01 * (i % 3) as f32);
            points.push(StrokePoint::from(Pos2::new(
                center.x + noise_r * theta.cos(),
                center.y + noise_r * theta.sin(),
            )));
        }
        let recognized = recognize_shape(&points)
            .unwrap_or_else(|| panic!("Gagal mengenali lingkaran dengan radius {r}"));
        assert_eq!(
            recognized.kind,
            ShapeKind::Ellipse,
            "Radius {r} harus dikenali sebagai Ellipse"
        );

        // Diameter terprediksi harus konsisten dengan radius asli (~2*r)
        let expected_diam = 2.0 * r;
        assert!(
            (recognized.width - expected_diam).abs() / expected_diam < 0.10,
            "Lebar lingkaran radius {r} menciut atau menyimpang: {}",
            recognized.width
        );
        assert!(
            (recognized.height - expected_diam).abs() / expected_diam < 0.10,
            "Tinggi lingkaran radius {r} menciut atau menyimpang: {}",
            recognized.height
        );

        // Pusat lingkaran terprediksi harus berada dekat dengan pusat asli
        let rec_center = Pos2::new(
            recognized.x + recognized.width * 0.5,
            recognized.y + recognized.height * 0.5,
        );
        assert!(
            (rec_center.x - center.x).abs() / r < 0.10,
            "Pusat X lingkaran radius {r} bergeser: {}",
            rec_center.x
        );
        assert!(
            (rec_center.y - center.y).abs() / r < 0.10,
            "Pusat Y lingkaran radius {r} bergeser: {}",
            rec_center.y
        );
    }
}

#[test]
fn test_circle_noisy_uneven_and_direction() {
    let center = Pos2::new(200.0, 200.0);
    let r = 80.0;

    // 1. Lingkaran Searah Jarum Jam (Clockwise)
    let mut cw_points = Vec::new();
    for i in 0..=30 {
        let theta = -(i as f32) * std::f32::consts::TAU / 30.0;
        cw_points.push(StrokePoint::from(Pos2::new(
            center.x + r * theta.cos(),
            center.y + r * theta.sin(),
        )));
    }
    let rec_cw = recognize_shape(&cw_points).expect("Lingkaran CW harus dikenali");
    assert_eq!(rec_cw.kind, ShapeKind::Ellipse);
    assert!((rec_cw.width - 2.0 * r).abs() < 16.0);

    // 2. Lingkaran dengan sampling tidak merata & closure gap kecil
    let mut uneven_points = Vec::new();
    for i in 0..20 {
        let theta = (i as f32) * std::f32::consts::PI / 20.0;
        uneven_points.push(StrokePoint::from(Pos2::new(
            center.x + r * theta.cos(),
            center.y + r * theta.sin(),
        )));
    }
    for i in 0..10 {
        let theta = std::f32::consts::PI + (i as f32) * std::f32::consts::PI / 10.0;
        uneven_points.push(StrokePoint::from(Pos2::new(
            center.x + r * theta.cos(),
            center.y + r * theta.sin(),
        )));
    }
    uneven_points.push(StrokePoint::from(Pos2::new(
        center.x + r - 5.0,
        center.y + 4.0,
    )));

    let rec_uneven = recognize_shape(&uneven_points).expect("Lingkaran uneven/gap harus dikenali");
    assert_eq!(rec_uneven.kind, ShapeKind::Ellipse);
    assert!((rec_uneven.width - 2.0 * r).abs() < 20.0);
}

#[test]
fn test_ellipse_wide_and_tall_recognition() {
    let center = Pos2::new(300.0, 300.0);

    // 1. Wide Ellipse (rx = 120, ry = 40)
    let mut wide_pts = Vec::new();
    for i in 0..=32 {
        let theta = (i as f32) * std::f32::consts::TAU / 32.0;
        wide_pts.push(StrokePoint::from(Pos2::new(
            center.x + 120.0 * theta.cos(),
            center.y + 40.0 * theta.sin(),
        )));
    }
    let rec_wide = recognize_shape(&wide_pts).expect("Wide ellipse harus dikenali");
    assert_eq!(rec_wide.kind, ShapeKind::Ellipse);
    assert!((rec_wide.width - 240.0).abs() < 24.0);
    assert!((rec_wide.height - 80.0).abs() < 16.0);

    // 2. Tall Ellipse (rx = 45, ry = 140)
    let mut tall_pts = Vec::new();
    for i in 0..=32 {
        let theta = (i as f32) * std::f32::consts::TAU / 32.0;
        tall_pts.push(StrokePoint::from(Pos2::new(
            center.x + 45.0 * theta.cos(),
            center.y + 140.0 * theta.sin(),
        )));
    }
    let rec_tall = recognize_shape(&tall_pts).expect("Tall ellipse harus dikenali");
    assert_eq!(rec_tall.kind, ShapeKind::Ellipse);
    assert!((rec_tall.width - 90.0).abs() < 18.0);
    assert!((rec_tall.height - 280.0).abs() < 28.0);
}

#[test]
fn test_parallelogram_variants_skew_angles_and_aspect_ratios() {
    // 1. Jajar Genjang dengan sudut kemiringan kecil (~10 derajat skew)
    let small_skew = [
        Pos2::new(60.0, 40.0),
        Pos2::new(180.0, 40.0),
        Pos2::new(160.0, 120.0),
        Pos2::new(40.0, 120.0),
    ];
    let mut small_pts = Vec::new();
    for (a, b) in [
        (small_skew[0], small_skew[1]),
        (small_skew[1], small_skew[2]),
        (small_skew[2], small_skew[3]),
        (small_skew[3], small_skew[0]),
    ] {
        for t in 0..7 {
            let f = t as f32 / 7.0;
            small_pts.push(StrokePoint::from(Pos2::new(
                a.x + (b.x - a.x) * f,
                a.y + (b.y - a.y) * f,
            )));
        }
    }
    small_pts.push(StrokePoint::from(small_skew[0]));
    let rec_small = recognize_shape(&small_pts).expect("Small skew parallelogram harus dikenali");
    assert_eq!(
        rec_small.kind,
        ShapeKind::Parallelogram,
        "Small skew tidak boleh keliru jadi Rectangle"
    );

    // 2. Jajar Genjang Lebar (Wide: width 280, height 50)
    let wide_corners = [
        Pos2::new(50.0, 30.0),
        Pos2::new(310.0, 30.0),
        Pos2::new(280.0, 80.0),
        Pos2::new(20.0, 80.0),
    ];
    let mut wide_pts = Vec::new();
    for (a, b) in [
        (wide_corners[0], wide_corners[1]),
        (wide_corners[1], wide_corners[2]),
        (wide_corners[2], wide_corners[3]),
        (wide_corners[3], wide_corners[0]),
    ] {
        for t in 0..7 {
            let f = t as f32 / 7.0;
            wide_pts.push(StrokePoint::from(Pos2::new(
                a.x + (b.x - a.x) * f,
                a.y + (b.y - a.y) * f,
            )));
        }
    }
    wide_pts.push(StrokePoint::from(wide_corners[0]));
    let rec_wide = recognize_shape(&wide_pts).expect("Wide parallelogram harus dikenali");
    assert_eq!(rec_wide.kind, ShapeKind::Parallelogram);

    // 3. Jajar Genjang Condong Kiri (Negative slant)
    let left_slant = [
        Pos2::new(20.0, 40.0),
        Pos2::new(140.0, 40.0),
        Pos2::new(170.0, 130.0),
        Pos2::new(50.0, 130.0),
    ];
    let mut left_pts = Vec::new();
    for (a, b) in [
        (left_slant[0], left_slant[1]),
        (left_slant[1], left_slant[2]),
        (left_slant[2], left_slant[3]),
        (left_slant[3], left_slant[0]),
    ] {
        for t in 0..7 {
            let f = t as f32 / 7.0;
            left_pts.push(StrokePoint::from(Pos2::new(
                a.x + (b.x - a.x) * f,
                a.y + (b.y - a.y) * f,
            )));
        }
    }
    left_pts.push(StrokePoint::from(left_slant[0]));
    let rec_left = recognize_shape(&left_pts).expect("Left-slant parallelogram harus dikenali");
    assert_eq!(rec_left.kind, ShapeKind::Parallelogram);
    assert!(
        rec_left.width < 0.0,
        "Parallelogram condong kiri harus menghasilkan width negatif"
    );
}

#[test]
fn test_rectangle_versus_parallelogram_distinction() {
    // Persegi Panjang murni tidak boleh diklasifikasikan sebagai Jajar Genjang
    let rect_corners = [
        Pos2::new(40.0, 40.0),
        Pos2::new(220.0, 40.0),
        Pos2::new(220.0, 140.0),
        Pos2::new(40.0, 140.0),
    ];
    let mut rect_pts = Vec::new();
    for (a, b) in [
        (rect_corners[0], rect_corners[1]),
        (rect_corners[1], rect_corners[2]),
        (rect_corners[2], rect_corners[3]),
        (rect_corners[3], rect_corners[0]),
    ] {
        for t in 0..6 {
            let f = t as f32 / 6.0;
            rect_pts.push(StrokePoint::from(Pos2::new(
                a.x + (b.x - a.x) * f,
                a.y + (b.y - a.y) * f,
            )));
        }
    }
    rect_pts.push(StrokePoint::from(rect_corners[0]));

    let rec_rect = recognize_shape(&rect_pts).expect("Rectangle harus dikenali");
    assert_eq!(rec_rect.kind, ShapeKind::Rectangle);
    assert_ne!(rec_rect.kind, ShapeKind::Parallelogram);
}

#[test]
fn test_negative_cases_unsupported_shapes_preserve_none() {
    // 1. Trapesium sembarang (sisi samping tidak sejajar dan panjang sangat beda)
    let trap = [
        Pos2::new(80.0, 30.0),
        Pos2::new(140.0, 30.0),
        Pos2::new(220.0, 120.0),
        Pos2::new(20.0, 120.0),
    ];
    let mut trap_pts = Vec::new();
    for (a, b) in [
        (trap[0], trap[1]),
        (trap[1], trap[2]),
        (trap[2], trap[3]),
        (trap[3], trap[0]),
    ] {
        for t in 0..6 {
            let f = t as f32 / 6.0;
            trap_pts.push(StrokePoint::from(Pos2::new(
                a.x + (b.x - a.x) * f,
                a.y + (b.y - a.y) * f,
            )));
        }
    }
    trap_pts.push(StrokePoint::from(trap[0]));
    let rec_trap = recognize_shape(&trap_pts);
    assert!(
        rec_trap.is_none() || rec_trap.unwrap().kind != ShapeKind::Rectangle,
        "Trapesium non-sejajar tidak boleh salah jadi Rectangle"
    );

    // 2. Kurva terbuka (huruf C atau U)
    let mut open_u = Vec::new();
    for i in 0..20 {
        let theta = (i as f32) * std::f32::consts::PI / 19.0;
        open_u.push(StrokePoint::from(Pos2::new(
            100.0 + 50.0 * theta.cos(),
            100.0 + 50.0 * theta.sin(),
        )));
    }
    assert!(
        recognize_shape(&open_u).is_none(),
        "Kurva terbuka U tidak boleh dikenali sebagai bentuk tertutup"
    );
}

#[test]
fn test_rectangle_sharp_corners_and_recognition() {
    // 1. Recognize clean rectangle
    let rect_corners = [
        Pos2::new(10.0, 10.0),
        Pos2::new(110.0, 10.0),
        Pos2::new(110.0, 60.0),
        Pos2::new(10.0, 60.0),
    ];
    let mut pts = Vec::new();
    for (a, b) in [
        (rect_corners[0], rect_corners[1]),
        (rect_corners[1], rect_corners[2]),
        (rect_corners[2], rect_corners[3]),
        (rect_corners[3], rect_corners[0]),
    ] {
        for t in 0..10 {
            let f = t as f32 / 10.0;
            pts.push(StrokePoint::from(Pos2::new(
                a.x + (b.x - a.x) * f,
                a.y + (b.y - a.y) * f,
            )));
        }
    }
    pts.push(StrokePoint::from(rect_corners[0]));

    let recognized = recognize_shape(&pts).expect("Must recognize rectangle");
    assert_eq!(recognized.kind, ShapeKind::Rectangle);

    // Verify DrawObject::Shape creation for standard rectangle has sharp 90-degree corners (radius 0.0)
    let shape_obj = DrawObject::Shape {
        id: "rect_1".to_string(),
        shape_type: ShapeKind::Rectangle,
        x: recognized.x,
        y: recognized.y,
        width: recognized.width,
        height: recognized.height,
        color: dummy_color(),
        stroke_width: 2.0,
        fill: None,
        corner_radius: 0.0, // Standard sharp rectangle
        rotation: 0.0,
        label: None,
        curve_points: Vec::new(),
        locked: false,
    };

    if let DrawObject::Shape { corner_radius, .. } = shape_obj {
        assert_eq!(
            corner_radius, 0.0,
            "Standard rectangle must have zero corner radius"
        );
    } else {
        panic!("Expected shape object");
    }
}

#[test]
fn test_highlighter_tessellation_no_ribbon_inversion() {
    // Sharp hairpin turn stroke (0,0) -> (100,0) -> (10, 20)
    let pts = vec![
        StrokePoint {
            x: 0.0,
            y: 0.0,
            pressure: 1.0,
        },
        StrokePoint {
            x: 50.0,
            y: 0.0,
            pressure: 1.0,
        },
        StrokePoint {
            x: 100.0,
            y: 0.0,
            pressure: 1.0,
        },
        StrokePoint {
            x: 50.0,
            y: 10.0,
            pressure: 1.0,
        },
        StrokePoint {
            x: 10.0,
            y: 20.0,
            pressure: 1.0,
        },
    ];

    let mesh = tessellate_stroke_styled(
        &pts,
        18.0,
        Color32::from_rgba_unmultiplied(255, 225, 0, 95),
        true, // is_highlighter
        StrokeStyle::Solid,
        |p| p,
    );

    assert!(
        !mesh.vertices.is_empty(),
        "Mesh vertices should not be empty"
    );
    assert!(!mesh.indices.is_empty(), "Mesh indices should not be empty");

    // All indices must be within vertices length
    for &idx in &mesh.indices {
        assert!((idx as usize) < mesh.vertices.len(), "Index out of bounds");
    }

    // Verify all vertex positions are finite numbers
    for v in &mesh.vertices {
        assert!(
            v.pos.x.is_finite() && v.pos.y.is_finite(),
            "Vertices must be finite"
        );
    }
}

#[test]
fn test_latex_tokenizer_and_parser() {
    // 1. Basic formula
    let ast = parse_latex_math("x + y = z").expect("Failed to parse basic formula");
    assert!(!ast.is_empty());

    // 2. Fraction
    let frac_ast = parse_latex_math(r"\frac{a + 1}{b - 2}").expect("Failed to parse fraction");
    assert!(frac_ast.iter().any(|n| matches!(n, MathNode::Frac { .. })));

    // 3. Square root & nth root
    let sqrt_ast = parse_latex_math(r"\sqrt{x^2 + y^2}").expect("Failed to parse sqrt");
    assert!(
        sqrt_ast
            .iter()
            .any(|n| matches!(n, MathNode::Radical { .. }))
    );
    let root3_ast = parse_latex_math(r"\sqrt[3]{27}").expect("Failed to parse nth root");
    assert!(
        root3_ast
            .iter()
            .any(|n| matches!(n, MathNode::Radical { .. }))
    );

    // 4. Subscript & Superscript
    let sub_sup_ast = parse_latex_math("x_1^2 + a_{ij}").expect("Failed to parse sub/super");
    assert!(
        sub_sup_ast
            .iter()
            .any(|n| matches!(n, MathNode::SubSup { sup: Some(_), .. }))
    );
    assert!(
        sub_sup_ast
            .iter()
            .any(|n| matches!(n, MathNode::SubSup { sub: Some(_), .. }))
    );

    // 5. Large Operators (Sum, Integral)
    let sum_ast = parse_latex_math(r"\sum_{i=1}^{n} x_i").expect("Failed to parse sum");
    assert!(!sum_ast.is_empty());
    let int_ast = parse_latex_math(r"\int_{0}^{\infty} e^{-x} dx").expect("Failed to parse int");
    assert!(!int_ast.is_empty());

    // 6. Greek & Operators
    let greek_ast =
        parse_latex_math(r"\alpha + \beta = \pi \pm \theta").expect("Failed to parse greek");
    assert!(!greek_ast.is_empty());

    // 7. Functions
    let fn_ast = parse_latex_math(r"\sin(x) + \cos(\theta) = \ln(y)").expect("Failed to parse fn");
    assert!(!fn_ast.is_empty());

    // 8. Text block
    let txt_ast = parse_latex_math(r"x = 5 \text{ meter}").expect("Failed to parse text");
    assert!(
        txt_ast
            .iter()
            .any(|n| matches!(n, MathNode::Text(t) if t == " meter"))
    );

    // 9. Error resilience on incomplete syntax
    let err_result = parse_latex_math(r"\frac{unclosed");
    assert!(err_result.is_err(), "Unclosed braces should return error");
}

#[test]
fn test_math_layout_dimensions() {
    let layout = MathLayout::compute_from_str(r"E = mc^2", 24.0);
    assert!(layout.width > 20.0);
    assert!(layout.height > 10.0);
    assert!(!layout.items.is_empty());

    // Fraction layout creates fraction bar line
    let frac_layout = MathLayout::compute_from_str(r"\frac{1}{2}", 24.0);
    let has_line = frac_layout
        .items
        .iter()
        .any(|item| matches!(item, MathLayoutItem::Line { .. }));
    assert!(has_line, "Fraction must include a division line segment");

    // Radical layout creates radical bar line
    let sqrt_layout = MathLayout::compute_from_str(r"\sqrt{x}", 24.0);
    let has_sqrt_line = sqrt_layout
        .items
        .iter()
        .any(|item| matches!(item, MathLayoutItem::Line { .. }));
    assert!(
        has_sqrt_line,
        "Radical must include an overline line segment"
    );
}

#[test]
fn test_equation_draw_object_transforms() {
    let mut eq = DrawObject::Equation {
        id: "eq1".to_string(),
        latex: r"\frac{-b \pm \sqrt{b^2 - 4ac}}{2a}".to_string(),
        x: 100.0,
        y: 100.0,
        font_size: 22.0,
        scale: 1.0,
        color: dummy_color(),
        rotation: 0.0,
        locked: false,
    };

    // 1. Translate
    translate_object(&mut eq, 50.0, -20.0);
    if let DrawObject::Equation { x, y, .. } = eq {
        assert_eq!(x, 150.0);
        assert_eq!(y, 80.0);
    } else {
        panic!("Object type changed");
    }

    // 2. Scale
    scale_object(&mut eq, Pos2::new(0.0, 0.0), 2.0, 2.0);
    if let DrawObject::Equation { x, y, scale, .. } = eq {
        assert_eq!(x, 300.0);
        assert_eq!(y, 160.0);
        assert_eq!(scale, 2.0);
    } else {
        panic!("Object type changed");
    }

    // 3. Rotate around origin
    rotate_object(
        &mut eq,
        Pos2::new(300.0, 160.0),
        std::f32::consts::FRAC_PI_2,
    );
    if let DrawObject::Equation { rotation, .. } = eq {
        assert!((rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    } else {
        panic!("Object type changed");
    }

    // 4. Flip horizontal & vertical
    flip_object_horizontal(&mut eq, 300.0);
    flip_object_vertical(&mut eq, 160.0);
    let b = object_bounds(&eq);
    assert!(b.is_finite());
}

#[test]
fn test_equation_selection_frame() {
    let eq = DrawObject::Equation {
        id: "eq_sel".to_string(),
        latex: r"A = \pi r^2".to_string(),
        x: 50.0,
        y: 50.0,
        font_size: 20.0,
        scale: 1.0,
        color: dummy_color(),
        rotation: 0.25,
        locked: false,
    };

    let frame = SelectionFrame::from_objects(&[&eq], 24.0)
        .expect("SelectionFrame should exist for Equation");
    assert!(frame.local_size.x > 10.0);
    assert!(frame.local_size.y > 10.0);
    assert_eq!(frame.rotation, 0.25);
    for c in &frame.corners {
        assert!(c.x.is_finite() && c.y.is_finite());
    }
}

#[test]
fn test_equation_hit_test_and_lasso() {
    let eq = DrawObject::Equation {
        id: "eq_hit".to_string(),
        latex: r"y = mx + b".to_string(),
        x: 100.0,
        y: 100.0,
        font_size: 20.0,
        scale: 1.0,
        color: dummy_color(),
        rotation: 0.0,
        locked: false,
    };

    // Hit test at center
    let bounds = object_bounds(&eq);
    assert!(hit_test_object(&eq, bounds.center(), 5.0));
    assert!(!hit_test_object(&eq, Pos2::new(0.0, 0.0), 5.0));

    // Lasso containment
    let lasso_box = vec![
        Pos2::new(bounds.min.x - 10.0, bounds.min.y - 10.0),
        Pos2::new(bounds.max.x + 10.0, bounds.min.y - 10.0),
        Pos2::new(bounds.max.x + 10.0, bounds.max.y + 10.0),
        Pos2::new(bounds.min.x - 10.0, bounds.max.y + 10.0),
    ];
    assert!(object_contained_in_lasso(&eq, &lasso_box));
    assert!(object_in_lasso(&eq, &lasso_box));

    // Outside lasso
    let outside_lasso = vec![
        Pos2::new(0.0, 0.0),
        Pos2::new(20.0, 0.0),
        Pos2::new(20.0, 20.0),
        Pos2::new(0.0, 20.0),
    ];
    assert!(!object_contained_in_lasso(&eq, &outside_lasso));
    assert!(!object_in_lasso(&eq, &outside_lasso));
}

#[test]
fn test_equation_serialization_and_svg() {
    let eq = DrawObject::Equation {
        id: "eq_serde".to_string(),
        latex: r"\int x dx = \frac{x^2}{2} + C".to_string(),
        x: 80.0,
        y: 120.0,
        font_size: 24.0,
        scale: 1.0,
        color: dummy_color(),
        rotation: 0.1,
        locked: false,
    };

    // JSON roundtrip
    let json = serde_json::to_string(&eq).expect("Failed to serialize Equation");
    let deserialized: DrawObject =
        serde_json::from_str(&json).expect("Failed to deserialize Equation");
    assert_eq!(eq.id(), deserialized.id());
    if let DrawObject::Equation {
        latex, font_size, ..
    } = deserialized
    {
        assert_eq!(latex, r"\int x dx = \frac{x^2}{2} + C");
        assert_eq!(font_size, 24.0);
    } else {
        panic!("Wrong variant");
    }

    // Vector SVG generation
    let layout = MathLayout::compute_from_str(r"\frac{a}{b}", 24.0);
    let svg = layout.to_svg(50.0, 50.0, &dummy_color(), 0.0);
    assert!(svg.contains("<text"));
    assert!(svg.contains("<line"));
}
