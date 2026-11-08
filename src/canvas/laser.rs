use eframe::egui::Pos2;

pub fn smooth_laser_points(pts: &[Pos2]) -> Vec<Pos2> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut smoothed = Vec::with_capacity(pts.len() * 3);
    smoothed.push(pts[0]);
    for i in 0..(pts.len() - 1) {
        let p0 = if i == 0 {
            pts[0]
        } else {
            Pos2::new(
                (pts[i - 1].x + pts[i].x) / 2.0,
                (pts[i - 1].y + pts[i].y) / 2.0,
            )
        };
        let p1 = pts[i];
        let p2 = if i + 2 >= pts.len() {
            pts[pts.len() - 1]
        } else {
            Pos2::new(
                (pts[i].x + pts[i + 1].x) / 2.0,
                (pts[i].y + pts[i + 1].y) / 2.0,
            )
        };
        for step in 1..=3 {
            let t = step as f32 / 3.0;
            let one_minus_t = 1.0 - t;
            let x = one_minus_t * one_minus_t * p0.x + 2.0 * one_minus_t * t * p1.x + t * t * p2.x;
            let y = one_minus_t * one_minus_t * p0.y + 2.0 * one_minus_t * t * p1.y + t * t * p2.y;
            smoothed.push(Pos2::new(x, y));
        }
    }
    if let Some(&last) = pts.last()
        && let Some(res_last) = smoothed.last_mut()
    {
        *res_last = last;
    }
    smoothed
}
