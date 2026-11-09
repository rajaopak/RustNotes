use crate::model::{ConnectionSite, DrawObject, Point2D, ShapeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Right,
    Down,
    Left,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::Up,
        Direction::Right,
        Direction::Down,
        Direction::Left,
    ];

    pub fn to_vector(self) -> Point2D {
        match self {
            Self::Up => Point2D::new(0.0, -1.0),
            Self::Right => Point2D::new(1.0, 0.0),
            Self::Down => Point2D::new(0.0, 1.0),
            Self::Left => Point2D::new(-1.0, 0.0),
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Right => Self::Left,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
        }
    }

    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }

    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }

    pub fn from_vector(v: Point2D) -> Self {
        if v.x.abs() > v.y.abs() {
            if v.x >= 0.0 { Self::Right } else { Self::Left }
        } else if v.y >= 0.0 {
            Self::Down
        } else {
            Self::Up
        }
    }
}

/// Menghitung titik sambung (anchor point) dan arah normal keluar dari bentuk yang mungkin berotasi.
///
/// Jika `site == ConnectionSite::Auto`, fungsi ini akan memilih sisi (`Top`, `Right`, `Bottom`, `Left`)
/// yang memiliki jarak terdekat dengan `target_hint` (A8).
pub fn shape_connection_point(
    shape: &DrawObject,
    site: ConnectionSite,
    target_hint: Option<Point2D>,
) -> (Point2D, Direction) {
    let (x, y, width, height, rotation, shape_type) = match shape {
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            rotation,
            shape_type,
            ..
        } => (*x, *y, *width, *height, *rotation, *shape_type),
        _ => return (Point2D::new(0.0, 0.0), Direction::Right),
    };

    let min_x = if width >= 0.0 { x } else { x + width };
    let min_y = if height >= 0.0 { y } else { y + height };
    let abs_w = width.abs();
    let abs_h = height.abs();
    let max_x = min_x + abs_w;
    let max_y = min_y + abs_h;
    let center = Point2D::new(min_x + abs_w * 0.5, min_y + abs_h * 0.5);

    let get_site_local = |s: ConnectionSite| -> (Point2D, Direction) {
        match shape_type {
            ShapeKind::Line => {
                let start_pt = Point2D::new(x, y);
                let end_pt = Point2D::new(x + width, y + height);
                let mid_pt =
                    Point2D::new((start_pt.x + end_pt.x) * 0.5, (start_pt.y + end_pt.y) * 0.5);
                match s {
                    ConnectionSite::Top | ConnectionSite::Left => (start_pt, Direction::Left),
                    ConnectionSite::Bottom | ConnectionSite::Right => (end_pt, Direction::Right),
                    ConnectionSite::Auto => (mid_pt, Direction::Right),
                }
            }
            ShapeKind::Diamond => match s {
                ConnectionSite::Top => (Point2D::new(center.x, min_y), Direction::Up),
                ConnectionSite::Right => (Point2D::new(max_x, center.y), Direction::Right),
                ConnectionSite::Bottom => (Point2D::new(center.x, max_y), Direction::Down),
                ConnectionSite::Left => (Point2D::new(min_x, center.y), Direction::Left),
                ConnectionSite::Auto => (Point2D::new(max_x, center.y), Direction::Right),
            },
            ShapeKind::Triangle => match s {
                ConnectionSite::Top => (Point2D::new(center.x, min_y), Direction::Up),
                ConnectionSite::Right => (
                    Point2D::new((center.x + max_x) * 0.5, (min_y + max_y) * 0.5),
                    Direction::Right,
                ),
                ConnectionSite::Bottom => (Point2D::new(center.x, max_y), Direction::Down),
                ConnectionSite::Left => (
                    Point2D::new((min_x + center.x) * 0.5, (min_y + max_y) * 0.5),
                    Direction::Left,
                ),
                ConnectionSite::Auto => (Point2D::new(center.x, min_y), Direction::Up),
            },
            _ => match s {
                ConnectionSite::Top => (Point2D::new(center.x, min_y), Direction::Up),
                ConnectionSite::Right => (Point2D::new(max_x, center.y), Direction::Right),
                ConnectionSite::Bottom => (Point2D::new(center.x, max_y), Direction::Down),
                ConnectionSite::Left => (Point2D::new(min_x, center.y), Direction::Left),
                ConnectionSite::Auto => (Point2D::new(max_x, center.y), Direction::Right),
            },
        }
    };

    let rotate_anchor = |(pt, dir): (Point2D, Direction)| -> (Point2D, Direction) {
        if rotation.abs() <= 1e-5 {
            (pt, dir)
        } else {
            let rot_pt = pt.rotate_around(center, rotation);
            let raw_dir_vec = dir
                .to_vector()
                .rotate_around(Point2D::new(0.0, 0.0), rotation);
            (rot_pt, Direction::from_vector(raw_dir_vec))
        }
    };

    if site == ConnectionSite::Auto {
        let candidates = [
            rotate_anchor(get_site_local(ConnectionSite::Top)),
            rotate_anchor(get_site_local(ConnectionSite::Right)),
            rotate_anchor(get_site_local(ConnectionSite::Bottom)),
            rotate_anchor(get_site_local(ConnectionSite::Left)),
        ];

        if let Some(target) = target_hint {
            let mut best = candidates[0];
            let mut best_dist = candidates[0].0.dist_sq(target);
            for candidate in candidates.iter().skip(1) {
                let d = candidate.0.dist_sq(target);
                if d < best_dist {
                    best_dist = d;
                    best = *candidate;
                }
            }
            best
        } else {
            candidates[1] // Default Right
        }
    } else {
        rotate_anchor(get_site_local(site))
    }
}

