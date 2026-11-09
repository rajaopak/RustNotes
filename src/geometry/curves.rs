use crate::model::Point2D;

/// Menghitung titik-titik keliling poligon Rounded Rectangle dengan mempertimbangkan rotasi.
///
/// Jika `corner_radius <= 0.0` atau `segments_per_corner == 0`, fungsi ini mengembalikan
/// 4 titik sudut persegi panjang. Radius dibatasi secara otomatis (clamped) agar tidak
/// melebihi setengah dari sisi terpendek (`min(width, height) / 2.0`).
pub fn rounded_rect_vertices(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: f32,
    rotation: f32,
    segments_per_corner: usize,
) -> Vec<Point2D> {
    let min_x = if width >= 0.0 { x } else { x + width };
    let min_y = if height >= 0.0 { y } else { y + height };
    let abs_w = width.abs();
    let abs_h = height.abs();
    let max_x = min_x + abs_w;
    let max_y = min_y + abs_h;

    let center = Point2D::new(min_x + abs_w * 0.5, min_y + abs_h * 0.5);

    let max_allowed_radius = (abs_w.min(abs_h) * 0.5).max(0.0);
    let r = corner_radius.clamp(0.0, max_allowed_radius);

    if r <= 1e-4 || segments_per_corner == 0 {
        let corners = [
            Point2D::new(min_x, min_y),
            Point2D::new(max_x, min_y),
            Point2D::new(max_x, max_y),
            Point2D::new(min_x, max_y),
        ];
        return corners
            .into_iter()
            .map(|pt| {
                if rotation.abs() > 1e-5 {
                    pt.rotate_around(center, rotation)
                } else {
                    pt
                }
            })
            .collect();
    }

    let mut vertices = Vec::with_capacity(segments_per_corner * 4 + 4);

    // 4 busur sudut: Top-Right, Bottom-Right, Bottom-Left, Top-Left
    let arc_centers = [
        (
            Point2D::new(max_x - r, min_y + r),
            -std::f32::consts::FRAC_PI_2,
            0.0,
        ), // TR
        (
            Point2D::new(max_x - r, max_y - r),
            0.0,
            std::f32::consts::FRAC_PI_2,
        ), // BR
        (
            Point2D::new(min_x + r, max_y - r),
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
        ), // BL
        (
            Point2D::new(min_x + r, min_y + r),
            std::f32::consts::PI,
            std::f32::consts::PI * 1.5,
        ), // TL
    ];

    let segs = segments_per_corner.max(1);
    for (arc_c, start_angle, end_angle) in arc_centers {
        for s in 0..=segs {
            let t = s as f32 / segs as f32;
            let angle = start_angle + (end_angle - start_angle) * t;
            let raw_pt = Point2D::new(arc_c.x + r * angle.cos(), arc_c.y + r * angle.sin());
            let final_pt = if rotation.abs() > 1e-5 {
                raw_pt.rotate_around(center, rotation)
            } else {
                raw_pt
            };
            vertices.push(final_pt);
        }
    }

    vertices
}

/// Menghitung sampel titik pada kurva Bézier kubik:
/// B(t) = (1-t)³ P0 + 3(1-t)²t P1 + 3(1-t)t² P2 + t³ P3
pub fn sample_cubic_bezier(
    p0: Point2D,
    p1: Point2D,
    p2: Point2D,
    p3: Point2D,
    steps: usize,
) -> Vec<Point2D> {
    let steps = steps.max(1);
    let mut points = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;

        let x = uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x;
        let y = uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y;
        points.push(Point2D::new(x, y));
    }
    points
}

/// Mengonversi rangkaian titik menjadi segmen kurva Bézier kubik via interpolasi Catmull-Rom.
/// Mengembalikan daftar segmen yang masing-masing terdiri dari `[p0, c1, c2, p3]`.
pub fn fit_catmull_rom_to_bezier(pts: &[Point2D]) -> Vec<[Point2D; 4]> {
    if pts.len() < 2 {
        return Vec::new();
    }
    if pts.len() == 2 {
        let p0 = pts[0];
        let p3 = pts[1];
        let c1 = p0 + (p3 - p0) * (1.0 / 3.0);
        let c2 = p0 + (p3 - p0) * (2.0 / 3.0);
        return vec![[p0, c1, c2, p3]];
    }

    let mut segments = Vec::with_capacity(pts.len() - 1);
    let n = pts.len();

    for i in 0..n - 1 {
        let p0 = if i == 0 {
            pts[0] - (pts[1] - pts[0])
        } else {
            pts[i - 1]
        };
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = if i + 2 < n {
            pts[i + 2]
        } else {
            pts[i + 1] + (pts[i + 1] - pts[i])
        };

        let c1 = p1 + (p2 - p0) * (1.0 / 6.0);
        let c2 = p2 - (p3 - p1) * (1.0 / 6.0);

        segments.push([p1, c1, c2, p2]);
    }

    segments
}

/// Algoritma penyederhanaan poligon Ramer-Douglas-Peucker (RDP)
/// untuk mereduksi noise coretan tangan sebelum fitting kurva.
pub fn simplify_points_rdp(points: &[Point2D], epsilon: f32) -> Vec<Point2D> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut dmax = 0.0f32;
    let mut index = 0;
    let start = points[0];
    let end = points[points.len() - 1];

    let last_idx = points.len() - 1;
    for (i, &pt) in points.iter().enumerate().take(last_idx).skip(1) {
        let d = point_to_line_dist(pt, start, end);
        if d > dmax {
            index = i;
            dmax = d;
        }
    }

    if dmax > epsilon {
        let left = simplify_points_rdp(&points[0..=index], epsilon);
        let right = simplify_points_rdp(&points[index..], epsilon);

        let mut result = left;
        result.pop(); // Hindari titik duplikat di sambungan
        result.extend(right);
        result
    } else {
        vec![start, end]
    }
}

/// Jarak tegak lurus dari titik P ke garis yang melalui A dan B.
fn point_to_line_dist(p: Point2D, a: Point2D, b: Point2D) -> f32 {
    let ab = b - a;
    let len_sq = ab.dist_sq(Point2D::new(0.0, 0.0));
    if len_sq < 1e-6 {
        return p.dist(a);
    }
    let t = ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len_sq;
    let proj = Point2D::new(a.x + ab.x * t, a.y + ab.y * t);
    p.dist(proj)
}
