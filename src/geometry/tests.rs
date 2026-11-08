use super::*;
use crate::model::{ColorData, DrawObject, ShapeKind, StrokePoint};
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
    };
    assert!(!object_in_lasso(&outside_stroke, &poly));
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