/// Jalur garis lurus sederhana dari start ke end.
pub fn route_straight(start: Point2D, end: Point2D) -> Vec<Point2D> {
    vec![start, end]
}

/// Manhattan (Orthogonal) routing dengan clearance dari bentuk.
///
/// Seluruh segmen dijamin sejajar sumbu X atau Y (tidak ada garis miring diagonal).
/// Clearance stub memastikan jalur keluar tegak lurus dari tepi bentuk sebelum berbelok.
pub fn route_orthogonal(
    start: Point2D,
    start_dir: Direction,
    end: Point2D,
    end_dir: Direction,
    clearance: f32,
) -> Vec<Point2D> {
    let cl = clearance.max(12.0);
    let p_start_stub = start + start_dir.to_vector() * cl;
    let p_end_stub = end + end_dir.to_vector() * cl;

    let mut path = vec![start, p_start_stub];

    // Hubungkan p_start_stub ke p_end_stub ortogonal
    if start_dir.is_horizontal() && end_dir.is_horizontal() {
        if (start_dir == Direction::Right && p_start_stub.x <= p_end_stub.x)
            || (start_dir == Direction::Left && p_start_stub.x >= p_end_stub.x)
        {
            let mid_x = (p_start_stub.x + p_end_stub.x) * 0.5;
            path.push(Point2D::new(mid_x, p_start_stub.y));
            path.push(Point2D::new(mid_x, p_end_stub.y));
        } else {
            let mid_y = (p_start_stub.y + p_end_stub.y) * 0.5;
            path.push(Point2D::new(p_start_stub.x, mid_y));
            path.push(Point2D::new(p_end_stub.x, mid_y));
        }
    } else if start_dir.is_vertical() && end_dir.is_vertical() {
        if (start_dir == Direction::Down && p_start_stub.y <= p_end_stub.y)
            || (start_dir == Direction::Up && p_start_stub.y >= p_end_stub.y)
        {
            let mid_y = (p_start_stub.y + p_end_stub.y) * 0.5;
            path.push(Point2D::new(p_start_stub.x, mid_y));
            path.push(Point2D::new(p_end_stub.x, mid_y));
        } else {
            let mid_x = (p_start_stub.x + p_end_stub.x) * 0.5;
            path.push(Point2D::new(mid_x, p_start_stub.y));
            path.push(Point2D::new(mid_x, p_end_stub.y));
        }
    } else if start_dir.is_horizontal() {
        // Start horizontal, End vertical
        path.push(Point2D::new(p_end_stub.x, p_start_stub.y));
    } else {
        // Start vertical, End horizontal
        path.push(Point2D::new(p_start_stub.x, p_end_stub.y));
    }

    path.push(p_end_stub);
    path.push(end);

    clean_collinear_points(&path)
}

