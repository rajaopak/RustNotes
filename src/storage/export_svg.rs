use crate::model::Page;
use std::path::Path;

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };
        result.push(CHARSET[(b0 >> 2) as usize] as char);
        result.push(CHARSET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARSET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARSET[(b2 & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

pub fn export_page_to_svg(page: &Page) -> String {
    let mut out = String::with_capacity(4096);
    let w = page.width;
    let h = page.height;

    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\">\n<defs>\n  <filter id=\"sticky-shadow\" x=\"-5%\" y=\"-5%\" width=\"110%\" height=\"115%\">\n    <feDropShadow dx=\"1\" dy=\"3\" stdDeviation=\"3\" flood-opacity=\"0.18\"/>\n  </filter>\n</defs>\n"
    ));

    // 1. Background
    let bg_color = format!(
        "rgba({},{},{},{:.3})",
        page.paper_color.r,
        page.paper_color.g,
        page.paper_color.b,
        page.paper_color.a as f32 / 255.0
    );
    out.push_str(&format!(
        "<rect width=\"{w}\" height=\"{h}\" fill=\"{bg_color}\"/>\n"
    ));

    // 2. Template Pattern
    let spacing = 28.0;
    match page.template {
        crate::model::PageTemplate::Blank => {}
        crate::model::PageTemplate::Ruled => {
            let mut y = 48.0;
            while y < h {
                out.push_str(&format!(
                    "<line x1=\"0\" y1=\"{y:.1}\" x2=\"{w:.1}\" y2=\"{y:.1}\" stroke=\"#E6EBF0\" stroke-width=\"0.8\"/>\n"
                ));
                y += spacing;
            }
        }
        crate::model::PageTemplate::RuledNarrow => {
            let sp = 20.0;
            let mut y = 30.0;
            while y < h {
                out.push_str(&format!(
                    "<line x1=\"0\" y1=\"{y:.1}\" x2=\"{w:.1}\" y2=\"{y:.1}\" stroke=\"#E6EBF0\" stroke-width=\"0.7\"/>\n"
                ));
                y += sp;
            }
        }
        crate::model::PageTemplate::RuledWide => {
            let sp = 40.0;
            let mut y = 50.0;
            while y < h {
                out.push_str(&format!(
                    "<line x1=\"0\" y1=\"{y:.1}\" x2=\"{w:.1}\" y2=\"{y:.1}\" stroke=\"#E6EBF0\" stroke-width=\"0.85\"/>\n"
                ));
                y += sp;
            }
        }
        crate::model::PageTemplate::Grid => {
            let mut y = spacing;
            while y < h {
                out.push_str(&format!(
                    "<line x1=\"0\" y1=\"{y:.1}\" x2=\"{w:.1}\" y2=\"{y:.1}\" stroke=\"#EDF0F3\" stroke-width=\"0.8\"/>\n"
                ));
                y += spacing;
            }
            let mut x = spacing;
            while x < w {
                out.push_str(&format!(
                    "<line x1=\"{x:.1}\" y1=\"0\" x2=\"{x:.1}\" y2=\"{h:.1}\" stroke=\"#EDF0F3\" stroke-width=\"0.8\"/>\n"
                ));
                x += spacing;
            }
        }
        crate::model::PageTemplate::GridFine => {
            let sp = 15.0;
            let mut y = sp;
            while y < h {
                out.push_str(&format!(
                    "<line x1=\"0\" y1=\"{y:.1}\" x2=\"{w:.1}\" y2=\"{y:.1}\" stroke=\"#EDF0F3\" stroke-width=\"0.6\"/>\n"
                ));
                y += sp;
            }
            let mut x = sp;
            while x < w {
                out.push_str(&format!(
                    "<line x1=\"{x:.1}\" y1=\"0\" x2=\"{x:.1}\" y2=\"{h:.1}\" stroke=\"#EDF0F3\" stroke-width=\"0.6\"/>\n"
                ));
                x += sp;
            }
        }
        crate::model::PageTemplate::Dotted => {
            let mut y = spacing;
            while y < h {
                let mut x = spacing;
                while x < w {
                    out.push_str(&format!(
                        "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"1.0\" fill=\"#CCD3DA\"/>\n"
                    ));
                    x += spacing;
                }
                y += spacing;
            }
        }
        crate::model::PageTemplate::DottedFine => {
            let sp = 15.0;
            let mut y = sp;
            while y < h {
                let mut x = sp;
                while x < w {
                    out.push_str(&format!(
                        "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"0.75\" fill=\"#CCD3DA\"/>\n"
                    ));
                    x += sp;
                }
                y += sp;
            }
        }
        crate::model::PageTemplate::Isometric => {
            let dx = 30.0;
            let dy = 30.0 * 0.8660254;
            let mut row = 0usize;
            let mut y = dy;
            while y < h {
                let x_offset = if row % 2 == 1 { dx * 0.5 } else { 0.0 };
                let mut x = x_offset + dx;
                while x < w {
                    out.push_str(&format!(
                        "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"0.9\" fill=\"#CCD3DA\"/>\n"
                    ));
                    x += dx;
                }
                y += dy;
                row += 1;
            }
        }
        crate::model::PageTemplate::Cornell => {
            let summary_y = h * 0.84;
            let cue_x = w * 0.28;
            let mut y = spacing * 1.5;
            while y < summary_y {
                out.push_str(&format!(
                    "<line x1=\"0\" y1=\"{y:.1}\" x2=\"{w:.1}\" y2=\"{y:.1}\" stroke=\"#E6EBF0\" stroke-width=\"0.8\"/>\n"
                ));
                y += spacing;
            }
            out.push_str(&format!(
                "<line x1=\"{cue_x:.1}\" y1=\"{:.1}\" x2=\"{cue_x:.1}\" y2=\"{summary_y:.1}\" stroke=\"#CCD3DA\" stroke-width=\"1.5\"/>\n",
                h * 0.04
            ));
            out.push_str(&format!(
                "<line x1=\"0\" y1=\"{summary_y:.1}\" x2=\"{w:.1}\" y2=\"{summary_y:.1}\" stroke=\"#CCD3DA\" stroke-width=\"1.5\"/>\n"
            ));
        }
    }

    // 3. DrawObjects
    for obj in &page.objects {
        match obj {
            crate::model::DrawObject::Stroke {
                points,
                color,
                width: stroke_w,
                is_highlighter,
                style,
                ..
            } => {
                if points.is_empty() {
                    continue;
                }
                let c_alpha = if *is_highlighter {
                    (color.a as f32 / 255.0) * crate::model::HIGHLIGHTER_OPACITY_F32
                } else {
                    color.a as f32 / 255.0
                };
                let stroke_color =
                    format!("rgba({},{},{},{:.3})", color.r, color.g, color.b, c_alpha);

                if points.len() == 1 {
                    let pt = &points[0];
                    let r = (stroke_w * 0.5 * pt.pressure).max(0.5);
                    out.push_str(&format!(
                        "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{r:.2}\" fill=\"{stroke_color}\"/>\n",
                        pt.x, pt.y
                    ));
                    continue;
                }

                let dash_attr = match style {
                    crate::model::StrokeStyle::Solid => String::new(),
                    crate::model::StrokeStyle::Dashed => {
                        format!(
                            " stroke-dasharray=\"{:.1},{:.1}\"",
                            stroke_w * 3.0,
                            stroke_w * 2.0
                        )
                    }
                    crate::model::StrokeStyle::DashDot => {
                        format!(
                            " stroke-dasharray=\"{:.1},{:.1},{:.1},{:.1}\"",
                            stroke_w * 3.0,
                            stroke_w * 1.5,
                            stroke_w * 0.8,
                            stroke_w * 1.5
                        )
                    }
                    crate::model::StrokeStyle::Dotted => {
                        format!(
                            " stroke-dasharray=\"{:.1},{:.1}\"",
                            stroke_w * 0.5,
                            stroke_w * 1.5
                        )
                    }
                };

                let mut d = String::new();
                for (i, pt) in points.iter().enumerate() {
                    if i == 0 {
                        d.push_str(&format!("M {:.1} {:.1}", pt.x, pt.y));
                    } else {
                        d.push_str(&format!(" L {:.1} {:.1}", pt.x, pt.y));
                    }
                }
                out.push_str(&format!(
                    "<path d=\"{d}\" stroke=\"{stroke_color}\" stroke-width=\"{stroke_w:.1}\"{dash_attr} stroke-linecap=\"round\" stroke-linejoin=\"round\" fill=\"none\"/>\n"
                ));
            }
            crate::model::DrawObject::Shape {
                shape_type,
                x,
                y,
                width: sw,
                height: sh,
                color: stroke_color,
                stroke_width,
                fill,
                corner_radius,
                rotation,
                label,
                ..
            } => {
                let fill_str = if let Some(f) = fill {
                    format!("rgba({},{},{},{:.3})", f.r, f.g, f.b, f.a as f32 / 255.0)
                } else {
                    "none".to_string()
                };
                let stroke_str = format!(
                    "rgba({},{},{},{:.3})",
                    stroke_color.r,
                    stroke_color.g,
                    stroke_color.b,
                    stroke_color.a as f32 / 255.0
                );
                let center_x = *x + *sw / 2.0;
                let center_y = *y + *sh / 2.0;
                let rot_attr = if rotation.abs() > 0.001 {
                    format!(
                        " transform=\"rotate({:.2} {:.1} {:.1})\"",
                        rotation.to_degrees(),
                        center_x,
                        center_y
                    )
                } else {
                    String::new()
                };

                match shape_type {
                    crate::model::ShapeKind::Rectangle => {
                        out.push_str(&format!(
                            "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{sw:.1}\" height=\"{sh:.1}\" rx=\"{corner_radius:.1}\" ry=\"{corner_radius:.1}\" fill=\"{fill_str}\" stroke=\"{stroke_str}\" stroke-width=\"{stroke_width:.1}\"{rot_attr}/>\n"
                        ));
                    }
                    crate::model::ShapeKind::Ellipse => {
                        let rx = (*sw / 2.0).max(0.5);
                        let ry = (*sh / 2.0).max(0.5);
                        out.push_str(&format!(
                            "<ellipse cx=\"{center_x:.1}\" cy=\"{center_y:.1}\" rx=\"{rx:.1}\" ry=\"{ry:.1}\" fill=\"{fill_str}\" stroke=\"{stroke_str}\" stroke-width=\"{stroke_width:.1}\"{rot_attr}/>\n"
                        ));
                    }
                    crate::model::ShapeKind::Line => {
                        out.push_str(&format!(
                            "<line x1=\"{x:.1}\" y1=\"{y:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{stroke_str}\" stroke-width=\"{stroke_width:.1}\" stroke-linecap=\"round\"{rot_attr}/>\n",
                            *x + *sw,
                            *y + *sh
                        ));
                    }
                    _ => {
                        let verts = crate::geometry::shape_vertices(*shape_type, *x, *y, *sw, *sh);
                        let points_str: String = verts
                            .iter()
                            .map(|v| format!("{:.1},{:.1}", v.x, v.y))
                            .collect::<Vec<_>>()
                            .join(" ");
                        out.push_str(&format!(
                            "<polygon points=\"{points_str}\" fill=\"{fill_str}\" stroke=\"{stroke_str}\" stroke-width=\"{stroke_width:.1}\" stroke-linejoin=\"round\"{rot_attr}/>\n"
                        ));
                    }
                }

                // Shape Label
                if let Some(lbl) = label
                    && !lbl.text.trim().is_empty()
                {
                    let (safe_w, safe_h) = crate::geometry::calculate_shape_text_bounds(
                        *shape_type,
                        *sw,
                        *sh,
                        *corner_radius,
                        *stroke_width,
                    );
                    let font_size = crate::geometry::calculate_auto_fit_font_size(
                        lbl.font_size,
                        &lbl.text,
                        safe_w,
                        safe_h,
                        8.0,
                    );
                    let text_col = format!(
                        "rgba({},{},{},{:.3})",
                        lbl.color.r,
                        lbl.color.g,
                        lbl.color.b,
                        lbl.color.a as f32 / 255.0
                    );
                    let lines: Vec<&str> = lbl.text.lines().collect();
                    let line_height = font_size * 1.25;
                    let total_text_h = lines.len() as f32 * line_height;
                    let start_y = center_y - total_text_h / 2.0 + font_size * 0.85;

                    out.push_str(&format!(
                        "<text x=\"{center_x:.1}\" y=\"{start_y:.1}\" font-family=\"sans-serif\" font-size=\"{font_size:.1}\" fill=\"{text_col}\" text-anchor=\"middle\"{rot_attr}>"
                    ));
                    for (i, line) in lines.iter().enumerate() {
                        let dy_str = if i == 0 {
                            String::new()
                        } else {
                            format!(" x=\"{center_x:.1}\" dy=\"{line_height:.1}\"")
                        };
                        out.push_str(&format!("<tspan{dy_str}>{}</tspan>", escape_xml(line)));
                    }
                    out.push_str("</text>\n");
                }
            }
            crate::model::DrawObject::Connector { style, .. } => {
                if let Some(poly) = crate::geometry::connector_polyline(obj, &page.objects)
                    && poly.len() >= 2
                {
                    let stroke_str = format!(
                        "rgba({},{},{},{:.3})",
                        style.stroke_color.r,
                        style.stroke_color.g,
                        style.stroke_color.b,
                        style.stroke_color.a as f32 / 255.0
                    );
                    let points_str: String = poly
                        .iter()
                        .map(|p| format!("{:.1},{:.1}", p.x, p.y))
                        .collect::<Vec<_>>()
                        .join(" ");
                    out.push_str(&format!(
                        "<polyline points=\"{points_str}\" stroke=\"{stroke_str}\" stroke-width=\"{:.1}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" fill=\"none\"/>\n",
                        style.stroke_width
                    ));

                    // Arrow markers
                    if style.end_marker == crate::model::MarkerKind::Arrow {
                        let last = poly[poly.len() - 1];
                        let prev = poly[poly.len() - 2];
                        let dx = last.x - prev.x;
                        let dy = last.y - prev.y;
                        let len = (dx * dx + dy * dy).sqrt();
                        if len > 0.001 {
                            let dir_x = dx / len;
                            let dir_y = dy / len;
                            let perp_x = -dir_y;
                            let perp_y = dir_x;
                            let arrow_len = (style.stroke_width * 3.5).max(9.0);
                            let arrow_half_w = arrow_len * 0.5;
                            let base_x = last.x - dir_x * arrow_len;
                            let base_y = last.y - dir_y * arrow_len;
                            let p1 = format!("{:.1},{:.1}", last.x, last.y);
                            let p2 = format!(
                                "{:.1},{:.1}",
                                base_x + perp_x * arrow_half_w,
                                base_y + perp_y * arrow_half_w
                            );
                            let p3 = format!(
                                "{:.1},{:.1}",
                                base_x - perp_x * arrow_half_w,
                                base_y - perp_y * arrow_half_w
                            );
                            out.push_str(&format!(
                                "<polygon points=\"{p1} {p2} {p3}\" fill=\"{stroke_str}\"/>\n"
                            ));
                        }
                    }
                    if style.start_marker == crate::model::MarkerKind::Arrow {
                        let first = poly[0];
                        let next = poly[1];
                        let dx = first.x - next.x;
                        let dy = first.y - next.y;
                        let len = (dx * dx + dy * dy).sqrt();
                        if len > 0.001 {
                            let dir_x = dx / len;
                            let dir_y = dy / len;
                            let perp_x = -dir_y;
                            let perp_y = dir_x;
                            let arrow_len = (style.stroke_width * 3.5).max(9.0);
                            let arrow_half_w = arrow_len * 0.5;
                            let base_x = first.x - dir_x * arrow_len;
                            let base_y = first.y - dir_y * arrow_len;
                            let p1 = format!("{:.1},{:.1}", first.x, first.y);
                            let p2 = format!(
                                "{:.1},{:.1}",
                                base_x + perp_x * arrow_half_w,
                                base_y + perp_y * arrow_half_w
                            );
                            let p3 = format!(
                                "{:.1},{:.1}",
                                base_x - perp_x * arrow_half_w,
                                base_y - perp_y * arrow_half_w
                            );
                            out.push_str(&format!(
                                "<polygon points=\"{p1} {p2} {p3}\" fill=\"{stroke_str}\"/>\n"
                            ));
                        }
                    }
                }
            }
            crate::model::DrawObject::Text {
                text,
                x,
                y,
                font_size,
                color,
                rotation,
                ..
            } => {
                let text_col = format!(
                    "rgba({},{},{},{:.3})",
                    color.r,
                    color.g,
                    color.b,
                    color.a as f32 / 255.0
                );
                let rot_attr = if rotation.abs() > 0.001 {
                    format!(
                        " transform=\"rotate({:.2} {:.1} {:.1})\"",
                        rotation.to_degrees(),
                        x,
                        y
                    )
                } else {
                    String::new()
                };
                let lines: Vec<&str> = text.lines().collect();
                let line_h = font_size * 1.25;
                let start_y = *y + font_size * 0.85;
                out.push_str(&format!(
                    "<text x=\"{x:.1}\" y=\"{start_y:.1}\" font-family=\"sans-serif\" font-size=\"{font_size:.1}\" fill=\"{text_col}\"{rot_attr}>"
                ));
                for (i, line) in lines.iter().enumerate() {
                    let dy_str = if i == 0 {
                        String::new()
                    } else {
                        format!(" x=\"{x:.1}\" dy=\"{line_h:.1}\"")
                    };
                    out.push_str(&format!("<tspan{dy_str}>{}</tspan>", escape_xml(line)));
                }
                out.push_str("</text>\n");
            }
            crate::model::DrawObject::StickyNote {
                text,
                x,
                y,
                width: sw,
                height: sh,
                color,
                ..
            } => {
                let note_col = format!(
                    "rgba({},{},{},{:.3})",
                    color.r,
                    color.g,
                    color.b,
                    color.a as f32 / 255.0
                );
                out.push_str(&format!(
                    "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{sw:.1}\" height=\"{sh:.1}\" rx=\"4\" ry=\"4\" fill=\"{note_col}\" filter=\"url(#sticky-shadow)\"/>\n"
                ));
                let lines: Vec<&str> = text.lines().collect();
                let font_size = 13.0;
                let line_h = 17.0;
                let start_y = *y + 18.0;
                let text_x = *x + 10.0;
                out.push_str(&format!(
                    "<text x=\"{text_x:.1}\" y=\"{start_y:.1}\" font-family=\"sans-serif\" font-size=\"{font_size:.1}\" fill=\"#2A2A2A\">"
                ));
                for (i, line) in lines.iter().enumerate() {
                    let dy_str = if i == 0 {
                        String::new()
                    } else {
                        format!(" x=\"{text_x:.1}\" dy=\"{line_h:.1}\"")
                    };
                    out.push_str(&format!("<tspan{dy_str}>{}</tspan>", escape_xml(line)));
                }
                out.push_str("</text>\n");
            }
            crate::model::DrawObject::Image {
                asset_id,
                x,
                y,
                width,
                height,
                rotation,
                ..
            } => {
                // Cari berkas aset dari storage direktori aplikasi
                let base_dir = super::io::app_storage_dir();
                // Cari di semua folder notebook
                if let Ok(entries) = std::fs::read_dir(base_dir.join("notebooks")) {
                    for entry in entries.flatten() {
                        let potential_asset = entry.path().join("assets").join(asset_id);
                        if potential_asset.exists()
                            && let Ok(bytes) = std::fs::read(&potential_asset)
                        {
                            let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                                "image/png"
                            } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
                                "image/jpeg"
                            } else if bytes.starts_with(b"RIFF") {
                                "image/webp"
                            } else {
                                "image/png"
                            };
                            let b64 = base64_encode(&bytes);
                            let rot_attr = if rotation.abs() > 0.001 {
                                let cx = *x + *width / 2.0;
                                let cy = *y + *height / 2.0;
                                format!(
                                    " transform=\"rotate({:.2} {:.1} {:.1})\"",
                                    rotation.to_degrees(),
                                    cx,
                                    cy
                                )
                            } else {
                                String::new()
                            };
                            out.push_str(&format!(
                                "<image x=\"{x:.1}\" y=\"{y:.1}\" width=\"{width:.1}\" height=\"{height:.1}\" href=\"data:{mime};base64,{b64}\"{rot_attr}/>\n"
                            ));
                            break;
                        }
                    }
                }
            }
        }
    }

    out.push_str("</svg>\n");
    out
}

pub fn export_page_to_svg_file(path: &Path, page: &Page) -> Result<(), String> {
    let svg = export_page_to_svg(page);
    std::fs::write(path, svg).map_err(|e| format!("Gagal menyimpan berkas SVG: {e}"))
}
