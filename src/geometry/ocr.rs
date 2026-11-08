use crate::model::{DrawObject, StrokePoint};
use eframe::egui::{Pos2, Rect};

/// Fase 8.6: Klasifikasi satu karakter tulisan tangan dari kumpulan goresan (glyph cluster)
/// menggunakan fitur geometri & topologi murni Rust (Zero External Runtime).
fn classify_glyph_strokes(strokes: &[&[StrokePoint]], bounds: Rect) -> char {
    let w = bounds.width().max(1.0);
    let h = bounds.height().max(1.0);
    let aspect = w / h;
    let num_strokes = strokes.len();

    // Titik sangat kecil -> titik baca '.'
    if w < 6.0 && h < 6.0 {
        return '.';
    }

    // Goresan tunggal sangat ramping vertikal -> 'I'
    if aspect < 0.28 {
        return 'I';
    }

    // Goresan tunggal sangat datar horizontal -> '-'
    if h < 8.0 && aspect > 2.5 {
        return '-';
    }

    if num_strokes == 1 {
        let pts = strokes[0];
        if pts.len() >= 2 {
            let first = pts.first().unwrap().to_pos2();
            let last = pts.last().unwrap().to_pos2();
            let diag = (w * w + h * h).sqrt().max(1.0);
            let close_ratio = (first - last).length() / diag;

            // Kurva tertutup (awal dan akhir berdekatan) -> 'O'
            if close_ratio < 0.28 && pts.len() >= 5 {
                return 'O';
            }

            // Cek bentuk sudut 'L', 'V', 'U', 'C', 'Z', 'S'
            let norm_first = Pos2::new((first.x - bounds.min.x) / w, (first.y - bounds.min.y) / h);
            let norm_last = Pos2::new((last.x - bounds.min.x) / w, (last.y - bounds.min.y) / h);

            let has_bottom_left = pts.iter().any(|p| {
                let nx = (p.x - bounds.min.x) / w;
                let ny = (p.y - bounds.min.y) / h;
                nx < 0.35 && ny > 0.75
            });
            let has_top_right = pts.iter().any(|p| {
                let nx = (p.x - bounds.min.x) / w;
                let ny = (p.y - bounds.min.y) / h;
                nx > 0.65 && ny < 0.30
            });

            if norm_first.x < 0.35
                && norm_first.y < 0.30
                && norm_last.x > 0.65
                && norm_last.y > 0.70
                && has_bottom_left
                && !has_top_right
            {
                return 'L';
            }

            if norm_first.y < 0.35
                && norm_last.y < 0.35
                && norm_first.x < 0.35
                && norm_last.x > 0.65
            {
                return 'V';
            }

            if norm_first.x > 0.60
                && norm_first.y < 0.35
                && norm_last.x > 0.60
                && norm_last.y > 0.65
            {
                let has_right_mid = pts.iter().any(|p| {
                    let nx = (p.x - bounds.min.x) / w;
                    let ny = (p.y - bounds.min.y) / h;
                    nx > 0.65 && (0.35..=0.65).contains(&ny)
                });
                if !has_right_mid {
                    return 'C';
                }
                return 'S';
            }

            if has_top_right && has_bottom_left {
                return 'Z';
            }
        }
        return 'I';
    }

    if num_strokes == 2 {
        let mut horiz = 0;
        let mut vert = 0;
        let mut diag = 0;
        let mut horiz_y = 0.5;
        for s in strokes {
            if let (Some(f), Some(l)) = (s.first(), s.last()) {
                let dx = (l.x - f.x).abs();
                let dy = (l.y - f.y).abs();
                if dx > dy * 1.6 {
                    horiz += 1;
                    horiz_y = (((f.y + l.y) * 0.5) - bounds.min.y) / h;
                } else if dy > dx * 1.6 {
                    vert += 1;
                } else {
                    diag += 1;
                }
            }
        }
        if horiz == 1 && vert == 1 {
            if horiz_y < 0.35 {
                return 'T';
            } else if horiz_y > 0.70 {
                return 'L';
            } else {
                return '+';
            }
        }
        if diag == 2 {
            return 'X';
        }
        return 'D';
    }

    if num_strokes == 3 {
        let mut horiz = 0;
        let mut vert = 0;
        for s in strokes {
            if let (Some(f), Some(l)) = (s.first(), s.last()) {
                let dx = (l.x - f.x).abs();
                let dy = (l.y - f.y).abs();
                if dx > dy * 1.5 {
                    horiz += 1;
                } else if dy > dx * 1.5 {
                    vert += 1;
                }
            }
        }
        if vert == 2 && horiz == 1 {
            return 'H';
        }
        if vert == 1 && horiz == 2 {
            return 'F';
        }
        return 'A';
    }

    let mut horiz = 0;
    for s in strokes {
        if let (Some(f), Some(l)) = (s.first(), s.last())
            && (l.x - f.x).abs() > (l.y - f.y).abs() * 1.4
        {
            horiz += 1;
        }
    }
    if horiz >= 3 { 'E' } else { 'M' }
}