/// Menghapus titik-titik redundant yang berada pada satu garis lurus yang sama.
pub fn clean_collinear_points(points: &[Point2D]) -> Vec<Point2D> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let mut cleaned = Vec::with_capacity(points.len());
    cleaned.push(points[0]);

    for i in 1..points.len() - 1 {
        let p_prev = cleaned[cleaned.len() - 1];
        let p_curr = points[i];
        let p_next = points[i + 1];

        // Jika jarak sangat dekat, lewati
        if p_curr.dist_sq(p_prev) < 1e-4 {
            continue;
        }

        // Cek kolinearitas horizontal atau vertikal
        let is_collinear_h =
            (p_prev.y - p_curr.y).abs() < 1e-4 && (p_curr.y - p_next.y).abs() < 1e-4;
        let is_collinear_v =
            (p_prev.x - p_curr.x).abs() < 1e-4 && (p_curr.x - p_next.x).abs() < 1e-4;

        if !is_collinear_h && !is_collinear_v {
            cleaned.push(p_curr);
        }
    }

    cleaned.push(points[points.len() - 1]);
    cleaned
}

/// Menghitung titik kontrol kurva Bézier halus untuk connector melengkung.
/// Mengembalikan 4 titik: `[start, c1, c2, end]`.
pub fn route_curved(
    start: Point2D,
    start_dir: Direction,
    end: Point2D,
    end_dir: Direction,
) -> [Point2D; 4] {
    let d = (start.dist(end) * 0.45).clamp(24.0, 300.0);
    let c1 = start + start_dir.to_vector() * d;
    let c2 = end + end_dir.to_vector() * d;
    [start, c1, c2, end]
}

/// Menghitung kurva Catmull-Rom spline yang melewati seluruh titik waypoints secara mulus (Task 8.4).
pub fn catmull_rom_spline(points: &[Point2D], samples_per_segment: usize) -> Vec<Point2D> {
    if points.len() <= 1 {
        return points.to_vec();
    }
    if points.len() == 2 {
        let p0 = points[0];
        let p1 = points[1];
        let samples = samples_per_segment.max(4);
        let mut res = Vec::with_capacity(samples + 1);
        for s in 0..=samples {
            let t = s as f32 / samples as f32;
            res.push(Point2D::new(
                p0.x + (p1.x - p0.x) * t,
                p0.y + (p1.y - p0.y) * t,
            ));
        }
        return res;
    }

    let n = points.len();
    let mut spline = Vec::new();

    for i in 0..n - 1 {
        let p0 = if i == 0 {
            points[0] - (points[1] - points[0])
        } else {
            points[i - 1]
        };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n {
            points[i + 2]
        } else {
            points[n - 1] + (points[n - 1] - points[n - 2])
        };

        // Tangent Catmull-Rom
        let t1 = (p2 - p0) * 0.5;
        let t2 = (p3 - p1) * 0.5;

        // Cubic Bézier control points
        let b0 = p1;
        let b1 = p1 + t1 * (1.0 / 3.0);
        let b2 = p2 - t2 * (1.0 / 3.0);
        let b3 = p2;

        let sampled =
            crate::geometry::sample_cubic_bezier(b0, b1, b2, b3, samples_per_segment.max(4));
        if i > 0 && !spline.is_empty() {
            spline.extend_from_slice(&sampled[1..]);
        } else {
            spline.extend_from_slice(&sampled);
        }
    }

    spline
}

/// Menghitung jalur orthogonal yang melewati titik-titik kontrol manual dengan tetap menjaga
/// setiap segmen tegak lurus horizontal atau vertikal (Task 8.4).
pub fn route_orthogonal_through_points(
    start: Point2D,
    start_dir: Direction,
    end: Point2D,
    end_dir: Direction,
    control_points: &[Point2D],
) -> Vec<Point2D> {
    if control_points.is_empty() {
        return route_orthogonal(start, start_dir, end, end_dir.opposite(), 16.0);
    }

    let mut waypoints = Vec::with_capacity(control_points.len() + 2);
    waypoints.push(start);
    waypoints.extend_from_slice(control_points);
    waypoints.push(end);

    let mut path = Vec::new();
    path.push(waypoints[0]);

    let mut current_dir = start_dir;

    for i in 0..waypoints.len() - 1 {
        let p_curr = path[path.len() - 1];
        let p_target = waypoints[i + 1];

        let dx = (p_target.x - p_curr.x).abs();
        let dy = (p_target.y - p_curr.y).abs();

        if dx < 1.0 && dy < 1.0 {
            continue;
        }

        if dx < 1.0 {
            path.push(Point2D::new(p_curr.x, p_target.y));
            current_dir = if p_target.y >= p_curr.y {
                Direction::Down
            } else {
                Direction::Up
            };
        } else if dy < 1.0 {
            path.push(Point2D::new(p_target.x, p_curr.y));
            current_dir = if p_target.x >= p_curr.x {
                Direction::Right
            } else {
                Direction::Left
            };
        } else if current_dir.is_horizontal() {
            path.push(Point2D::new(p_target.x, p_curr.y));
            path.push(p_target);
            current_dir = if p_target.y >= p_curr.y {
                Direction::Down
            } else {
                Direction::Up
            };
        } else {
            path.push(Point2D::new(p_curr.x, p_target.y));
            path.push(p_target);
            current_dir = if p_target.x >= p_curr.x {
                Direction::Right
            } else {
                Direction::Left
            };
        }
    }

    clean_collinear_points(&path)
}

/// Menambahkan titik kontrol baru pada segmen terdekat dari connector (Task 8.2).
/// Mengembalikan indeks posisi baru di dalam `control_points`.
pub fn insert_control_point_at(
    control_points: &mut Vec<Point2D>,
    start: Point2D,
    end: Point2D,
    new_pt: Point2D,
) -> usize {
    let mut pts = Vec::with_capacity(control_points.len() + 2);
    pts.push(start);
    pts.extend_from_slice(control_points);
    pts.push(end);

    let mut best_seg = 0;
    let mut best_dist = f32::MAX;

    for i in 0..pts.len() - 1 {
        let a = pts[i];
        let b = pts[i + 1];
        let ab = b - a;
        let len_sq = ab.dist_sq(Point2D::new(0.0, 0.0));
        let dist = if len_sq < 1e-6 {
            new_pt.dist(a)
        } else {
            let t = (((new_pt.x - a.x) * ab.x + (new_pt.y - a.y) * ab.y) / len_sq).clamp(0.0, 1.0);
            let proj = Point2D::new(a.x + ab.x * t, a.y + ab.y * t);
            new_pt.dist(proj)
        };
        if dist < best_dist {
            best_dist = dist;
            best_seg = i;
        }
    }

    let insert_idx = best_seg.min(control_points.len());
    control_points.insert(insert_idx, new_pt);
    insert_idx
}

/// Menghapus titik kontrol yang hampir segaris lurus dengan tetangganya (Task 8.3).
pub fn prune_collinear_control_points(
    control_points: &mut Vec<Point2D>,
    start: Point2D,
    end: Point2D,
    tolerance: f32,
) -> bool {
    if control_points.is_empty() {
        return false;
    }
    let tol = tolerance.max(4.0);
    let mut changed = false;
    let mut i = 0;
    while i < control_points.len() {
        let prev = if i == 0 { start } else { control_points[i - 1] };
        let next = if i + 1 == control_points.len() {
            end
        } else {
            control_points[i + 1]
        };
        let curr = control_points[i];

        let ab = next - prev;
        let len_sq = ab.dist_sq(Point2D::new(0.0, 0.0));
        let dist = if len_sq < 1e-6 {
            curr.dist(prev)
        } else {
            let t =
                (((curr.x - prev.x) * ab.x + (curr.y - prev.y) * ab.y) / len_sq).clamp(0.0, 1.0);
            let proj = Point2D::new(prev.x + ab.x * t, prev.y + ab.y * t);
            curr.dist(proj)
        };

        if dist <= tol {
            control_points.remove(i);
            changed = true;
        } else {
            i += 1;
        }
    }
    changed
}