/// Fase 8.6: Ekstraksi OCR tulisan tangan murni Rust dari objek halaman untuk indeks pencarian.
/// Mengelompokkan goresan pena ke dalam baris dan karakter, mengenali karakter per klaster,
/// serta menggabungkannya secara otomatis.
pub fn recognize_handwriting_from_strokes(objects: &[DrawObject]) -> String {
    struct StrokeItem<'a> {
        pts: &'a [StrokePoint],
        bounds: Rect,
    }

    let mut items: Vec<StrokeItem<'_>> = Vec::new();
    for obj in objects {
        if let DrawObject::Stroke {
            points,
            is_highlighter,
            ..
        } = obj
        {
            if *is_highlighter || points.len() < 2 {
                continue;
            }
            let mut min_x = f32::INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for p in points {
                min_x = min_x.min(p.x);
                min_y = min_y.min(p.y);
                max_x = max_x.max(p.x);
                max_y = max_y.max(p.y);
            }
            items.push(StrokeItem {
                pts: points.as_slice(),
                bounds: Rect::from_min_max(Pos2::new(min_x, min_y), Pos2::new(max_x, max_y)),
            });
        }
    }

    if items.is_empty() {
        return String::new();
    }

    items.sort_by(|a, b| {
        a.bounds
            .center()
            .y
            .partial_cmp(&b.bounds.center().y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut lines: Vec<(Rect, Vec<StrokeItem<'_>>)> = Vec::new();
    for item in items {
        let cy = item.bounds.center().y;
        if let Some((line_rect, line_items)) = lines.iter_mut().find(|(r, _)| {
            let overlap_y =
                item.bounds.min.y <= r.max.y + 8.0 && item.bounds.max.y >= r.min.y - 8.0;
            let close_cy = (cy - r.center().y).abs() <= r.height().max(24.0) * 0.65;
            overlap_y || close_cy
        }) {
            *line_rect = line_rect.union(item.bounds);
            line_items.push(item);
        } else {
            lines.push((item.bounds, vec![item]));
        }
    }

    lines.sort_by(|a, b| {
        a.0.min
            .y
            .partial_cmp(&b.0.min.y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result_lines: Vec<String> = Vec::new();

    for (line_rect, mut line_items) in lines {
        let line_h = line_rect.height().max(20.0);
        line_items.sort_by(|a, b| {
            a.bounds
                .min
                .x
                .partial_cmp(&b.bounds.min.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut clusters: Vec<(Rect, Vec<&[StrokePoint]>)> = Vec::new();
        for item in line_items {
            if let Some((c_rect, c_strokes)) = clusters.last_mut() {
                let gap_x = item.bounds.min.x - c_rect.max.x;
                if gap_x <= line_h * 0.12 {
                    *c_rect = c_rect.union(item.bounds);
                    c_strokes.push(item.pts);
                    continue;
                }
            }
            clusters.push((item.bounds, vec![item.pts]));
        }

        let mut line_str = String::new();
        let mut prev_max_x: Option<f32> = None;
        for (c_rect, c_strokes) in clusters {
            if let Some(px) = prev_max_x {
                let gap = c_rect.min.x - px;
                if gap > line_h * 0.65 {
                    line_str.push(' ');
                }
            }
            let ch = classify_glyph_strokes(&c_strokes, c_rect);
            line_str.push(ch);
            prev_max_x = Some(c_rect.max.x);
        }

        if !line_str.is_empty() {
            result_lines.push(line_str);
        }
    }

    result_lines.join(" ")
}