/// Memeriksa dan membuang titik kontrol yang berada di dalam bentuk terhubung saat bentuk bergerak (Task 8.5).
pub fn prune_invalid_connector_control_points(
    connector: &mut DrawObject,
    all_objects: &[DrawObject],
) -> bool {
    if let DrawObject::Connector {
        start,
        end,
        control_points,
        ..
    } = connector
    {
        if control_points.is_empty() {
            return false;
        }

        let mut attached_shapes: Vec<&DrawObject> = Vec::new();
        if let crate::model::ConnectorEnd::Attached { shape_id, .. } = start
            && let Some(s) = all_objects.iter().find(|o| o.id() == shape_id)
        {
            attached_shapes.push(s);
        }
        if let crate::model::ConnectorEnd::Attached { shape_id, .. } = end
            && let Some(s) = all_objects.iter().find(|o| o.id() == shape_id)
        {
            attached_shapes.push(s);
        }

        if attached_shapes.is_empty() {
            return false;
        }

        let initial_len = control_points.len();
        control_points.retain(|cp| {
            for shape in &attached_shapes {
                if hit_test_rotated_shape(*cp, shape, 0.0) {
                    return false;
                }
            }
            true
        });

        initial_len != control_points.len()
    } else {
        false
    }
}

/// Menguji apakah titik `p` mendekati titik tengah salah satu segmen jalur connector
/// untuk menambahkan tikungan baru (Task 8.2).
pub fn hit_test_connector_midpoint(
    p: Point2D,
    poly: &[Point2D],
    tolerance: f32,
) -> Option<(usize, Point2D)> {
    if poly.len() < 2 {
        return None;
    }
    let handle_radius = (tolerance + 4.0).max(7.0);
    for i in 0..poly.len() - 1 {
        let mid = (poly[i] + poly[i + 1]) * 0.5;
        if p.dist(mid) <= handle_radius {
            return Some((i, mid));
        }
    }
    None
}

/// Menghasilkan 3 titik poligon panah simetris `[tip, left_wing, right_wing]`
/// berdasarkan arah datang garis ke titik tujuan.
pub fn arrowhead_geometry(
    tip: Point2D,
    incoming_dir: Point2D,
    arrow_len: f32,
    arrow_width: f32,
) -> [Point2D; 3] {
    let d = incoming_dir.normalized().unwrap_or(Point2D::new(1.0, 0.0));
    let perp = Point2D::new(-d.y, d.x);
    let base = tip - d * arrow_len;
    let left = base + perp * (arrow_width * 0.5);
    let right = base - perp * (arrow_width * 0.5);
    [tip, left, right]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectorHitTarget {
    StartHandle,
    EndHandle,
    ControlPoint(usize),
    LineSegment(usize),
}

/// Hit-testing rinci pada elemen connector untuk manipulasi interaktif.
pub fn hit_test_connector_details(
    p: Point2D,
    points: &[Point2D],
    tolerance: f32,
) -> Option<ConnectorHitTarget> {
    if points.is_empty() {
        return None;
    }

    let handle_radius = (tolerance + 5.0).max(8.0);
    let line_tol = (tolerance + 2.0).max(4.0);

    // 1. Cek start handle
    if p.dist(points[0]) <= handle_radius {
        return Some(ConnectorHitTarget::StartHandle);
    }

    // 2. Cek end handle
    let last_idx = points.len() - 1;
    if p.dist(points[last_idx]) <= handle_radius {
        return Some(ConnectorHitTarget::EndHandle);
    }

    // 3. Cek intermediate control points (tikungan)
    for (i, &cp) in points.iter().enumerate().take(last_idx).skip(1) {
        if p.dist(cp) <= handle_radius {
            return Some(ConnectorHitTarget::ControlPoint(i));
        }
    }

    // 4. Cek segmen garis
    for i in 0..points.len() - 1 {
        let a = points[i];
        let b = points[i + 1];
        let ab = b - a;
        let len_sq = ab.dist_sq(Point2D::new(0.0, 0.0));
        let dist = if len_sq < 1e-6 {
            p.dist(a)
        } else {
            let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len_sq).clamp(0.0, 1.0);
            let proj = Point2D::new(a.x + ab.x * t, a.y + ab.y * t);
            p.dist(proj)
        };

        if dist <= line_tol {
            return Some(ConnectorHitTarget::LineSegment(i));
        }
    }

    None
}

/// Menghitung Axis-Aligned Bounding Box (AABB) dari persegi panjang yang berotasi.
/// Mengembalikan `(min_pt, max_pt)`.
pub fn rotated_rect_bounds(
    center: Point2D,
    width: f32,
    height: f32,
    rotation: f32,
) -> (Point2D, Point2D) {
    let hw = width.abs() * 0.5;
    let hh = height.abs() * 0.5;

    let corners = [
        Point2D::new(-hw, -hh),
        Point2D::new(hw, -hh),
        Point2D::new(hw, hh),
        Point2D::new(-hw, hh),
    ];

    let cos_a = rotation.cos();
    let sin_a = rotation.sin();

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for c in corners {
        let rx = center.x + c.x * cos_a - c.y * sin_a;
        let ry = center.y + c.x * sin_a + c.y * cos_a;
        min_x = min_x.min(rx);
        min_y = min_y.min(ry);
        max_x = max_x.max(rx);
        max_y = max_y.max(ry);
    }

    (Point2D::new(min_x, min_y), Point2D::new(max_x, max_y))
}

/// Hit-testing akurat pada bentuk berotasi:
/// Mentransformasi titik uji `p` ke sistem koordinat lokal bentuk (invers rotasi),
/// lalu menguji apakah titik tersebut berada di dalam batas bentuk.
pub fn hit_test_rotated_shape(p: Point2D, shape: &DrawObject, tolerance: f32) -> bool {
    let (x, y, width, height, rotation) = match shape {
        DrawObject::Shape {
            x,
            y,
            width,
            height,
            rotation,
            ..
        } => (*x, *y, *width, *height, *rotation),
        _ => return false,
    };

    let min_x = if width >= 0.0 { x } else { x + width };
    let min_y = if height >= 0.0 { y } else { y + height };
    let abs_w = width.abs();
    let abs_h = height.abs();
    let center = Point2D::new(min_x + abs_w * 0.5, min_y + abs_h * 0.5);

    // Invers rotasi titik uji mengelilingi center
    let local_p = p.rotate_around(center, -rotation);

    let tol = tolerance.max(0.0);
    local_p.x >= min_x - tol
        && local_p.x <= min_x + abs_w + tol
        && local_p.y >= min_y - tol
        && local_p.y <= min_y + abs_h + tol
}

/// Menghitung koordinat titik awal dan titik akhir serta arah normal connector
/// berdasarkan daftar seluruh objek pada halaman (Task 7.1).
pub fn resolve_connector_endpoints(
    start: &crate::model::ConnectorEnd,
    end: &crate::model::ConnectorEnd,
    all_objects: &[DrawObject],
) -> ((Point2D, Direction), (Point2D, Direction)) {
    let (start_pt, start_dir) = match start {
        crate::model::ConnectorEnd::Attached { shape_id, site } => {
            if let Some(shape) = all_objects.iter().find(|o| o.id() == shape_id) {
                shape_connection_point(shape, *site, None)
            } else {
                (Point2D::new(50.0, 50.0), Direction::Right)
            }
        }
        crate::model::ConnectorEnd::Free { x, y } => (Point2D::new(*x, *y), Direction::Right),
    };

    let (end_pt, end_dir) = match end {
        crate::model::ConnectorEnd::Attached { shape_id, site } => {
            if let Some(shape) = all_objects.iter().find(|o| o.id() == shape_id) {
                shape_connection_point(shape, *site, Some(start_pt))
            } else {
                (Point2D::new(150.0, 150.0), Direction::Left)
            }
        }
        crate::model::ConnectorEnd::Free { x, y } => (Point2D::new(*x, *y), Direction::Left),
    };

    ((start_pt, start_dir), (end_pt, end_dir))
}

/// Menghitung seluruh titik jalur (polyline) connector dari start hingga end (Task 7.1, 7.4, 8.4).
pub fn calculate_connector_path(
    start_pt: Point2D,
    start_dir: Direction,
    end_pt: Point2D,
    end_dir: Direction,
    route: crate::model::ConnectorRoute,
    control_points: &[Point2D],
) -> Vec<Point2D> {
    if !control_points.is_empty() {
        match route {
            crate::model::ConnectorRoute::Straight => {
                let mut full = Vec::with_capacity(control_points.len() + 2);
                full.push(start_pt);
                full.extend_from_slice(control_points);
                full.push(end_pt);
                clean_collinear_points(&full)
            }
            crate::model::ConnectorRoute::Orthogonal => route_orthogonal_through_points(
                start_pt,
                start_dir,
                end_pt,
                end_dir,
                control_points,
            ),
            crate::model::ConnectorRoute::Curved => {
                let mut full = Vec::with_capacity(control_points.len() + 2);
                full.push(start_pt);
                full.extend_from_slice(control_points);
                full.push(end_pt);
                catmull_rom_spline(&full, 16)
            }
        }
    } else {
        match route {
            crate::model::ConnectorRoute::Straight => route_straight(start_pt, end_pt),
            crate::model::ConnectorRoute::Orthogonal => {
                route_orthogonal(start_pt, start_dir, end_pt, end_dir.opposite(), 16.0)
            }
            crate::model::ConnectorRoute::Curved => {
                let ctrl = route_curved(start_pt, start_dir, end_pt, end_dir.opposite());
                crate::geometry::sample_cubic_bezier(ctrl[0], ctrl[1], ctrl[2], ctrl[3], 24)
            }
        }
    }
}

/// Mencari kandidat titik sambung terdekat dari seluruh bentuk di halaman dalam radius `snap_radius` (Task 7.1).
/// Mengembalikan `(shape_id, ConnectionSite, visual_snap_point)`.
pub fn find_connector_snap_site(
    objects: &[DrawObject],
    p: Point2D,
    snap_radius: f32,
    exclude_shape_id: Option<&str>,
) -> Option<(String, ConnectionSite, Point2D)> {
    let mut best: Option<(String, ConnectionSite, Point2D, f32)> = None;
    for obj in objects {
        if let DrawObject::Shape { id, .. } = obj {
            if exclude_shape_id == Some(id) {
                continue;
            }
            for &site in &[
                ConnectionSite::Top,
                ConnectionSite::Right,
                ConnectionSite::Bottom,
                ConnectionSite::Left,
            ] {
                let (anchor, _) = shape_connection_point(obj, site, None);
                let d = p.dist(anchor);
                if d <= snap_radius {
                    let is_better = best.as_ref().is_none_or(|b| d < b.3);
                    if is_better {
                        best = Some((id.clone(), site, anchor, d));
                    }
                }
            }
        }
    }
    best.map(|(id, site, anchor, _)| (id, site, anchor))
}

/// Melepaskan connector yang terikat pada bentuk yang dihapus,
/// mengubah Attached menjadi Free di koordinat terakhir titik sambung (Task 7.5, Asumsi A5).
pub fn detach_connectors_from_shapes(objects: &mut [DrawObject], deleted_shape_ids: &[String]) {
    if deleted_shape_ids.is_empty() {
        return;
    }
    let mut resolved_positions: std::collections::HashMap<(String, ConnectionSite), Point2D> =
        std::collections::HashMap::new();
    for obj in objects.iter() {
        if let DrawObject::Shape { id, .. } = obj
            && deleted_shape_ids.contains(id)
        {
            for &site in &[
                ConnectionSite::Top,
                ConnectionSite::Right,
                ConnectionSite::Bottom,
                ConnectionSite::Left,
                ConnectionSite::Auto,
            ] {
                let (pt, _) = shape_connection_point(obj, site, None);
                resolved_positions.insert((id.clone(), site), pt);
            }
        }
    }

    for obj in objects.iter_mut() {
        if let DrawObject::Connector { start, end, .. } = obj {
            if let crate::model::ConnectorEnd::Attached { shape_id, site } = start
                && deleted_shape_ids.contains(shape_id)
            {
                let last_pt = resolved_positions
                    .get(&(shape_id.clone(), *site))
                    .copied()
                    .unwrap_or(Point2D::new(0.0, 0.0));
                *start = crate::model::ConnectorEnd::Free {
                    x: last_pt.x,
                    y: last_pt.y,
                };
            }
            if let crate::model::ConnectorEnd::Attached { shape_id, site } = end
                && deleted_shape_ids.contains(shape_id)
            {
                let last_pt = resolved_positions
                    .get(&(shape_id.clone(), *site))
                    .copied()
                    .unwrap_or(Point2D::new(0.0, 0.0));
                *end = crate::model::ConnectorEnd::Free {
                    x: last_pt.x,
                    y: last_pt.y,
                };
            }
        }
    }
}

/// Memetakan ulang shape_id pada connector salinan ke bentuk salinan (Task 7.5).
/// Jika bentuk pasangannya tidak ikut disalin, ubah ujungnya menjadi Free di koordinat terakhir.
pub fn remap_connector_endpoints_on_copy(
    pasted_objects: &mut [DrawObject],
    id_map: &std::collections::HashMap<String, String>,
    original_shapes: &[DrawObject],
) {
    for obj in pasted_objects {
        if let DrawObject::Connector { start, end, .. } = obj {
            match start {
                crate::model::ConnectorEnd::Attached { shape_id, site } => {
                    if let Some(new_shape_id) = id_map.get(shape_id) {
                        *shape_id = new_shape_id.clone();
                    } else {
                        let last_pt = original_shapes
                            .iter()
                            .find(|s| s.id() == shape_id)
                            .map(|s| shape_connection_point(s, *site, None).0)
                            .unwrap_or(Point2D::new(0.0, 0.0));
                        *start = crate::model::ConnectorEnd::Free {
                            x: last_pt.x,
                            y: last_pt.y,
                        };
                    }
                }
                crate::model::ConnectorEnd::Free { .. } => {}
            }
            match end {
                crate::model::ConnectorEnd::Attached { shape_id, site } => {
                    if let Some(new_shape_id) = id_map.get(shape_id) {
                        *shape_id = new_shape_id.clone();
                    } else {
                        let last_pt = original_shapes
                            .iter()
                            .find(|s| s.id() == shape_id)
                            .map(|s| shape_connection_point(s, *site, None).0)
                            .unwrap_or(Point2D::new(0.0, 0.0));
                        *end = crate::model::ConnectorEnd::Free {
                            x: last_pt.x,
                            y: last_pt.y,
                        };
                    }
                }
                crate::model::ConnectorEnd::Free { .. } => {}
            }
        }
    }
}

/// Menghitung seluruh rangkaian titik (polyline) dari sebuah connector
/// dengan memperhitungkan posisi bentuk yang terhubung (Task 7.1, 7.3).
pub fn connector_polyline(
    connector: &DrawObject,
    all_objects: &[DrawObject],
) -> Option<Vec<Point2D>> {
    if let DrawObject::Connector {
        start,
        end,
        route,
        control_points,
        ..
    } = connector
    {
        let ((start_pt, start_dir), (end_pt, end_dir)) =
            resolve_connector_endpoints(start, end, all_objects);
        Some(calculate_connector_path(
            start_pt,
            start_dir,
            end_pt,
            end_dir,
            *route,
            control_points,
        ))
    } else {
        None
    }
}

/// Hit-testing akurat pada seluruh segmen jalur connector (Task 7.1, 7.3).
pub fn hit_test_connector_with_objects(
    connector: &DrawObject,
    p: Point2D,
    tolerance: f32,
    all_objects: &[DrawObject],
) -> bool {
    if let Some(poly) = connector_polyline(connector, all_objects) {
        let tol = if let DrawObject::Connector { style, .. } = connector {
            tolerance + style.stroke_width * 0.5
        } else {
            tolerance
        };
        for i in 0..poly.len() - 1 {
            let a = eframe::egui::Pos2::new(poly[i].x, poly[i].y);
            let b = eframe::egui::Pos2::new(poly[i + 1].x, poly[i + 1].y);
            if crate::geometry::point_to_segment_dist(eframe::egui::Pos2::new(p.x, p.y), a, b)
                <= tol
            {
                return true;
            }
        }
    }
    false
}
