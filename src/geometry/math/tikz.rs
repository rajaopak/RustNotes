use super::expr::{eval_const_expr, eval_math_expr, eval_math_expr_2d};
use super::{
    MathLayoutItem, MathLineDash, MathParseError, extract_braced_group, extract_bracket_group,
    strip_latex_comments,
};
use eframe::egui::{Color32, Pos2};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColormapKind {
    #[default]
    PgfpStandard,
    Viridis,
    Hot,
    Cool,
    Bluered,
}

fn parse_colormap_name(s: &str) -> Option<ColormapKind> {
    let low = s.trim().to_ascii_lowercase();
    let name = low.strip_prefix("colormap/").unwrap_or(&low).trim();
    match name {
        "viridis" => Some(ColormapKind::Viridis),
        "hot" => Some(ColormapKind::Hot),
        "cool" => Some(ColormapKind::Cool),
        "bluered" => Some(ColormapKind::Bluered),
        "jet" | "pgfpstandard" | "standard" => Some(ColormapKind::PgfpStandard),
        _ => None,
    }
}

fn sample_colormap(kind: ColormapKind, t_raw: f32) -> Color32 {
    let t = t_raw.clamp(0.0, 1.0);
    let stops: &[(f32, [u8; 3])] = match kind {
        ColormapKind::PgfpStandard => &[
            (0.00, [42, 72, 184]),
            (0.28, [32, 168, 208]),
            (0.54, [78, 194, 108]),
            (0.78, [242, 202, 52]),
            (1.00, [222, 54, 42]),
        ],
        ColormapKind::Viridis => &[
            (0.00, [68, 1, 84]),
            (0.25, [59, 82, 139]),
            (0.50, [33, 145, 140]),
            (0.75, [94, 201, 98]),
            (1.00, [253, 231, 37]),
        ],
        ColormapKind::Hot => &[
            (0.00, [35, 15, 25]),
            (0.35, [190, 35, 30]),
            (0.70, [242, 145, 28]),
            (1.00, [252, 240, 120]),
        ],
        ColormapKind::Cool => &[
            (0.00, [40, 210, 235]),
            (0.50, [135, 120, 235]),
            (1.00, [235, 55, 210]),
        ],
        ColormapKind::Bluered => &[
            (0.00, [36, 92, 214]),
            (0.50, [225, 228, 238]),
            (1.00, [218, 48, 56]),
        ],
    };
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        if t <= t1 {
            let u = ((t - t0) / (t1 - t0).max(1e-6)).clamp(0.0, 1.0);
            let lerp =
                |a: u8, b: u8| -> u8 { ((a as f32) + ((b as f32) - (a as f32)) * u).round() as u8 };
            return Color32::from_rgb(lerp(c0[0], c1[0]), lerp(c0[1], c1[1]), lerp(c0[2], c1[2]));
        }
    }
    let last = stops.last().unwrap().1;
    Color32::from_rgb(last[0], last[1], last[2])
}

#[derive(Clone, Debug, PartialEq)]
pub enum DiagramSpec {
    Tikz(TikzPicture),
    Axis(Box<AxisPlot>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum NodeAnchor {
    #[default]
    Center,
    Above,
    Below,
    Left,
    Right,
    AboveLeft,
    AboveRight,
    BelowLeft,
    BelowRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeShape {
    None,
    Circle,
    Rectangle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AxisLinesMode {
    #[default]
    Box,
    Middle,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlotMark {
    #[default]
    None,
    FilledCircle,
    OpenCircle,
    Square,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TikzStyle {
    pub draw_color: Option<Color32>,
    pub draw_disabled: bool,
    pub fill_color: Option<Color32>,
    pub fill_opacity: f32,
    pub stroke_width: f32,
    pub dash: MathLineDash,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub node_anchor: NodeAnchor,
    pub node_shape: NodeShape,
    pub node_pos: Option<f32>,
    pub text_color: Option<Color32>,
    pub mark: PlotMark,
    pub only_marks: bool,
    pub ybar: bool,
    pub domain: Option<(f64, f64)>,
    pub y_domain: Option<(f64, f64)>,
    pub samples: Option<usize>,
    pub samples_y: Option<usize>,
    pub step: Option<f64>,
    pub surf: bool,
    pub mesh: bool,
    pub shader_flat: bool,
    pub colormap: Option<ColormapKind>,
}

impl Default for TikzStyle {
    fn default() -> Self {
        Self {
            draw_color: None,
            draw_disabled: false,
            fill_color: None,
            fill_opacity: 0.28,
            stroke_width: 1.4,
            dash: MathLineDash::Solid,
            arrow_start: false,
            arrow_end: false,
            node_anchor: NodeAnchor::Center,
            node_shape: NodeShape::None,
            node_pos: None,
            text_color: None,
            mark: PlotMark::None,
            only_marks: false,
            ybar: false,
            domain: None,
            y_domain: None,
            samples: None,
            samples_y: None,
            step: None,
            surf: false,
            mesh: false,
            shader_flat: false,
            colormap: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TikzCoord {
    Point(f64, f64),
    Polar(f64, f64),
    Named(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlinePathNode {
    pub style: TikzStyle,
    pub label: String,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PathSegment {
    MoveTo(TikzCoord),
    LineTo(TikzCoord, Option<InlinePathNode>),
    Cycle,
    Circle(f64),
    Ellipse(f64, f64),
    Rectangle(TikzCoord),
    Arc {
        start_deg: f64,
        end_deg: f64,
        rx: f64,
        ry: f64,
    },
    Grid(TikzCoord),
    PlotExpr {
        expr: String,
        domain: (f64, f64),
        samples: usize,
    },
    PlotCoords(Vec<(f64, f64)>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TikzStmt {
    Coordinate {
        name: String,
        coord: TikzCoord,
    },
    Node {
        name: Option<String>,
        coord: TikzCoord,
        style: TikzStyle,
        label: String,
    },
    DrawPath {
        style: TikzStyle,
        segments: Vec<PathSegment>,
    },
    EmbeddedAxis(AxisPlot),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TikzPicture {
    pub scale: f32,
    pub stmts: Vec<TikzStmt>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AddPlotSource {
    Expression(String),
    Coordinates(Vec<(f64, f64)>),
    Surface3D(String),
    Parametric3D(String, String, String),
    Coordinates3D(Vec<(f64, f64, f64)>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AddPlotSpec {
    pub style: TikzStyle,
    pub source: AddPlotSource,
}

impl AddPlotSpec {
    pub fn is_3d(&self) -> bool {
        matches!(
            self.source,
            AddPlotSource::Surface3D(_)
                | AddPlotSource::Parametric3D(..)
                | AddPlotSource::Coordinates3D(_)
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AxisPlot {
    pub title: Option<String>,
    pub xlabel: Option<String>,
    pub ylabel: Option<String>,
    pub zlabel: Option<String>,
    pub xmin: Option<f64>,
    pub xmax: Option<f64>,
    pub ymin: Option<f64>,
    pub ymax: Option<f64>,
    pub zmin: Option<f64>,
    pub zmax: Option<f64>,
    pub grid: bool,
    pub axis_lines: AxisLinesMode,
    pub domain: (f64, f64),
    pub y_domain: Option<(f64, f64)>,
    pub samples: usize,
    pub samples_y: Option<usize>,
    pub view: Option<(f64, f64)>,
    pub colormap: ColormapKind,
    pub ybar: bool,
    pub plots: Vec<AddPlotSpec>,
    pub legend_entries: Vec<String>,
    pub overlays: Vec<TikzStmt>,
}

fn named_tikz_color(name: &str) -> Option<Color32> {
    match name.trim().to_ascii_lowercase().as_str() {
        "black" => Some(Color32::from_rgb(25, 28, 34)),
        "white" => Some(Color32::WHITE),
        "red" => Some(Color32::from_rgb(220, 48, 56)),
        "blue" => Some(Color32::from_rgb(24, 110, 214)),
        "green" => Some(Color32::from_rgb(34, 154, 72)),
        "orange" => Some(Color32::from_rgb(235, 118, 22)),
        "purple" | "violet" => Some(Color32::from_rgb(128, 62, 200)),
        "cyan" => Some(Color32::from_rgb(0, 168, 206)),
        "magenta" => Some(Color32::from_rgb(212, 42, 134)),
        "teal" => Some(Color32::from_rgb(0, 140, 132)),
        "brown" => Some(Color32::from_rgb(138, 86, 48)),
        "yellow" | "amber" => Some(Color32::from_rgb(220, 168, 12)),
        "olive" => Some(Color32::from_rgb(112, 130, 42)),
        "pink" => Some(Color32::from_rgb(232, 108, 152)),
        "gray" | "grey" => Some(Color32::from_rgb(130, 136, 146)),
        "darkgray" | "darkgrey" => Some(Color32::from_rgb(76, 82, 92)),
        "lightgray" | "lightgrey" => Some(Color32::from_rgb(205, 210, 218)),
        _ => None,
    }
}

fn parse_tikz_color(raw: &str) -> Option<Color32> {
    let parts: Vec<&str> = raw.split('!').collect();
    if parts.len() == 1 {
        return named_tikz_color(parts[0]);
    }
    let c1 = named_tikz_color(parts[0])?;
    let pct = parts[1].trim().parse::<f32>().ok()?.clamp(0.0, 100.0) / 100.0;
    let c2 = if parts.len() >= 3 {
        named_tikz_color(parts[2]).unwrap_or(Color32::WHITE)
    } else {
        Color32::WHITE
    };
    let lerp = |a: u8, b: u8| -> u8 { ((a as f32) * pct + (b as f32) * (1.0 - pct)).round() as u8 };
    Some(Color32::from_rgb(
        lerp(c1.r(), c2.r()),
        lerp(c1.g(), c2.g()),
        lerp(c1.b(), c2.b()),
    ))
}

fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut brace = 0usize;
    let mut paren = 0usize;
    for c in s.chars() {
        match c {
            '{' => {
                brace += 1;
                cur.push(c);
            }
            '}' => {
                brace = brace.saturating_sub(1);
                cur.push(c);
            }
            '(' => {
                paren += 1;
                cur.push(c);
            }
            ')' => {
                paren = paren.saturating_sub(1);
                cur.push(c);
            }
            ',' if brace == 0 && paren == 0 => {
                let t = cur.trim().to_string();
                if !t.is_empty() {
                    out.push(t);
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    let t = cur.trim().to_string();
    if !t.is_empty() {
        out.push(t);
    }
    out
}

fn strip_outer_braces(s: &str) -> &str {
    let t = s.trim();
    if t.starts_with('{') && t.ends_with('}') && t.len() >= 2 {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

fn parse_length_num(raw: &str) -> Option<f64> {
    let s = raw.trim();
    for (suffix, scale) in [("cm", 1.0), ("mm", 0.1), ("pt", 1.0)] {
        if let Some(num) = s.strip_suffix(suffix) {
            return eval_const_expr(num.trim()).map(|v| v * scale);
        }
    }
    eval_const_expr(s)
}

fn parse_tikz_style(opts_str: &str, default_draw: bool, default_fill: bool) -> TikzStyle {
    let mut st = TikzStyle {
        draw_disabled: !default_draw,
        ..Default::default()
    };
    let mut explicit_fill_opacity = false;
    let mut saw_bare_color = None;

    for raw_opt in split_top_level_commas(opts_str) {
        let opt = raw_opt.trim();
        if opt.is_empty() {
            continue;
        }
        if let Some((k_raw, v_raw)) = opt.split_once('=') {
            let k = k_raw.trim().to_ascii_lowercase();
            let v = strip_outer_braces(v_raw).trim();
            match k.as_str() {
                "color" => {
                    if let Some(col) = parse_tikz_color(v) {
                        st.draw_color = Some(col);
                        st.text_color = Some(col);
                    }
                }
                "draw" => {
                    if v.eq_ignore_ascii_case("none") {
                        st.draw_disabled = true;
                        st.draw_color = None;
                    } else {
                        st.draw_disabled = false;
                        if let Some(col) = parse_tikz_color(v) {
                            st.draw_color = Some(col);
                        }
                    }
                }
                "fill" => {
                    if v.eq_ignore_ascii_case("none") {
                        st.fill_color = None;
                    } else if let Some(col) = parse_tikz_color(v) {
                        st.fill_color = Some(col);
                    }
                }
                "fill opacity" => {
                    if let Ok(op) = v.parse::<f32>() {
                        st.fill_opacity = op.clamp(0.0, 1.0);
                        explicit_fill_opacity = true;
                    }
                }
                "opacity" => {
                    if let Ok(op) = v.parse::<f32>() {
                        st.fill_opacity = op.clamp(0.0, 1.0);
                        explicit_fill_opacity = true;
                    }
                }
                "text" => {
                    if let Some(col) = parse_tikz_color(v) {
                        st.text_color = Some(col);
                    }
                }
                "line width" => {
                    if let Some(w) = parse_length_num(v) {
                        st.stroke_width = (w as f32).clamp(0.5, 12.0);
                    }
                }
                "pos" => {
                    if let Ok(p) = v.parse::<f32>() {
                        st.node_pos = Some(p.clamp(0.0, 1.0));
                    }
                }
                "mark" => {
                    st.mark = match v {
                        "*" | "circle*" => PlotMark::FilledCircle,
                        "o" | "circle" => PlotMark::OpenCircle,
                        "square*" | "square" => PlotMark::Square,
                        _ => PlotMark::None,
                    };
                }
                "domain" => {
                    if let Some((a_s, b_s)) = v.split_once(':')
                        && let (Some(a), Some(b)) = (eval_const_expr(a_s), eval_const_expr(b_s))
                        && b > a
                    {
                        st.domain = Some((a, b));
                    }
                }
                "y domain" | "domain y" => {
                    if let Some((a_s, b_s)) = v.split_once(':')
                        && let (Some(a), Some(b)) = (eval_const_expr(a_s), eval_const_expr(b_s))
                        && b > a
                    {
                        st.y_domain = Some((a, b));
                    }
                }
                "samples" => {
                    if let Ok(n) = v.parse::<usize>() {
                        st.samples = Some(n.clamp(6, 400));
                    }
                }
                "samples y" => {
                    if let Ok(n) = v.parse::<usize>() {
                        st.samples_y = Some(n.clamp(6, 64));
                    }
                }
                "shader" => {
                    st.shader_flat = v.eq_ignore_ascii_case("flat");
                }
                "step" => {
                    if let Some(sv) = parse_length_num(v)
                        && sv > 0.05
                    {
                        st.step = Some(sv);
                    }
                }
                _ => {
                    if let Some(cm) = parse_colormap_name(&k) {
                        st.colormap = Some(cm);
                    }
                }
            }
        } else {
            let low = opt.to_ascii_lowercase();
            match low.as_str() {
                "surf" => st.surf = true,
                "mesh" => st.mesh = true,
                "draw" => st.draw_disabled = false,
                "fill" => {
                    if st.fill_color.is_none() {
                        st.fill_color = Some(Color32::from_rgb(24, 110, 214));
                    }
                }
                "ultra thin" => st.stroke_width = 0.7,
                "very thin" => st.stroke_width = 0.9,
                "thin" => st.stroke_width = 1.15,
                "semithick" => st.stroke_width = 1.6,
                "thick" => st.stroke_width = 2.1,
                "very thick" => st.stroke_width = 2.8,
                "ultra thick" => st.stroke_width = 3.5,
                "dashed" | "densely dashed" | "loosely dashed" => st.dash = MathLineDash::Dashed,
                "dotted" | "densely dotted" | "loosely dotted" => st.dash = MathLineDash::Dotted,
                "solid" => st.dash = MathLineDash::Solid,
                "->" | "-latex" | "-stealth" | "-to" | "->>" => st.arrow_end = true,
                "<-" | "latex-" | "stealth-" | "to-" => st.arrow_start = true,
                "<->" | "latex-latex" | "stealth-stealth" => {
                    st.arrow_start = true;
                    st.arrow_end = true;
                }
                "above" => st.node_anchor = NodeAnchor::Above,
                "below" => st.node_anchor = NodeAnchor::Below,
                "left" => st.node_anchor = NodeAnchor::Left,
                "right" => st.node_anchor = NodeAnchor::Right,
                "above left" => st.node_anchor = NodeAnchor::AboveLeft,
                "above right" => st.node_anchor = NodeAnchor::AboveRight,
                "below left" => st.node_anchor = NodeAnchor::BelowLeft,
                "below right" => st.node_anchor = NodeAnchor::BelowRight,
                "midway" => st.node_pos = Some(0.5),
                "circle" => st.node_shape = NodeShape::Circle,
                "rectangle" => st.node_shape = NodeShape::Rectangle,
                "only marks" => st.only_marks = true,
                "ybar" => st.ybar = true,
                _ => {
                    if let Some(cm) = parse_colormap_name(&low) {
                        st.colormap = Some(cm);
                    } else if let Some(col) = parse_tikz_color(opt) {
                        saw_bare_color = Some(col);
                    }
                }
            }
        }
    }

    if let Some(col) = saw_bare_color {
        if default_fill && st.fill_color.is_none() {
            st.fill_color = Some(col);
        }
        if !st.draw_disabled && st.draw_color.is_none() {
            st.draw_color = Some(col);
        }
        if st.text_color.is_none() {
            st.text_color = Some(col);
        }
    }
    if default_fill && st.fill_color.is_none() {
        st.fill_color = st.draw_color.or(Some(Color32::from_rgb(24, 110, 214)));
    }
    if !explicit_fill_opacity && st.fill_color.is_some() && !default_fill {
        st.fill_opacity = 0.28;
    } else if !explicit_fill_opacity && default_fill && default_draw {
        st.fill_opacity = 0.25;
    }
    st
}

fn split_semicolon_stmts(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut i = 0;
    let mut brace = 0usize;
    let mut bracket = 0usize;
    let mut paren = 0usize;
    let mut axis_depth = 0usize;

    while i < body.len() {
        if body[i..].starts_with(r"\begin{axis}") {
            let trimmed_cur = cur.trim();
            if !trimmed_cur.is_empty() && brace == 0 && paren == 0 {
                out.push(std::mem::take(&mut cur));
            }
            axis_depth += 1;
        } else if body[i..].starts_with(r"\end{axis}") {
            let len = r"\end{axis}".len();
            cur.push_str(&body[i..i + len]);
            i += len;
            axis_depth = axis_depth.saturating_sub(1);
            if axis_depth == 0 {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        let ch = body[i..].chars().next().unwrap();
        let ch_len = ch.len_utf8();
        match ch {
            '{' => brace += 1,
            '}' => brace = brace.saturating_sub(1),
            '[' => bracket += 1,
            ']' => bracket = bracket.saturating_sub(1),
            '(' => paren += 1,
            ')' => paren = paren.saturating_sub(1),
            ';' if brace == 0 && bracket == 0 && paren == 0 && axis_depth == 0 => {
                let t = cur.trim().to_string();
                if !t.is_empty() {
                    out.push(t);
                }
                cur.clear();
                i += 1;
                continue;
            }
            _ => {}
        }
        cur.push(ch);
        i += ch_len;
    }
    let t = cur.trim().to_string();
    if !t.is_empty() {
        out.push(t);
    }
    out
}

fn extract_paren_group(s: &str, start: usize) -> Option<(String, usize)> {
    let rest = s.get(start..)?;
    if !rest.starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    let mut content = String::new();
    for (idx, ch) in rest.char_indices() {
        match ch {
            '(' => {
                if depth > 0 {
                    content.push(ch);
                }
                depth += 1;
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((content, idx + ch.len_utf8()));
                }
                content.push(ch);
            }
            _ => content.push(ch),
        }
    }
    None
}

fn parse_coord_str(inner: &str) -> Result<TikzCoord, MathParseError> {
    let trimmed = inner
        .trim()
        .strip_prefix("axis cs:")
        .unwrap_or(inner.trim())
        .trim();
    if let Some((a, b)) = trimmed.split_once(',') {
        let x = eval_const_expr(a.trim()).ok_or_else(|| MathParseError {
            message: format!("Koordinat X tidak valid: '{}'", a.trim()),
            position: 0,
        })?;
        let y = eval_const_expr(b.trim()).ok_or_else(|| MathParseError {
            message: format!("Koordinat Y tidak valid: '{}'", b.trim()),
            position: 0,
        })?;
        Ok(TikzCoord::Point(x, y))
    } else if let Some((deg_s, rad_s)) = trimmed.split_once(':') {
        let deg = eval_const_expr(deg_s.trim()).ok_or_else(|| MathParseError {
            message: format!("Sudut polar tidak valid: '{}'", deg_s.trim()),
            position: 0,
        })?;
        let r = parse_length_num(rad_s.trim()).ok_or_else(|| MathParseError {
            message: format!("Radius polar tidak valid: '{}'", rad_s.trim()),
            position: 0,
        })?;
        Ok(TikzCoord::Polar(deg, r))
    } else if !trimmed.is_empty() {
        Ok(TikzCoord::Named(trimmed.to_string()))
    } else {
        Err(MathParseError {
            message: "Koordinat kosong".into(),
            position: 0,
        })
    }
}

fn skip_ws(s: &str, mut i: usize) -> usize {
    while i < s.len() {
        let ch = s[i..].chars().next().unwrap();
        if ch.is_whitespace() {
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    i
}

fn parse_node_or_coord_stmt(stmt: &str, is_coord: bool) -> Result<TikzStmt, MathParseError> {
    let mut i = if is_coord {
        r"\coordinate".len()
    } else {
        r"\node".len()
    };
    let mut style = TikzStyle::default();
    let mut name: Option<String> = None;
    let mut coord: Option<TikzCoord> = None;
    let mut label = String::new();

    while i < stmt.len() {
        i = skip_ws(stmt, i);
        if i >= stmt.len() {
            break;
        }
        let rest = &stmt[i..];
        if rest.starts_with('[') {
            if let Some((opts, adv)) = extract_bracket_group(stmt, i) {
                style = parse_tikz_style(&opts, false, false);
                i += adv;
                continue;
            }
        } else if let Some(after_at) = rest.strip_prefix("at")
            && after_at.starts_with(|c: char| c.is_whitespace() || c == '(')
        {
            i += 2;
            i = skip_ws(stmt, i);
            if let Some((c_str, adv)) = extract_paren_group(stmt, i) {
                coord = Some(parse_coord_str(&c_str)?);
                i += adv;
                continue;
            }
        } else if rest.starts_with('(') {
            if let Some((p_str, adv)) = extract_paren_group(stmt, i) {
                if p_str.contains(',') || p_str.contains(':') {
                    coord = Some(parse_coord_str(&p_str)?);
                } else {
                    name = Some(p_str.trim().to_string());
                }
                i += adv;
                continue;
            }
        } else if rest.starts_with('{')
            && let Some((lbl, adv)) = extract_braced_group(stmt, i)
        {
            label = lbl.trim().to_string();
            i += adv;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        i += ch.len_utf8();
    }

    let final_coord = coord.unwrap_or(TikzCoord::Point(0.0, 0.0));
    if is_coord {
        Ok(TikzStmt::Coordinate {
            name: name.unwrap_or_else(|| "P".into()),
            coord: final_coord,
        })
    } else {
        Ok(TikzStmt::Node {
            name,
            coord: final_coord,
            style,
            label,
        })
    }
}

fn parse_draw_path_stmt(
    stmt: &str,
    default_draw: bool,
    default_fill: bool,
) -> Result<TikzStmt, MathParseError> {
    let cmd_len = if stmt.starts_with(r"\filldraw") {
        r"\filldraw".len()
    } else if stmt.starts_with(r"\draw") {
        r"\draw".len()
    } else if stmt.starts_with(r"\fill") {
        r"\fill".len()
    } else {
        r"\path".len()
    };

    let mut i = skip_ws(stmt, cmd_len);
    let mut style = if default_draw || default_fill {
        parse_tikz_style("", default_draw, default_fill)
    } else {
        TikzStyle::default()
    };

    if i < stmt.len()
        && stmt[i..].starts_with('[')
        && let Some((opts, adv)) = extract_bracket_group(stmt, i)
    {
        style = parse_tikz_style(&opts, default_draw, default_fill);
        i += adv;
    }

    let mut segments = Vec::new();
    let mut pending_line_to = false;
    let mut pending_inline_node: Option<InlinePathNode> = None;

    while i < stmt.len() {
        i = skip_ws(stmt, i);
        if i >= stmt.len() {
            break;
        }
        let rest = &stmt[i..];

        if rest.starts_with("--") {
            pending_line_to = true;
            i += 2;
            continue;
        }

        if let Some(after_kw) = rest.strip_prefix("cycle")
            && after_kw.chars().next().is_none_or(|c| !c.is_alphanumeric())
        {
            segments.push(PathSegment::Cycle);
            i += "cycle".len();
            continue;
        }

        if let Some(after_kw) = rest.strip_prefix("node")
            && after_kw
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '[' || c == '{' || c == '(')
        {
            i += 4;
            let mut n_style = TikzStyle::default();
            let mut n_name = None;
            let mut n_label = String::new();
            loop {
                i = skip_ws(stmt, i);
                let r2 = &stmt[i..];
                if r2.starts_with('[')
                    && let Some((opts, adv)) = extract_bracket_group(stmt, i)
                {
                    n_style = parse_tikz_style(&opts, false, false);
                    i += adv;
                } else if r2.starts_with('(')
                    && let Some((nm, adv)) = extract_paren_group(stmt, i)
                {
                    n_name = Some(nm.trim().to_string());
                    i += adv;
                } else {
                    break;
                }
            }
            i = skip_ws(stmt, i);
            if i < stmt.len()
                && stmt[i..].starts_with('{')
                && let Some((lbl, adv)) = extract_braced_group(stmt, i)
            {
                n_label = lbl.trim().to_string();
                i += adv;
            }
            let inode = InlinePathNode {
                style: n_style,
                label: n_label,
                name: n_name,
            };
            if let Some(PathSegment::LineTo(_, slot)) = segments.last_mut() {
                *slot = Some(inode);
            } else {
                pending_inline_node = Some(inode);
            }
            continue;
        }

        if let Some(after_kw) = rest.strip_prefix("circle")
            && after_kw
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '(' || c == '[')
        {
            i = skip_ws(stmt, i + "circle".len());
            if let Some((r_str, adv)) = extract_paren_group(stmt, i) {
                let r = parse_length_num(&r_str).ok_or_else(|| MathParseError {
                    message: format!("Radius lingkaran tidak valid: '{r_str}'"),
                    position: i,
                })?;
                segments.push(PathSegment::Circle(r.abs()));
                i += adv;
                continue;
            } else if let Some((opts, adv)) = extract_bracket_group(stmt, i) {
                let r = opts
                    .split('=')
                    .nth(1)
                    .and_then(parse_length_num)
                    .unwrap_or(1.0);
                segments.push(PathSegment::Circle(r.abs()));
                i += adv;
                continue;
            }
        }

        if let Some(after_kw) = rest.strip_prefix("ellipse")
            && after_kw
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '(' || c == '[')
        {
            i = skip_ws(stmt, i + "ellipse".len());
            if let Some((r_str, adv)) = extract_paren_group(stmt, i) {
                let parts: Vec<&str> = r_str.split("and").collect();
                if parts.len() == 2
                    && let (Some(rx), Some(ry)) =
                        (parse_length_num(parts[0]), parse_length_num(parts[1]))
                {
                    segments.push(PathSegment::Ellipse(rx.abs(), ry.abs()));
                }
                i += adv;
                continue;
            } else if let Some((opts, adv)) = extract_bracket_group(stmt, i) {
                let mut rx = 1.0;
                let mut ry = 0.6;
                for p in opts.split(',') {
                    if let Some((k, v)) = p.split_once('=') {
                        if k.contains('x') {
                            rx = parse_length_num(v).unwrap_or(rx);
                        } else if k.contains('y') {
                            ry = parse_length_num(v).unwrap_or(ry);
                        }
                    }
                }
                segments.push(PathSegment::Ellipse(rx.abs(), ry.abs()));
                i += adv;
                continue;
            }
        }

        if let Some(after_kw) = rest.strip_prefix("rectangle")
            && after_kw
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '(')
        {
            i = skip_ws(stmt, i + "rectangle".len());
            if let Some((c_str, adv)) = extract_paren_group(stmt, i) {
                let coord = parse_coord_str(&c_str)?;
                segments.push(PathSegment::Rectangle(coord));
                i += adv;
                continue;
            }
        }

        if let Some(after_kw) = rest.strip_prefix("arc")
            && after_kw
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '(' || c == '[')
        {
            i = skip_ws(stmt, i + "arc".len());
            if let Some((arc_str, adv)) = extract_paren_group(stmt, i) {
                let parts: Vec<&str> = arc_str.split(':').collect();
                if parts.len() == 3 {
                    let start_deg = eval_const_expr(parts[0]).unwrap_or(0.0);
                    let end_deg = eval_const_expr(parts[1]).unwrap_or(90.0);
                    let (rx, ry) = if let Some((rx_s, ry_s)) = parts[2].split_once("and") {
                        (
                            parse_length_num(rx_s).unwrap_or(1.0).abs(),
                            parse_length_num(ry_s).unwrap_or(1.0).abs(),
                        )
                    } else {
                        let r = parse_length_num(parts[2]).unwrap_or(1.0).abs();
                        (r, r)
                    };
                    segments.push(PathSegment::Arc {
                        start_deg,
                        end_deg,
                        rx,
                        ry,
                    });
                }
                i += adv;
                continue;
            } else if let Some((opts, adv)) = extract_bracket_group(stmt, i) {
                let mut start_deg = 0.0;
                let mut end_deg = 90.0;
                let mut rx = 1.0;
                let mut ry = 1.0;
                for p in opts.split(',') {
                    if let Some((k, v)) = p.split_once('=') {
                        let kl = k.trim().to_ascii_lowercase();
                        if kl.contains("start") {
                            start_deg = eval_const_expr(v).unwrap_or(start_deg);
                        } else if kl.contains("end") {
                            end_deg = eval_const_expr(v).unwrap_or(end_deg);
                        } else if kl.contains("x radius") {
                            rx = parse_length_num(v).unwrap_or(rx).abs();
                        } else if kl.contains("y radius") {
                            ry = parse_length_num(v).unwrap_or(ry).abs();
                        } else if kl.contains("radius") {
                            let r = parse_length_num(v).unwrap_or(1.0).abs();
                            rx = r;
                            ry = r;
                        }
                    }
                }
                segments.push(PathSegment::Arc {
                    start_deg,
                    end_deg,
                    rx,
                    ry,
                });
                i += adv;
                continue;
            }
        }

        if let Some(after_kw) = rest.strip_prefix("grid")
            && after_kw
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '(' || c == '[')
        {
            i = skip_ws(stmt, i + "grid".len());
            if stmt[i..].starts_with('[')
                && let Some((g_opts, adv)) = extract_bracket_group(stmt, i)
            {
                let parsed_g = parse_tikz_style(&g_opts, true, false);
                if parsed_g.step.is_some() {
                    style.step = parsed_g.step;
                }
                i = skip_ws(stmt, i + adv);
            }
            if let Some((c_str, adv)) = extract_paren_group(stmt, i) {
                let coord = parse_coord_str(&c_str)?;
                segments.push(PathSegment::Grid(coord));
                i += adv;
                continue;
            }
        }

        if let Some(after_kw) = rest.strip_prefix("plot")
            && after_kw
                .chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '[' || c == '(' || c == '{')
        {
            i = skip_ws(stmt, i + "plot".len());
            let mut domain = style.domain.unwrap_or((-3.0, 3.0));
            let mut samples = style.samples.unwrap_or(60);
            if stmt[i..].starts_with('[')
                && let Some((p_opts, adv)) = extract_bracket_group(stmt, i)
            {
                let ps = parse_tikz_style(&p_opts, true, false);
                if let Some(d) = ps.domain {
                    domain = d;
                }
                if let Some(sp) = ps.samples {
                    samples = sp;
                }
                i = skip_ws(stmt, i + adv);
            }
            if stmt[i..].starts_with("coordinates") {
                i = skip_ws(stmt, i + "coordinates".len());
                if let Some((coords_str, adv)) = extract_braced_group(stmt, i) {
                    let pts = parse_coordinate_pairs(&coords_str)?;
                    segments.push(PathSegment::PlotCoords(pts));
                    i += adv;
                    continue;
                }
            } else if let Some((tuple_str, adv)) = extract_paren_group(stmt, i) {
                let expr = tuple_str
                    .split_once(',')
                    .map(|(_, y)| strip_outer_braces(y.trim()).to_string())
                    .unwrap_or(tuple_str);
                segments.push(PathSegment::PlotExpr {
                    expr,
                    domain,
                    samples,
                });
                i += adv;
                continue;
            }
        }

        if rest.starts_with('(')
            && let Some((c_str, adv)) = extract_paren_group(stmt, i)
        {
            let coord = parse_coord_str(&c_str)?;
            if pending_line_to || (!segments.is_empty() && pending_inline_node.is_some()) {
                segments.push(PathSegment::LineTo(coord, pending_inline_node.take()));
                pending_line_to = false;
            } else {
                segments.push(PathSegment::MoveTo(coord));
            }
            i += adv;
            continue;
        }

        let ch = rest.chars().next().unwrap();
        i += ch.len_utf8();
    }

    Ok(TikzStmt::DrawPath { style, segments })
}

fn parse_coordinate_pairs(raw: &str) -> Result<Vec<(f64, f64)>, MathParseError> {
    let mut pts = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        i = skip_ws(raw, i);
        if i >= raw.len() {
            break;
        }
        if raw[i..].starts_with('(')
            && let Some((inner, adv)) = extract_paren_group(raw, i)
        {
            if let Some((a, b)) = inner.split_once(',')
                && let (Some(x), Some(y)) = (eval_const_expr(a.trim()), eval_const_expr(b.trim()))
            {
                pts.push((x, y));
            }
            i += adv;
            continue;
        }
        let ch = raw[i..].chars().next().unwrap();
        i += ch.len_utf8();
    }
    Ok(pts)
}

fn parse_coordinate_triplets(raw: &str) -> Result<Vec<(f64, f64, f64)>, MathParseError> {
    let mut pts = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        i = skip_ws(raw, i);
        if i >= raw.len() {
            break;
        }
        if raw[i..].starts_with('(')
            && let Some((inner, adv)) = extract_paren_group(raw, i)
        {
            let parts: Vec<&str> = inner.split(',').collect();
            if parts.len() >= 3
                && let (Some(x), Some(y), Some(z)) = (
                    eval_const_expr(parts[0].trim()),
                    eval_const_expr(parts[1].trim()),
                    eval_const_expr(parts[2].trim()),
                )
            {
                pts.push((x, y, z));
            }
            i += adv;
            continue;
        }
        let ch = raw[i..].chars().next().unwrap();
        i += ch.len_utf8();
    }
    Ok(pts)
}

fn parse_view_angles(raw: &str) -> Option<(f64, f64)> {
    let normalized = raw
        .replace("}{", ",")
        .replace(['{', '}', ';'], " ")
        .trim()
        .to_string();
    let nums: Vec<f64> = normalized
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(eval_const_expr)
        .collect();
    if nums.len() >= 2 {
        Some((nums[0], nums[1]))
    } else {
        None
    }
}

fn parse_tikz_statements(stmts_raw: Vec<String>) -> Result<Vec<TikzStmt>, MathParseError> {
    let mut stmts = Vec::new();
    for s_raw in stmts_raw {
        let s = s_raw.trim();
        if s.is_empty() {
            continue;
        }
        if let Some(after_axis) = s.strip_prefix(r"\begin{axis}") {
            let mut rest = after_axis;
            let opts = if rest.trim_start().starts_with('[') {
                let trim_r = rest.trim_start();
                if let Some((o, adv)) = extract_bracket_group(trim_r, 0) {
                    rest = &trim_r[adv..];
                    Some(o)
                } else {
                    None
                }
            } else {
                None
            };
            let body = rest.trim_end().strip_suffix(r"\end{axis}").unwrap_or(rest);
            let ax = parse_axis_env(opts.as_deref(), body)?;
            stmts.push(TikzStmt::EmbeddedAxis(ax));
        } else if s.starts_with(r"\coordinate") {
            stmts.push(parse_node_or_coord_stmt(s, true)?);
        } else if s.starts_with(r"\node") {
            stmts.push(parse_node_or_coord_stmt(s, false)?);
        } else if s.starts_with(r"\draw") {
            stmts.push(parse_draw_path_stmt(s, true, false)?);
        } else if s.starts_with(r"\filldraw") {
            stmts.push(parse_draw_path_stmt(s, true, true)?);
        } else if s.starts_with(r"\fill") {
            stmts.push(parse_draw_path_stmt(s, false, true)?);
        } else if s.starts_with(r"\path") {
            stmts.push(parse_draw_path_stmt(s, false, false)?);
        }
    }
    Ok(stmts)
}

/// Mem-parse lingkungan `\begin{tikzpicture}[opts] ... \end{tikzpicture}`.
pub fn parse_tikzpicture_env(
    opts: Option<&str>,
    body: &str,
) -> Result<TikzPicture, MathParseError> {
    let clean = strip_latex_comments(body);
    let mut scale = 1.0_f32;
    if let Some(o_str) = opts {
        for kv in split_top_level_commas(o_str) {
            if let Some((k, v)) = kv.split_once('=')
                && k.trim().eq_ignore_ascii_case("scale")
                && let Ok(sc) = strip_outer_braces(v).trim().parse::<f32>()
            {
                scale = sc.clamp(0.25, 4.0);
            }
        }
    }
    let stmts = parse_tikz_statements(split_semicolon_stmts(&clean))?;
    Ok(TikzPicture { scale, stmts })
}

/// Mem-parse lingkungan grafik PGFPlots `\begin{axis}[opts] ... \end{axis}`.
pub fn parse_axis_env(opts: Option<&str>, body: &str) -> Result<AxisPlot, MathParseError> {
    let clean = strip_latex_comments(body);
    let mut plot = AxisPlot {
        title: None,
        xlabel: None,
        ylabel: None,
        zlabel: None,
        xmin: None,
        xmax: None,
        ymin: None,
        ymax: None,
        zmin: None,
        zmax: None,
        grid: false,
        axis_lines: AxisLinesMode::Box,
        domain: (-5.0, 5.0),
        y_domain: None,
        samples: 80,
        samples_y: None,
        view: None,
        colormap: ColormapKind::default(),
        ybar: false,
        plots: Vec::new(),
        legend_entries: Vec::new(),
        overlays: Vec::new(),
    };

    if let Some(o_str) = opts {
        for kv in split_top_level_commas(o_str) {
            let trimmed = kv.trim();
            if let Some((k_raw, v_raw)) = trimmed.split_once('=') {
                let k = k_raw.trim().to_ascii_lowercase();
                let v = strip_outer_braces(v_raw).trim();
                match k.as_str() {
                    "title" => plot.title = Some(v.to_string()),
                    "xlabel" => plot.xlabel = Some(v.to_string()),
                    "ylabel" => plot.ylabel = Some(v.to_string()),
                    "zlabel" => plot.zlabel = Some(v.to_string()),
                    "xmin" => plot.xmin = eval_const_expr(v),
                    "xmax" => plot.xmax = eval_const_expr(v),
                    "ymin" => plot.ymin = eval_const_expr(v),
                    "ymax" => plot.ymax = eval_const_expr(v),
                    "zmin" => plot.zmin = eval_const_expr(v),
                    "zmax" => plot.zmax = eval_const_expr(v),
                    "grid" => plot.grid = !v.eq_ignore_ascii_case("none"),
                    "axis lines" => {
                        plot.axis_lines = match v.to_ascii_lowercase().as_str() {
                            "middle" | "center" => AxisLinesMode::Middle,
                            "left" => AxisLinesMode::Left,
                            _ => AxisLinesMode::Box,
                        };
                    }
                    "domain" => {
                        if let Some((a_s, b_s)) = v.split_once(':')
                            && let (Some(a), Some(b)) = (eval_const_expr(a_s), eval_const_expr(b_s))
                            && b > a
                        {
                            plot.domain = (a, b);
                        }
                    }
                    "y domain" | "domain y" => {
                        if let Some((a_s, b_s)) = v.split_once(':')
                            && let (Some(a), Some(b)) = (eval_const_expr(a_s), eval_const_expr(b_s))
                            && b > a
                        {
                            plot.y_domain = Some((a, b));
                        }
                    }
                    "samples" => {
                        if let Ok(n) = v.parse::<usize>() {
                            plot.samples = n.clamp(6, 400);
                        }
                    }
                    "samples y" => {
                        if let Ok(n) = v.parse::<usize>() {
                            plot.samples_y = Some(n.clamp(6, 64));
                        }
                    }
                    "view" => {
                        if let Some(angles) = parse_view_angles(v_raw) {
                            plot.view = Some(angles);
                        }
                    }
                    "colormap" | "colormap name" => {
                        if let Some(cm) = parse_colormap_name(v) {
                            plot.colormap = cm;
                        }
                    }
                    _ => {
                        if let Some(cm) = parse_colormap_name(&k) {
                            plot.colormap = cm;
                        }
                    }
                }
            } else if trimmed.eq_ignore_ascii_case("ybar") {
                plot.ybar = true;
            } else if let Some(cm) = parse_colormap_name(trimmed) {
                plot.colormap = cm;
            }
        }
    }

    let palette = [
        Color32::from_rgb(24, 110, 214),
        Color32::from_rgb(220, 48, 56),
        Color32::from_rgb(34, 154, 72),
        Color32::from_rgb(235, 118, 22),
        Color32::from_rgb(128, 62, 200),
    ];

    for raw_stmt in split_semicolon_stmts(&clean) {
        let s = raw_stmt.trim();
        let is_addplot3 = s.starts_with(r"\addplot3");
        if is_addplot3 || s.starts_with(r"\addplot") {
            let mut i = if is_addplot3 {
                r"\addplot3".len()
            } else {
                r"\addplot".len()
            };
            if s[i..].starts_with('+') {
                i += 1;
            }
            i = skip_ws(s, i);
            let default_col = palette[plot.plots.len() % palette.len()];
            let mut style = TikzStyle {
                draw_color: Some(default_col),
                stroke_width: 2.0,
                ..Default::default()
            };
            if i < s.len()
                && s[i..].starts_with('[')
                && let Some((opts_s, adv)) = extract_bracket_group(s, i)
            {
                style = parse_tikz_style(&opts_s, true, false);
                if style.draw_color.is_none() && !style.draw_disabled {
                    style.draw_color = Some(default_col);
                }
                if opts_s.trim().is_empty()
                    || !opts_s.contains("thick")
                        && !opts_s.contains("thin")
                        && !opts_s.contains("line width")
                {
                    style.stroke_width = 2.0;
                }
                i = skip_ws(s, i + adv);
            }
            if plot.ybar {
                style.ybar = true;
            }
            if style.ybar && style.fill_color.is_none() {
                style.fill_color = style.draw_color.or(Some(default_col));
            }
            if let Some(after_kw) = s[i..].strip_prefix("expression") {
                let offset = s.len() - after_kw.len();
                i = skip_ws(s, offset);
            }

            if is_addplot3 {
                if i < s.len() && s[i..].starts_with("coordinates") {
                    i = skip_ws(s, i + "coordinates".len());
                    if let Some((coords_str, _)) = extract_braced_group(s, i) {
                        let pts = parse_coordinate_triplets(&coords_str)?;
                        plot.plots.push(AddPlotSpec {
                            style,
                            source: AddPlotSource::Coordinates3D(pts),
                        });
                    }
                } else if i < s.len() && s[i..].starts_with('(') {
                    if let Some((tuple_str, _)) = extract_paren_group(s, i) {
                        let parts = split_top_level_commas(&tuple_str);
                        if parts.len() >= 3 {
                            plot.plots.push(AddPlotSpec {
                                style,
                                source: AddPlotSource::Parametric3D(
                                    strip_outer_braces(parts[0].trim()).to_string(),
                                    strip_outer_braces(parts[1].trim()).to_string(),
                                    strip_outer_braces(parts[2].trim()).to_string(),
                                ),
                            });
                        }
                    }
                } else {
                    if !style.surf && !style.mesh && !style.only_marks {
                        style.surf = true;
                    }
                    if i < s.len() && s[i..].starts_with('{') {
                        if let Some((expr_str, _)) = extract_braced_group(s, i) {
                            plot.plots.push(AddPlotSpec {
                                style,
                                source: AddPlotSource::Surface3D(expr_str),
                            });
                        }
                    } else {
                        let expr_str = s[i..].trim().to_string();
                        if !expr_str.is_empty() {
                            plot.plots.push(AddPlotSpec {
                                style,
                                source: AddPlotSource::Surface3D(expr_str),
                            });
                        }
                    }
                }
            } else if i < s.len() && s[i..].starts_with("coordinates") {
                i = skip_ws(s, i + "coordinates".len());
                if let Some((coords_str, _)) = extract_braced_group(s, i) {
                    let pts = parse_coordinate_pairs(&coords_str)?;
                    plot.plots.push(AddPlotSpec {
                        style,
                        source: AddPlotSource::Coordinates(pts),
                    });
                }
            } else if i < s.len() && s[i..].starts_with('{') {
                if let Some((expr_str, _)) = extract_braced_group(s, i) {
                    plot.plots.push(AddPlotSpec {
                        style,
                        source: AddPlotSource::Expression(expr_str),
                    });
                }
            } else {
                let expr_str = s[i..].trim().to_string();
                if !expr_str.is_empty() {
                    plot.plots.push(AddPlotSpec {
                        style,
                        source: AddPlotSource::Expression(expr_str),
                    });
                }
            }
        } else if let Some(after_leg) = s.strip_prefix(r"\legend") {
            let trimmed_after = after_leg.trim_start();
            if let Some((inner, _)) = extract_braced_group(trimmed_after, 0) {
                for entry in split_top_level_commas(&inner) {
                    plot.legend_entries.push(entry);
                }
            }
        } else if let Some(after_le) = s.strip_prefix(r"\addlegendentry") {
            let trimmed_after = after_le.trim_start();
            if let Some((inner, _)) = extract_braced_group(trimmed_after, 0) {
                plot.legend_entries.push(inner.trim().to_string());
            }
        } else if (s.starts_with(r"\draw")
            || s.starts_with(r"\fill")
            || s.starts_with(r"\node")
            || s.starts_with(r"\coordinate"))
            && let Ok(mut parsed) = parse_tikz_statements(vec![s.to_string()])
        {
            plot.overlays.append(&mut parsed);
        }
    }

    if plot.plots.is_empty() && plot.overlays.is_empty() {
        return Err(MathParseError {
            message: "Blok \\begin{axis} tidak memiliki \\addplot".into(),
            position: 0,
        });
    }

    Ok(plot)
}

fn clean_inline_label(raw: &str) -> (String, bool) {
    let trimmed = raw.trim();
    if let Some(inner) = trimmed.strip_prefix('$').and_then(|s| s.strip_suffix('$')) {
        let nice = super::parse_latex_math(inner)
            .ok()
            .map(|nodes| flatten_math_text(&nodes))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| inner.to_string());
        (nice, true)
    } else {
        (trimmed.replace('$', ""), false)
    }
}

fn flatten_math_text(nodes: &[super::MathNode]) -> String {
    let mut s = String::new();
    for n in nodes {
        match n {
            super::MathNode::Text(t) => s.push_str(t.strip_prefix("__raw_text:").unwrap_or(t)),
            super::MathNode::Symbol(c) => s.push(*c),
            super::MathNode::Delimited {
                open,
                close,
                content,
            } => {
                if *open != '\0' {
                    s.push(*open);
                }
                s.push_str(&flatten_math_text(content));
                if *close != '\0' {
                    s.push(*close);
                }
            }
            super::MathNode::Frac { num, den } => {
                s.push_str(&flatten_math_text(num));
                s.push('/');
                s.push_str(&flatten_math_text(den));
            }
            super::MathNode::SubSup { base, sub, sup } => {
                if let Some(b) = base {
                    s.push_str(&flatten_math_text(std::slice::from_ref(b.as_ref())));
                }
                if let Some(sb) = sub {
                    s.push('_');
                    s.push_str(&flatten_math_text(sb));
                }
                if let Some(sp) = sup {
                    s.push('^');
                    s.push_str(&flatten_math_text(sp));
                }
            }
            _ => {}
        }
    }
    s
}

fn anchor_offset(anchor: NodeAnchor, text_w: f32, text_h: f32, pad: f32) -> (f32, f32) {
    match anchor {
        NodeAnchor::Center => (-text_w * 0.5, -text_h * 0.5),
        NodeAnchor::Above => (-text_w * 0.5, -text_h - pad),
        NodeAnchor::Below => (-text_w * 0.5, pad),
        NodeAnchor::Left => (-text_w - pad, -text_h * 0.5),
        NodeAnchor::Right => (pad, -text_h * 0.5),
        NodeAnchor::AboveLeft => (-text_w - pad, -text_h - pad),
        NodeAnchor::AboveRight => (pad, -text_h - pad),
        NodeAnchor::BelowLeft => (-text_w - pad, pad),
        NodeAnchor::BelowRight => (pad, pad),
    }
}

fn emit_label(
    items: &mut Vec<MathLayoutItem>,
    center: Pos2,
    raw_label: &str,
    font_size: f32,
    anchor: NodeAnchor,
    color: Option<Color32>,
    pad: f32,
) {
    let (clean, is_math) = clean_inline_label(raw_label);
    if clean.is_empty() {
        return;
    }
    let w = clean.chars().count() as f32 * font_size * 0.55;
    let h = font_size * 1.15;
    let (dx, dy) = anchor_offset(anchor, w, h, pad);
    items.push(MathLayoutItem::ColoredGlyph {
        text: clean,
        font_size,
        is_italic: is_math && anchor != NodeAnchor::Center,
        is_bold: false,
        pos: Pos2::new(center.x + dx, center.y + dy),
        color,
    });
}

fn apply_fill_opacity(col: Option<Color32>, opacity: f32) -> Option<Color32> {
    col.map(|c| {
        let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
    })
}

fn compute_nice_ticks(min_v: f64, max_v: f64, target_count: usize) -> Vec<f64> {
    let span = (max_v - min_v).abs();
    if span <= 1e-12 || !span.is_finite() {
        return vec![min_v];
    }
    let raw_step = span / (target_count.max(2) as f64);
    let mag = 10_f64.powf(raw_step.log10().floor());
    let norm = raw_step / mag;
    let nice_mult = if norm <= 1.2 {
        1.0
    } else if norm <= 2.5 {
        2.0
    } else if norm <= 5.5 {
        5.0
    } else {
        10.0
    };
    let step = nice_mult * mag;
    let start = (min_v / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut v = start;
    let mut guard = 0;
    while v <= max_v + step * 1e-6 && guard < 24 {
        let snapped = if v.abs() < step * 1e-6 { 0.0 } else { v };
        ticks.push(snapped);
        v += step;
        guard += 1;
    }
    ticks
}

fn format_tick(v: f64) -> String {
    if v.abs() < 1e-9 {
        "0".into()
    } else if (v - v.round()).abs() < 1e-6 {
        format!("{:.0}", v.round())
    } else if (v * 10.0 - (v * 10.0).round()).abs() < 1e-5 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    }
}

fn layout_tikzpicture(pic: &TikzPicture, font_size: f32) -> Vec<MathLayoutItem> {
    let unit_px = 42.0 * pic.scale * (font_size / 20.0).clamp(0.2, 6.0);
    let label_fs = (font_size * 0.78).clamp(6.0, 72.0);
    let mut named_pts: HashMap<String, Pos2> = HashMap::new();
    let mut node_radii: HashMap<String, f32> = HashMap::new();

    let resolve_raw_pt = |c: &TikzCoord, map: &HashMap<String, Pos2>| -> Pos2 {
        match c {
            TikzCoord::Point(x, y) => Pos2::new((*x as f32) * unit_px, -(*y as f32) * unit_px),
            TikzCoord::Polar(deg, r) => {
                let rad = deg.to_radians();
                let x = r * rad.cos();
                let y = r * rad.sin();
                Pos2::new((x as f32) * unit_px, -(y as f32) * unit_px)
            }
            TikzCoord::Named(n) => map.get(n).copied().unwrap_or(Pos2::ZERO),
        }
    };

    // Pass 1: Daftarkan seluruh named coordinates & nodes
    for st in &pic.stmts {
        match st {
            TikzStmt::Coordinate { name, coord } => {
                let pt = resolve_raw_pt(coord, &named_pts);
                named_pts.insert(name.clone(), pt);
            }
            TikzStmt::Node {
                name: Some(name),
                coord,
                style,
                label,
            } => {
                let pt = resolve_raw_pt(coord, &named_pts);
                named_pts.insert(name.clone(), pt);
                if style.node_shape != NodeShape::None {
                    let r = (label.chars().count().max(1) as f32 * label_fs * 0.35
                        + label_fs * 0.65)
                        .clamp(12.0, 40.0);
                    node_radii.insert(name.clone(), r);
                }
            }
            _ => {}
        }
    }

    let mut items = Vec::new();
    for st in &pic.stmts {
        match st {
            TikzStmt::Coordinate { .. } => {}
            TikzStmt::Node {
                name,
                coord,
                style,
                label,
            } => {
                let pt = resolve_raw_pt(coord, &named_pts);
                match style.node_shape {
                    NodeShape::Circle => {
                        let r = name
                            .as_ref()
                            .and_then(|n| node_radii.get(n).copied())
                            .unwrap_or(14.0);
                        items.push(MathLayoutItem::Ellipse {
                            center: pt,
                            rx: r,
                            ry: r,
                            fill: apply_fill_opacity(
                                style.fill_color,
                                style.fill_opacity.max(0.15),
                            ),
                            stroke_color: if style.draw_disabled {
                                None
                            } else {
                                Some(style.draw_color.unwrap_or(Color32::from_rgb(35, 38, 46)))
                            },
                            stroke_width: style.stroke_width,
                        });
                    }
                    NodeShape::Rectangle => {
                        let hw = (label.chars().count().max(1) as f32 * label_fs * 0.34 + 10.0)
                            .max(18.0);
                        let hh = (label_fs * 0.85).max(13.0);
                        items.push(MathLayoutItem::Polygon {
                            points: vec![
                                Pos2::new(pt.x - hw, pt.y - hh),
                                Pos2::new(pt.x + hw, pt.y - hh),
                                Pos2::new(pt.x + hw, pt.y + hh),
                                Pos2::new(pt.x - hw, pt.y + hh),
                            ],
                            fill: apply_fill_opacity(
                                style.fill_color,
                                style.fill_opacity.max(0.15),
                            ),
                            stroke_color: if style.draw_disabled {
                                None
                            } else {
                                Some(style.draw_color.unwrap_or(Color32::from_rgb(35, 38, 46)))
                            },
                            stroke_width: style.stroke_width,
                        });
                    }
                    NodeShape::None => {}
                }
                emit_label(
                    &mut items,
                    pt,
                    label,
                    label_fs,
                    style.node_anchor,
                    style.text_color,
                    6.0,
                );
            }
            TikzStmt::DrawPath { style, segments } => {
                let mut cur_pt = Pos2::ZERO;
                let mut cur_name: Option<String> = None;
                let mut poly_pts: Vec<Pos2> = Vec::new();
                let mut poly_closed = false;
                let mut explicit_segments: Vec<(Pos2, Pos2, Option<String>, Option<String>)> =
                    Vec::new();
                let mut inline_nodes: Vec<(Pos2, Pos2, InlinePathNode)> = Vec::new();

                for seg in segments {
                    match seg {
                        PathSegment::MoveTo(c) => {
                            if poly_pts.len() >= 2 && (style.fill_color.is_some() || poly_closed) {
                                items.push(MathLayoutItem::Polygon {
                                    points: std::mem::take(&mut poly_pts),
                                    fill: apply_fill_opacity(style.fill_color, style.fill_opacity),
                                    stroke_color: if style.draw_disabled {
                                        None
                                    } else {
                                        style.draw_color
                                    },
                                    stroke_width: style.stroke_width,
                                });
                            }
                            poly_pts.clear();
                            poly_closed = false;
                            cur_pt = resolve_raw_pt(c, &named_pts);
                            cur_name = match c {
                                TikzCoord::Named(n) => Some(n.clone()),
                                _ => None,
                            };
                            poly_pts.push(cur_pt);
                        }
                        PathSegment::LineTo(c, opt_node) => {
                            let next_pt = resolve_raw_pt(c, &named_pts);
                            let next_name = match c {
                                TikzCoord::Named(n) => Some(n.clone()),
                                _ => None,
                            };
                            if let Some(inode) = opt_node {
                                inline_nodes.push((cur_pt, next_pt, inode.clone()));
                            }
                            explicit_segments.push((
                                cur_pt,
                                next_pt,
                                cur_name.clone(),
                                next_name.clone(),
                            ));
                            poly_pts.push(next_pt);
                            cur_pt = next_pt;
                            cur_name = next_name;
                        }
                        PathSegment::Cycle => {
                            if poly_pts.len() >= 2 {
                                let first = poly_pts[0];
                                explicit_segments.push((cur_pt, first, cur_name.clone(), None));
                                poly_closed = true;
                            }
                        }
                        PathSegment::Circle(r) => {
                            let r_px = (*r as f32) * unit_px;
                            items.push(MathLayoutItem::Ellipse {
                                center: cur_pt,
                                rx: r_px,
                                ry: r_px,
                                fill: apply_fill_opacity(style.fill_color, style.fill_opacity),
                                stroke_color: if style.draw_disabled {
                                    None
                                } else {
                                    Some(style.draw_color.unwrap_or(Color32::from_rgb(30, 34, 42)))
                                },
                                stroke_width: style.stroke_width,
                            });
                        }
                        PathSegment::Ellipse(rx, ry) => {
                            items.push(MathLayoutItem::Ellipse {
                                center: cur_pt,
                                rx: (*rx as f32) * unit_px,
                                ry: (*ry as f32) * unit_px,
                                fill: apply_fill_opacity(style.fill_color, style.fill_opacity),
                                stroke_color: if style.draw_disabled {
                                    None
                                } else {
                                    Some(style.draw_color.unwrap_or(Color32::from_rgb(30, 34, 42)))
                                },
                                stroke_width: style.stroke_width,
                            });
                        }
                        PathSegment::Rectangle(c2) => {
                            let p2 = resolve_raw_pt(c2, &named_pts);
                            items.push(MathLayoutItem::Polygon {
                                points: vec![
                                    Pos2::new(cur_pt.x, cur_pt.y),
                                    Pos2::new(p2.x, cur_pt.y),
                                    Pos2::new(p2.x, p2.y),
                                    Pos2::new(cur_pt.x, p2.y),
                                ],
                                fill: apply_fill_opacity(style.fill_color, style.fill_opacity),
                                stroke_color: if style.draw_disabled {
                                    None
                                } else {
                                    Some(style.draw_color.unwrap_or(Color32::from_rgb(30, 34, 42)))
                                },
                                stroke_width: style.stroke_width,
                            });
                        }
                        PathSegment::Arc {
                            start_deg,
                            end_deg,
                            rx,
                            ry,
                        } => {
                            let rx_px = (*rx as f32) * unit_px;
                            let ry_px = (*ry as f32) * unit_px;
                            let a0 = start_deg.to_radians() as f32;
                            let a1 = end_deg.to_radians() as f32;
                            let cx = cur_pt.x - rx_px * a0.cos();
                            let cy = cur_pt.y + ry_px * a0.sin();
                            let steps = 28;
                            let mut arc_pts = Vec::with_capacity(steps + 1);
                            for k in 0..=steps {
                                let t = k as f32 / (steps as f32);
                                let a = a0 + (a1 - a0) * t;
                                arc_pts.push(Pos2::new(cx + rx_px * a.cos(), cy - ry_px * a.sin()));
                            }
                            if let Some(last) = arc_pts.last().copied() {
                                cur_pt = last;
                            }
                            items.push(MathLayoutItem::Polyline {
                                points: arc_pts,
                                stroke_width: style.stroke_width,
                                color: style.draw_color,
                                dash: style.dash,
                                arrow_start: style.arrow_start,
                                arrow_end: style.arrow_end,
                            });
                        }
                        PathSegment::Grid(c2) => {
                            let p2 = resolve_raw_pt(c2, &named_pts);
                            let step_px = (style.step.unwrap_or(1.0) as f32) * unit_px;
                            let min_x = cur_pt.x.min(p2.x);
                            let max_x = cur_pt.x.max(p2.x);
                            let min_y = cur_pt.y.min(p2.y);
                            let max_y = cur_pt.y.max(p2.y);
                            let grid_col = style
                                .draw_color
                                .unwrap_or(Color32::from_rgba_unmultiplied(160, 168, 180, 160));
                            let mut gx = min_x;
                            let mut g_cnt = 0;
                            while gx <= max_x + 0.5 && g_cnt < 100 {
                                items.push(MathLayoutItem::StyledLine {
                                    start: Pos2::new(gx, min_y),
                                    end: Pos2::new(gx, max_y),
                                    stroke_width: 0.7,
                                    color: Some(grid_col),
                                    dash: MathLineDash::Solid,
                                    arrow_start: false,
                                    arrow_end: false,
                                });
                                gx += step_px.max(6.0);
                                g_cnt += 1;
                            }
                            let mut gy = min_y;
                            g_cnt = 0;
                            while gy <= max_y + 0.5 && g_cnt < 100 {
                                items.push(MathLayoutItem::StyledLine {
                                    start: Pos2::new(min_x, gy),
                                    end: Pos2::new(max_x, gy),
                                    stroke_width: 0.7,
                                    color: Some(grid_col),
                                    dash: MathLineDash::Solid,
                                    arrow_start: false,
                                    arrow_end: false,
                                });
                                gy += step_px.max(6.0);
                                g_cnt += 1;
                            }
                        }
                        PathSegment::PlotExpr {
                            expr,
                            domain,
                            samples,
                        } => {
                            let n = (*samples).clamp(8, 300);
                            let (a, b) = *domain;
                            let mut run = Vec::new();
                            for k in 0..n {
                                let t = k as f64 / ((n - 1) as f64);
                                let xv = a + (b - a) * t;
                                if let Some(yv) = eval_math_expr(expr, xv) {
                                    run.push(Pos2::new(
                                        (xv as f32) * unit_px,
                                        -(yv as f32) * unit_px,
                                    ));
                                } else if run.len() >= 2 {
                                    items.push(MathLayoutItem::Polyline {
                                        points: std::mem::take(&mut run),
                                        stroke_width: style.stroke_width,
                                        color: style.draw_color,
                                        dash: style.dash,
                                        arrow_start: false,
                                        arrow_end: false,
                                    });
                                } else {
                                    run.clear();
                                }
                            }
                            if run.len() >= 2 {
                                items.push(MathLayoutItem::Polyline {
                                    points: run,
                                    stroke_width: style.stroke_width,
                                    color: style.draw_color,
                                    dash: style.dash,
                                    arrow_start: style.arrow_start,
                                    arrow_end: style.arrow_end,
                                });
                            }
                        }
                        PathSegment::PlotCoords(pts) => {
                            let mapped: Vec<Pos2> = pts
                                .iter()
                                .map(|&(x, y)| {
                                    Pos2::new((x as f32) * unit_px, -(y as f32) * unit_px)
                                })
                                .collect();
                            if mapped.len() >= 2 {
                                items.push(MathLayoutItem::Polyline {
                                    points: mapped,
                                    stroke_width: style.stroke_width,
                                    color: style.draw_color,
                                    dash: style.dash,
                                    arrow_start: style.arrow_start,
                                    arrow_end: style.arrow_end,
                                });
                            }
                        }
                    }
                }

                if style.fill_color.is_some() && poly_pts.len() >= 3 {
                    items.push(MathLayoutItem::Polygon {
                        points: poly_pts,
                        fill: apply_fill_opacity(style.fill_color, style.fill_opacity),
                        stroke_color: None,
                        stroke_width: style.stroke_width,
                    });
                }

                if !style.draw_disabled {
                    for (idx, (mut p1, mut p2, n1, n2)) in
                        explicit_segments.iter().cloned().enumerate()
                    {
                        // Potong ujung garis jika menghubungkan node berbentuk lingkaran agar panah menyentuh tepi
                        let dir = p2 - p1;
                        let len = dir.length();
                        if len > 1e-3 {
                            let u = dir / len;
                            if let Some(ref name1) = n1
                                && let Some(&r1) = node_radii.get(name1)
                                && len > r1 * 1.4
                            {
                                p1 += u * r1;
                            }
                            if let Some(ref name2) = n2
                                && let Some(&r2) = node_radii.get(name2)
                                && len > r2 * 1.4
                            {
                                p2 -= u * r2;
                            }
                        }
                        let is_first = idx == 0;
                        let is_last = idx + 1 == explicit_segments.len();
                        items.push(MathLayoutItem::StyledLine {
                            start: p1,
                            end: p2,
                            stroke_width: style.stroke_width,
                            color: style.draw_color,
                            dash: style.dash,
                            arrow_start: style.arrow_start && is_first,
                            arrow_end: style.arrow_end && is_last,
                        });
                    }
                }

                for (p1, p2, inode) in inline_nodes {
                    let t = inode.style.node_pos.unwrap_or(0.5);
                    let pos = p1.lerp(p2, t);
                    let mut anchor = inode.style.node_anchor;
                    if anchor == NodeAnchor::Center {
                        anchor = NodeAnchor::Above;
                    }
                    emit_label(
                        &mut items,
                        pos,
                        &inode.label,
                        label_fs,
                        anchor,
                        inode.style.text_color,
                        5.0,
                    );
                }
            }
            TikzStmt::EmbeddedAxis(ax) => {
                items.extend(layout_axis_plot(ax, font_size));
            }
        }
    }
    items
}

fn layout_axis_plot_3d(ax: &AxisPlot, font_size: f32) -> Vec<MathLayoutItem> {
    let box_scale = (font_size * 11.2).clamp(90.0, 900.0);
    let label_fs = (font_size * 0.72).clamp(6.0, 64.0);
    let tick_fs = (font_size * 0.56).clamp(5.0, 48.0);

    let (az_deg, el_deg) = ax.view.unwrap_or((-37.5, 30.0));
    let az = (az_deg as f32).to_radians();
    let el = (el_deg as f32).to_radians();

    // Basis ortonormal kamera 3D tangan-kanan (vektor Z mengarah ke atas)
    let eye = (az.sin() * el.cos(), -az.cos() * el.cos(), el.sin());
    let right = (az.cos(), az.sin(), 0.0_f32);
    let up = (-az.sin() * el.sin(), az.cos() * el.sin(), el.cos());

    struct SurfaceGrid<'a> {
        spec: &'a AddPlotSpec,
        nx: usize,
        ny: usize,
        pts: Vec<Option<(f64, f64, f64)>>,
    }

    type Series3D<'a> = (&'a AddPlotSpec, Vec<(f64, f64, f64)>);
    let mut grids: Vec<SurfaceGrid<'_>> = Vec::new();
    let mut param_curves: Vec<Series3D<'_>> = Vec::new();
    let mut scatter_pts: Vec<Series3D<'_>> = Vec::new();

    let mut data_xmin = f64::INFINITY;
    let mut data_xmax = f64::NEG_INFINITY;
    let mut data_ymin = f64::INFINITY;
    let mut data_ymax = f64::NEG_INFINITY;
    let mut data_zmin = f64::INFINITY;
    let mut data_zmax = f64::NEG_INFINITY;

    let mut include_pt = |x: f64, y: f64, z: f64| {
        data_xmin = data_xmin.min(x);
        data_xmax = data_xmax.max(x);
        data_ymin = data_ymin.min(y);
        data_ymax = data_ymax.max(y);
        data_zmin = data_zmin.min(z);
        data_zmax = data_zmax.max(z);
    };

    for pspec in &ax.plots {
        match &pspec.source {
            AddPlotSource::Surface3D(expr) => {
                let (dx0, dx1) = pspec.style.domain.unwrap_or(ax.domain);
                let dx0 = ax.xmin.unwrap_or(dx0);
                let dx1 = ax.xmax.unwrap_or(dx1);
                let (dy0, dy1) = pspec.style.y_domain.or(ax.y_domain).unwrap_or((dx0, dx1));
                let dy0 = ax.ymin.unwrap_or(dy0);
                let dy1 = ax.ymax.unwrap_or(dy1);

                let default_samples = if ax.samples == 80 && pspec.style.samples.is_none() {
                    24
                } else {
                    pspec.style.samples.unwrap_or(ax.samples)
                };
                let nx = default_samples.clamp(6, 32);
                let ny = pspec
                    .style
                    .samples_y
                    .or(ax.samples_y)
                    .unwrap_or(nx)
                    .clamp(6, 32);

                let mut pts = Vec::with_capacity(nx * ny);
                for iy in 0..ny {
                    let ty = iy as f64 / ((ny - 1) as f64);
                    let yv = dy0 + (dy1 - dy0) * ty;
                    for ix in 0..nx {
                        let tx = ix as f64 / ((nx - 1) as f64);
                        let xv = dx0 + (dx1 - dx0) * tx;
                        if let Some(zv) = eval_math_expr_2d(expr, xv, yv)
                            && zv.abs() < 1e6
                        {
                            include_pt(xv, yv, zv);
                            pts.push(Some((xv, yv, zv)));
                        } else {
                            pts.push(None);
                        }
                    }
                }
                grids.push(SurfaceGrid {
                    spec: pspec,
                    nx,
                    ny,
                    pts,
                });
            }
            AddPlotSource::Parametric3D(fx, fy, fz) => {
                let (d0, d1) = pspec.style.domain.unwrap_or(ax.domain);
                let n = pspec.style.samples.unwrap_or(80).clamp(12, 240);
                let mut pts = Vec::with_capacity(n);
                for k in 0..n {
                    let tv = d0 + (d1 - d0) * (k as f64 / ((n - 1) as f64));
                    if let (Some(xv), Some(yv), Some(zv)) = (
                        eval_math_expr(fx, tv),
                        eval_math_expr(fy, tv),
                        eval_math_expr(fz, tv),
                    ) {
                        include_pt(xv, yv, zv);
                        pts.push((xv, yv, zv));
                    }
                }
                param_curves.push((pspec, pts));
            }
            AddPlotSource::Coordinates3D(pts) => {
                let mut valid = Vec::with_capacity(pts.len());
                for &(xv, yv, zv) in pts {
                    if xv.is_finite() && yv.is_finite() && zv.is_finite() {
                        include_pt(xv, yv, zv);
                        valid.push((xv, yv, zv));
                    }
                }
                scatter_pts.push((pspec, valid));
            }
            _ => {}
        }
    }

    let mut xmin = ax.xmin.unwrap_or_else(|| {
        if data_xmin.is_finite() {
            data_xmin
        } else {
            ax.domain.0
        }
    });
    let mut xmax = ax.xmax.unwrap_or_else(|| {
        if data_xmax.is_finite() {
            data_xmax
        } else {
            ax.domain.1
        }
    });
    if (xmax - xmin).abs() < 1e-6 {
        xmin -= 1.0;
        xmax += 1.0;
    }

    let (yd0, yd1) = ax.y_domain.unwrap_or(ax.domain);
    let mut ymin = ax.ymin.unwrap_or_else(|| {
        if data_ymin.is_finite() {
            data_ymin
        } else {
            yd0
        }
    });
    let mut ymax = ax.ymax.unwrap_or_else(|| {
        if data_ymax.is_finite() {
            data_ymax
        } else {
            yd1
        }
    });
    if (ymax - ymin).abs() < 1e-6 {
        ymin -= 1.0;
        ymax += 1.0;
    }

    let mut zmin = ax.zmin.unwrap_or_else(|| {
        if data_zmin.is_finite() {
            data_zmin
        } else {
            -1.0
        }
    });
    let mut zmax = ax.zmax.unwrap_or_else(|| {
        if data_zmax.is_finite() {
            data_zmax
        } else {
            1.0
        }
    });
    if (zmax - zmin).abs() < 1e-9 {
        zmin -= 0.5;
        zmax += 0.5;
    } else if ax.zmin.is_none() || ax.zmax.is_none() {
        let zpad = (zmax - zmin) * 0.06;
        if ax.zmin.is_none() {
            zmin -= zpad;
        }
        if ax.zmax.is_none() {
            zmax += zpad;
        }
    }

    let to_norm = |x: f64, y: f64, z: f64| -> (f32, f32, f32) {
        let nx = (((x - xmin) / (xmax - xmin).max(1e-9)) - 0.5) as f32;
        let ny = (((y - ymin) / (ymax - ymin).max(1e-9)) - 0.5) as f32;
        let nz = (((z - zmin) / (zmax - zmin).max(1e-9)) - 0.5) as f32;
        (nx, ny, nz)
    };
    let proj_norm = |nx: f32, ny: f32, nz: f32| -> (Pos2, f32) {
        let sx = (nx * right.0 + ny * right.1) * box_scale;
        let sy = -(nx * up.0 + ny * up.1 + nz * up.2) * box_scale;
        let depth = nx * eye.0 + ny * eye.1 + nz * eye.2;
        (Pos2::new(sx, sy), depth)
    };
    let proj_data = |x: f64, y: f64, z: f64| -> (Pos2, f32, (f32, f32, f32)) {
        let n = to_norm(x, y, z);
        let (p, d) = proj_norm(n.0, n.1, n.2);
        (p, d, n)
    };

    let mut items = Vec::new();

    // Tentukan sudut terjauh dari kamera sebagai pertemuan 3 bidang belakang kotak 3D
    let bx = if eye.0 >= 0.0 { -0.5_f32 } else { 0.5_f32 };
    let by = if eye.1 >= 0.0 { -0.5_f32 } else { 0.5_f32 };
    let bz = if eye.2 >= 0.0 { -0.5_f32 } else { 0.5_f32 };
    let fx = -bx;
    let fy = -by;
    let fz = -bz;

    let wall_fill = Some(Color32::from_rgba_unmultiplied(244, 247, 252, 170));
    let wall_stroke = Some(Color32::from_rgb(175, 182, 194));
    let wall_faces = [
        [(bx, by, bz), (fx, by, bz), (fx, fy, bz), (bx, fy, bz)],
        [(bx, by, bz), (fx, by, bz), (fx, by, fz), (bx, by, fz)],
        [(bx, by, bz), (bx, fy, bz), (bx, fy, fz), (bx, by, fz)],
    ];
    for face in wall_faces {
        items.push(MathLayoutItem::Polygon {
            points: face
                .into_iter()
                .map(|(x, y, z)| proj_norm(x, y, z).0)
                .collect(),
            fill: wall_fill,
            stroke_color: wall_stroke,
            stroke_width: 0.9,
        });
    }

    // Garis kisi tipis pada lantai & dinding belakang kotak 3D
    let x_ticks = compute_nice_ticks(xmin, xmax, 4);
    let y_ticks = compute_nice_ticks(ymin, ymax, 4);
    let z_ticks = compute_nice_ticks(zmin, zmax, 4);
    let grid_col = Some(Color32::from_rgba_unmultiplied(150, 158, 172, 130));
    let emit_grid_seg =
        |items: &mut Vec<MathLayoutItem>, p0: (f32, f32, f32), p1: (f32, f32, f32)| {
            items.push(MathLayoutItem::StyledLine {
                start: proj_norm(p0.0, p0.1, p0.2).0,
                end: proj_norm(p1.0, p1.1, p1.2).0,
                stroke_width: 0.65,
                color: grid_col,
                dash: MathLineDash::Dotted,
                arrow_start: false,
                arrow_end: false,
            });
        };
    for &xt in &x_ticks {
        let nx = (((xt - xmin) / (xmax - xmin).max(1e-9)) - 0.5) as f32;
        if (-0.501..=0.501).contains(&nx) {
            emit_grid_seg(&mut items, (nx, by, bz), (nx, fy, bz));
            emit_grid_seg(&mut items, (nx, by, bz), (nx, by, fz));
        }
    }
    for &yt in &y_ticks {
        let ny = (((yt - ymin) / (ymax - ymin).max(1e-9)) - 0.5) as f32;
        if (-0.501..=0.501).contains(&ny) {
            emit_grid_seg(&mut items, (bx, ny, bz), (fx, ny, bz));
            emit_grid_seg(&mut items, (bx, ny, bz), (bx, ny, fz));
        }
    }
    for &zt in &z_ticks {
        let nz = (((zt - zmin) / (zmax - zmin).max(1e-9)) - 0.5) as f32;
        if (-0.501..=0.501).contains(&nz) {
            emit_grid_seg(&mut items, (bx, by, nz), (fx, by, nz));
            emit_grid_seg(&mut items, (bx, by, nz), (bx, fy, nz));
        }
    }

    // Render permukaan 3D memakai Painter's Algorithm (urut belakang -> depan) + pencahayaan lembut
    struct Quad3D {
        depth: f32,
        poly: [Pos2; 4],
        fill: Option<Color32>,
        stroke_color: Option<Color32>,
        stroke_width: f32,
    }
    let mut quads: Vec<Quad3D> = Vec::new();
    let light_dir = (0.35_f32, -0.55_f32, 0.76_f32);

    for g in &grids {
        let cm = g.spec.style.colormap.unwrap_or(ax.colormap);
        let z_span = (zmax - zmin).max(1e-9);
        let wireframe_only = g.spec.style.mesh && !g.spec.style.surf;
        let edge_sw = (font_size * 0.032).clamp(0.4, 1.4);

        for iy in 0..g.ny.saturating_sub(1) {
            for ix in 0..g.nx.saturating_sub(1) {
                let Some(p00) = g.pts[iy * g.nx + ix] else {
                    continue;
                };
                let Some(p10) = g.pts[iy * g.nx + ix + 1] else {
                    continue;
                };
                let Some(p11) = g.pts[(iy + 1) * g.nx + ix + 1] else {
                    continue;
                };
                let Some(p01) = g.pts[(iy + 1) * g.nx + ix] else {
                    continue;
                };

                let (s00, d00, n00) = proj_data(p00.0, p00.1, p00.2);
                let (s10, d10, n10) = proj_data(p10.0, p10.1, p10.2);
                let (s11, d11, _n11) = proj_data(p11.0, p11.1, p11.2);
                let (s01, d01, n01) = proj_data(p01.0, p01.1, p01.2);

                let avg_depth = (d00 + d10 + d11 + d01) * 0.25;
                let avg_z = (p00.2 + p10.2 + p11.2 + p01.2) * 0.25;
                let t_z = ((avg_z - zmin) / z_span) as f32;
                let base_col = sample_colormap(cm, t_z);

                if wireframe_only {
                    quads.push(Quad3D {
                        depth: avg_depth,
                        poly: [s00, s10, s11, s01],
                        fill: Some(Color32::from_rgba_unmultiplied(255, 255, 255, 215)),
                        stroke_color: Some(base_col),
                        stroke_width: (edge_sw * 1.35).max(0.6),
                    });
                } else {
                    let e1 = (n10.0 - n00.0, n10.1 - n00.1, n10.2 - n00.2);
                    let e2 = (n01.0 - n00.0, n01.1 - n00.1, n01.2 - n00.2);
                    let cx = e1.1 * e2.2 - e1.2 * e2.1;
                    let cy = e1.2 * e2.0 - e1.0 * e2.2;
                    let cz = e1.0 * e2.1 - e1.1 * e2.0;
                    let clen = (cx * cx + cy * cy + cz * cz).sqrt().max(1e-5);
                    let mut nrm = (cx / clen, cy / clen, cz / clen);
                    if nrm.0 * eye.0 + nrm.1 * eye.1 + nrm.2 * eye.2 < 0.0 {
                        nrm = (-nrm.0, -nrm.1, -nrm.2);
                    }
                    let shade = (0.82
                        + 0.24
                            * (nrm.0 * light_dir.0 + nrm.1 * light_dir.1 + nrm.2 * light_dir.2)
                                .max(0.0))
                    .clamp(0.75, 1.06);
                    let fill_col = Color32::from_rgb(
                        ((base_col.r() as f32) * shade).clamp(0.0, 255.0) as u8,
                        ((base_col.g() as f32) * shade).clamp(0.0, 255.0) as u8,
                        ((base_col.b() as f32) * shade).clamp(0.0, 255.0) as u8,
                    );
                    let border_col = Color32::from_rgb(
                        ((fill_col.r() as f32) * 0.72) as u8,
                        ((fill_col.g() as f32) * 0.72) as u8,
                        ((fill_col.b() as f32) * 0.72) as u8,
                    );
                    quads.push(Quad3D {
                        depth: avg_depth,
                        poly: [s00, s10, s11, s01],
                        fill: Some(fill_col),
                        stroke_color: Some(border_col),
                        stroke_width: edge_sw,
                    });
                }
            }
        }
    }

    quads.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    for q in quads {
        items.push(MathLayoutItem::Polygon {
            points: q.poly.to_vec(),
            fill: q.fill,
            stroke_color: q.stroke_color,
            stroke_width: q.stroke_width,
        });
    }

    for (pspec, pts) in param_curves {
        if pts.len() >= 2 {
            let col = pspec
                .style
                .draw_color
                .unwrap_or(Color32::from_rgb(24, 110, 214));
            items.push(MathLayoutItem::Polyline {
                points: pts
                    .into_iter()
                    .map(|(x, y, z)| proj_data(x, y, z).0)
                    .collect(),
                stroke_width: pspec.style.stroke_width,
                color: Some(col),
                dash: pspec.style.dash,
                arrow_start: false,
                arrow_end: false,
            });
        }
    }

    for (pspec, pts) in scatter_pts {
        if pts.is_empty() {
            continue;
        }
        let col = pspec
            .style
            .draw_color
            .unwrap_or(Color32::from_rgb(24, 110, 214));
        let projected: Vec<Pos2> = pts.iter().map(|&(x, y, z)| proj_data(x, y, z).0).collect();
        if !pspec.style.only_marks && projected.len() >= 2 {
            items.push(MathLayoutItem::Polyline {
                points: projected.clone(),
                stroke_width: pspec.style.stroke_width,
                color: Some(col),
                dash: pspec.style.dash,
                arrow_start: false,
                arrow_end: false,
            });
        }
        if pspec.style.mark != PlotMark::None || pspec.style.only_marks {
            let r = (font_size * 0.16).clamp(2.2, 7.0);
            for pt in projected {
                items.push(MathLayoutItem::Ellipse {
                    center: pt,
                    rx: r,
                    ry: r,
                    fill: Some(col),
                    stroke_color: Some(Color32::WHITE),
                    stroke_width: 0.9,
                });
            }
        }
    }

    // Garis tepi depan sumbu X, Y, Z + tanda tick & label numerik
    let axis_col = Some(Color32::from_rgb(40, 44, 52));
    let axis_sw = (font_size * 0.065).clamp(0.9, 2.2);
    let draw_axis_line =
        |items: &mut Vec<MathLayoutItem>, a: (f32, f32, f32), b: (f32, f32, f32)| {
            items.push(MathLayoutItem::StyledLine {
                start: proj_norm(a.0, a.1, a.2).0,
                end: proj_norm(b.0, b.1, b.2).0,
                stroke_width: axis_sw,
                color: axis_col,
                dash: MathLineDash::Solid,
                arrow_start: false,
                arrow_end: false,
            });
        };

    // Tentukan tepi sumbu Z paling kiri di layar agar label Z tidak menutupi grafik
    let z_left_candidates = [(bx, fy), (fx, by)];
    let (zx_edge, zy_edge) = if proj_norm(z_left_candidates[0].0, z_left_candidates[0].1, 0.0)
        .0
        .x
        <= proj_norm(z_left_candidates[1].0, z_left_candidates[1].1, 0.0)
            .0
            .x
    {
        z_left_candidates[0]
    } else {
        z_left_candidates[1]
    };

    // Tepi depan bawah untuk sumbu X dan Y
    draw_axis_line(&mut items, (-0.5, fy, bz), (0.5, fy, bz));
    draw_axis_line(&mut items, (fx, -0.5, bz), (fx, 0.5, bz));
    draw_axis_line(
        &mut items,
        (zx_edge, zy_edge, -0.5),
        (zx_edge, zy_edge, 0.5),
    );

    let emit_3d_tick_label =
        |items: &mut Vec<MathLayoutItem>, pos: Pos2, text: String, dx: f32, dy: f32| {
            items.push(MathLayoutItem::ColoredGlyph {
                text,
                font_size: tick_fs,
                is_italic: false,
                is_bold: false,
                pos: Pos2::new(pos.x + dx, pos.y + dy),
                color: axis_col,
            });
        };

    for &xt in &x_ticks {
        let nx = (((xt - xmin) / (xmax - xmin).max(1e-9)) - 0.5) as f32;
        if (-0.501..=0.501).contains(&nx) {
            let p = proj_norm(nx, fy, bz).0;
            let lbl = format_tick(xt);
            let tw = lbl.chars().count() as f32 * tick_fs * 0.52;
            emit_3d_tick_label(&mut items, p, lbl, -tw * 0.5, 4.0);
        }
    }
    for &yt in &y_ticks {
        let ny = (((yt - ymin) / (ymax - ymin).max(1e-9)) - 0.5) as f32;
        if (-0.501..=0.501).contains(&ny) {
            let p = proj_norm(fx, ny, bz).0;
            emit_3d_tick_label(&mut items, p, format_tick(yt), 5.0, -tick_fs * 0.35);
        }
    }
    for &zt in &z_ticks {
        let nz = (((zt - zmin) / (zmax - zmin).max(1e-9)) - 0.5) as f32;
        if (-0.501..=0.501).contains(&nz) {
            let p = proj_norm(zx_edge, zy_edge, nz).0;
            let lbl = format_tick(zt);
            let tw = lbl.chars().count() as f32 * tick_fs * 0.54;
            emit_3d_tick_label(&mut items, p, lbl, -tw - 6.0, -tick_fs * 0.5);
        }
    }

    // Label sumbu X, Y, Z & Judul
    let xl_text = ax
        .xlabel
        .as_deref()
        .map(|s| clean_inline_label(s).0)
        .unwrap_or_else(|| "x".into());
    let x_mid = proj_norm(0.0, fy, bz).0;
    items.push(MathLayoutItem::ColoredGlyph {
        text: xl_text,
        font_size: label_fs,
        is_italic: true,
        is_bold: false,
        pos: Pos2::new(x_mid.x, x_mid.y + tick_fs + 8.0),
        color: axis_col,
    });

    let yl_text = ax
        .ylabel
        .as_deref()
        .map(|s| clean_inline_label(s).0)
        .unwrap_or_else(|| "y".into());
    let y_mid = proj_norm(fx, 0.0, bz).0;
    items.push(MathLayoutItem::ColoredGlyph {
        text: yl_text,
        font_size: label_fs,
        is_italic: true,
        is_bold: false,
        pos: Pos2::new(y_mid.x + tick_fs * 2.2, y_mid.y + tick_fs * 0.4),
        color: axis_col,
    });

    let zl_text = ax
        .zlabel
        .as_deref()
        .map(|s| clean_inline_label(s).0)
        .unwrap_or_else(|| "z".into());
    let z_mid = proj_norm(zx_edge, zy_edge, 0.0).0;
    items.push(MathLayoutItem::ColoredGlyph {
        text: zl_text,
        font_size: label_fs,
        is_italic: true,
        is_bold: false,
        pos: Pos2::new(z_mid.x - label_fs * 3.0, z_mid.y - label_fs * 0.5),
        color: axis_col,
    });

    if let Some(title) = &ax.title {
        let (clean_t, _) = clean_inline_label(title);
        let top_p = proj_norm(0.0, 0.0, 0.5).0;
        let tw = clean_t.chars().count() as f32 * font_size * 0.48;
        items.push(MathLayoutItem::ColoredGlyph {
            text: clean_t,
            font_size: (font_size * 0.88).clamp(8.0, 48.0),
            is_italic: false,
            is_bold: true,
            pos: Pos2::new(top_p.x - tw * 0.5, top_p.y - label_fs * 2.1),
            color: axis_col,
        });
    }

    items
}

fn layout_axis_plot(ax: &AxisPlot, font_size: f32) -> Vec<MathLayoutItem> {
    if ax.plots.iter().any(AddPlotSpec::is_3d) {
        return layout_axis_plot_3d(ax, font_size);
    }

    let plot_w = (font_size * 15.0).clamp(110.0, 1200.0);
    let plot_h = (font_size * 10.5).clamp(78.0, 840.0);
    let label_fs = (font_size * 0.72).clamp(6.0, 64.0);
    let tick_fs = (font_size * 0.62).clamp(5.0, 52.0);
    let margin_left = (font_size * 2.1).max(24.0);
    let margin_top = if ax.title.is_some() {
        (font_size * 1.4).max(16.0)
    } else {
        (font_size * 0.6).max(6.0)
    };
    let plot_rect_min = Pos2::new(margin_left, margin_top);
    let plot_rect_max = Pos2::new(margin_left + plot_w, margin_top + plot_h);

    // 1. Sampel seluruh plot untuk menentukan rentang X dan Y
    struct SampledSeries<'a> {
        spec: &'a AddPlotSpec,
        runs: Vec<Vec<(f64, f64)>>,
        raw_pts: Vec<(f64, f64)>,
    }

    let mut series_list = Vec::with_capacity(ax.plots.len());
    let mut data_xmin = f64::INFINITY;
    let mut data_xmax = f64::NEG_INFINITY;
    let mut data_ymin = f64::INFINITY;
    let mut data_ymax = f64::NEG_INFINITY;

    for pspec in &ax.plots {
        let mut runs: Vec<Vec<(f64, f64)>> = Vec::new();
        let mut raw_pts = Vec::new();
        match &pspec.source {
            AddPlotSource::Expression(expr) => {
                let (d0, d1) = pspec.style.domain.unwrap_or(ax.domain);
                let d0 = ax.xmin.unwrap_or(d0);
                let d1 = ax.xmax.unwrap_or(d1);
                let n = pspec.style.samples.unwrap_or(ax.samples).clamp(8, 400);
                let mut cur_run: Vec<(f64, f64)> = Vec::new();
                for k in 0..n {
                    let t = k as f64 / ((n - 1) as f64);
                    let xv = d0 + (d1 - d0) * t;
                    if let Some(yv) = eval_math_expr(expr, xv)
                        && yv.abs() < 1e5
                    {
                        cur_run.push((xv, yv));
                        raw_pts.push((xv, yv));
                        data_xmin = data_xmin.min(xv);
                        data_xmax = data_xmax.max(xv);
                        data_ymin = data_ymin.min(yv);
                        data_ymax = data_ymax.max(yv);
                    } else if !cur_run.is_empty() {
                        runs.push(std::mem::take(&mut cur_run));
                    }
                }
                if !cur_run.is_empty() {
                    runs.push(cur_run);
                }
            }
            AddPlotSource::Coordinates(pts) => {
                let mut cur_run = Vec::new();
                for &(xv, yv) in pts {
                    if xv.is_finite() && yv.is_finite() {
                        cur_run.push((xv, yv));
                        raw_pts.push((xv, yv));
                        data_xmin = data_xmin.min(xv);
                        data_xmax = data_xmax.max(xv);
                        data_ymin = data_ymin.min(yv);
                        data_ymax = data_ymax.max(yv);
                    }
                }
                if !cur_run.is_empty() {
                    runs.push(cur_run);
                }
            }
            AddPlotSource::Surface3D(_)
            | AddPlotSource::Parametric3D(..)
            | AddPlotSource::Coordinates3D(_) => {}
        }
        series_list.push(SampledSeries {
            spec: pspec,
            runs,
            raw_pts,
        });
    }

    let xmin = ax.xmin.unwrap_or_else(|| {
        if data_xmin.is_finite() {
            data_xmin
        } else {
            ax.domain.0
        }
    });
    let xmax = ax.xmax.unwrap_or_else(|| {
        if data_xmax.is_finite() && data_xmax > xmin {
            data_xmax
        } else {
            xmin + 10.0
        }
    });

    let mut ymin = ax.ymin.unwrap_or_else(|| {
        if data_ymin.is_finite() {
            if ax.ybar || ax.plots.iter().any(|p| p.style.ybar) {
                data_ymin.min(0.0)
            } else {
                data_ymin
            }
        } else {
            -5.0
        }
    });
    let mut ymax = ax.ymax.unwrap_or_else(|| {
        if data_ymax.is_finite() {
            data_ymax
        } else {
            5.0
        }
    });
    if (ymax - ymin).abs() < 1e-6 {
        ymin -= 1.0;
        ymax += 1.0;
    } else if ax.ymin.is_none() || ax.ymax.is_none() {
        let pad = (ymax - ymin) * 0.08;
        if ax.ymin.is_none() && !(ax.ybar && (ymin - 0.0).abs() < 1e-9) {
            ymin -= pad;
        }
        if ax.ymax.is_none() {
            ymax += pad;
        }
    }

    let to_px = |xv: f64, yv: f64| -> Pos2 {
        let tx = ((xv - xmin) / (xmax - xmin).max(1e-9)) as f32;
        let ty = ((yv - ymin) / (ymax - ymin).max(1e-9)) as f32;
        Pos2::new(plot_rect_min.x + tx * plot_w, plot_rect_max.y - ty * plot_h)
    };

    let mut items = Vec::new();
    let x_ticks = compute_nice_ticks(xmin, xmax, 6);
    let y_ticks = compute_nice_ticks(ymin, ymax, 5);

    // 2. Grid lines
    if ax.grid {
        let grid_col = Some(Color32::from_rgba_unmultiplied(155, 162, 174, 130));
        for &xt in &x_ticks {
            let sx = to_px(xt, ymin).x;
            if sx >= plot_rect_min.x - 0.5 && sx <= plot_rect_max.x + 0.5 {
                items.push(MathLayoutItem::StyledLine {
                    start: Pos2::new(sx, plot_rect_min.y),
                    end: Pos2::new(sx, plot_rect_max.y),
                    stroke_width: 0.75,
                    color: grid_col,
                    dash: MathLineDash::Dotted,
                    arrow_start: false,
                    arrow_end: false,
                });
            }
        }
        for &yt in &y_ticks {
            let sy = to_px(xmin, yt).y;
            if sy >= plot_rect_min.y - 0.5 && sy <= plot_rect_max.y + 0.5 {
                items.push(MathLayoutItem::StyledLine {
                    start: Pos2::new(plot_rect_min.x, sy),
                    end: Pos2::new(plot_rect_max.x, sy),
                    stroke_width: 0.75,
                    color: grid_col,
                    dash: MathLineDash::Dotted,
                    arrow_start: false,
                    arrow_end: false,
                });
            }
        }
    }

    // 3. Sumbu X dan Y + ticks
    let axis_col = None;
    let x_axis_y = if ax.axis_lines == AxisLinesMode::Middle && ymin < 0.0 && ymax > 0.0 {
        to_px(xmin, 0.0).y
    } else {
        plot_rect_max.y
    };
    let y_axis_x = if ax.axis_lines == AxisLinesMode::Middle && xmin < 0.0 && xmax > 0.0 {
        to_px(0.0, ymin).x
    } else {
        plot_rect_min.x
    };

    match ax.axis_lines {
        AxisLinesMode::Box => {
            items.push(MathLayoutItem::Polygon {
                points: vec![
                    plot_rect_min,
                    Pos2::new(plot_rect_max.x, plot_rect_min.y),
                    plot_rect_max,
                    Pos2::new(plot_rect_min.x, plot_rect_max.y),
                ],
                fill: None,
                stroke_color: axis_col,
                stroke_width: 1.2,
            });
        }
        AxisLinesMode::Middle | AxisLinesMode::Left => {
            items.push(MathLayoutItem::StyledLine {
                start: Pos2::new(plot_rect_min.x, x_axis_y),
                end: Pos2::new(plot_rect_max.x + 8.0, x_axis_y),
                stroke_width: 1.3,
                color: axis_col,
                dash: MathLineDash::Solid,
                arrow_start: false,
                arrow_end: true,
            });
            items.push(MathLayoutItem::StyledLine {
                start: Pos2::new(y_axis_x, plot_rect_max.y),
                end: Pos2::new(y_axis_x, plot_rect_min.y - 8.0),
                stroke_width: 1.3,
                color: axis_col,
                dash: MathLineDash::Solid,
                arrow_start: false,
                arrow_end: true,
            });
        }
    }

    for &xt in &x_ticks {
        let sx = to_px(xt, ymin).x;
        if ax.axis_lines == AxisLinesMode::Middle && xt.abs() < 1e-6 && (xmin < 0.0 && xmax > 0.0) {
            continue;
        }
        items.push(MathLayoutItem::StyledLine {
            start: Pos2::new(sx, x_axis_y - 3.0),
            end: Pos2::new(sx, x_axis_y + 3.0),
            stroke_width: 1.0,
            color: axis_col,
            dash: MathLineDash::Solid,
            arrow_start: false,
            arrow_end: false,
        });
        let t_lbl = format_tick(xt);
        let tw = t_lbl.chars().count() as f32 * tick_fs * 0.52;
        items.push(MathLayoutItem::ColoredGlyph {
            text: t_lbl,
            font_size: tick_fs,
            is_italic: false,
            is_bold: false,
            pos: Pos2::new(sx - tw * 0.5, x_axis_y + 4.5),
            color: axis_col,
        });
    }
    for &yt in &y_ticks {
        let sy = to_px(xmin, yt).y;
        if ax.axis_lines == AxisLinesMode::Middle && yt.abs() < 1e-6 && (ymin < 0.0 && ymax > 0.0) {
            continue;
        }
        items.push(MathLayoutItem::StyledLine {
            start: Pos2::new(y_axis_x - 3.0, sy),
            end: Pos2::new(y_axis_x + 3.0, sy),
            stroke_width: 1.0,
            color: axis_col,
            dash: MathLineDash::Solid,
            arrow_start: false,
            arrow_end: false,
        });
        let t_lbl = format_tick(yt);
        let tw = t_lbl.chars().count() as f32 * tick_fs * 0.54;
        items.push(MathLayoutItem::ColoredGlyph {
            text: t_lbl,
            font_size: tick_fs,
            is_italic: false,
            is_bold: false,
            pos: Pos2::new(y_axis_x - tw - 6.0, sy - tick_fs * 0.55),
            color: axis_col,
        });
    }

    // 4. Kurva / Batang / Penanda
    for s in &series_list {
        let series_col = s
            .spec
            .style
            .draw_color
            .unwrap_or(Color32::from_rgb(24, 110, 214));
        if s.spec.style.ybar {
            let zero_y = to_px(xmin, 0.0).y.clamp(plot_rect_min.y, plot_rect_max.y);
            let bar_w = (plot_w / (s.raw_pts.len().max(1) as f32 * 2.0))
                .clamp(4.0, (plot_w * 0.25).max(8.0));
            let fill_c = apply_fill_opacity(Some(series_col), 0.72);
            for &(xv, yv) in &s.raw_pts {
                if xv < xmin || xv > xmax {
                    continue;
                }
                let pt = to_px(xv, yv.clamp(ymin, ymax));
                let y_top = pt.y.min(zero_y);
                let y_bot = pt.y.max(zero_y);
                items.push(MathLayoutItem::Polygon {
                    points: vec![
                        Pos2::new(pt.x - bar_w * 0.5, y_top),
                        Pos2::new(pt.x + bar_w * 0.5, y_top),
                        Pos2::new(pt.x + bar_w * 0.5, y_bot),
                        Pos2::new(pt.x - bar_w * 0.5, y_bot),
                    ],
                    fill: fill_c,
                    stroke_color: Some(series_col),
                    stroke_width: 1.2,
                });
            }
            continue;
        }

        if !s.spec.style.only_marks {
            for run in &s.runs {
                let mut clipped_seg: Vec<Pos2> = Vec::new();
                let mut prev_in: Option<(f64, f64)> = None;
                for &(xv, yv) in run {
                    let inside = yv >= ymin && yv <= ymax && xv >= xmin && xv <= xmax;
                    if let Some((pxv, pyv)) = prev_in {
                        // Deteksi lompatan asimtot curam (seperti tan(x) atau 1/x)
                        if (yv - pyv).abs() > (ymax - ymin) * 1.4
                            && (yv - ymax) * (pyv - ymin) != 0.0
                        {
                            if clipped_seg.len() >= 2 {
                                items.push(MathLayoutItem::Polyline {
                                    points: std::mem::take(&mut clipped_seg),
                                    stroke_width: s.spec.style.stroke_width,
                                    color: Some(series_col),
                                    dash: s.spec.style.dash,
                                    arrow_start: false,
                                    arrow_end: false,
                                });
                            } else {
                                clipped_seg.clear();
                            }
                        } else if !inside && clipped_seg.len() >= 2 {
                            let edge_p = to_px(xv.clamp(xmin, xmax), yv.clamp(ymin, ymax));
                            clipped_seg.push(edge_p);
                            items.push(MathLayoutItem::Polyline {
                                points: std::mem::take(&mut clipped_seg),
                                stroke_width: s.spec.style.stroke_width,
                                color: Some(series_col),
                                dash: s.spec.style.dash,
                                arrow_start: false,
                                arrow_end: false,
                            });
                        } else if inside && clipped_seg.is_empty() && (pxv >= xmin && pxv <= xmax) {
                            clipped_seg.push(to_px(pxv.clamp(xmin, xmax), pyv.clamp(ymin, ymax)));
                        }
                    }
                    if inside {
                        clipped_seg.push(to_px(xv, yv));
                    } else if clipped_seg.len() >= 2 {
                        items.push(MathLayoutItem::Polyline {
                            points: std::mem::take(&mut clipped_seg),
                            stroke_width: s.spec.style.stroke_width,
                            color: Some(series_col),
                            dash: s.spec.style.dash,
                            arrow_start: false,
                            arrow_end: false,
                        });
                    } else {
                        clipped_seg.clear();
                    }
                    prev_in = Some((xv, yv));
                }
                if clipped_seg.len() >= 2 {
                    items.push(MathLayoutItem::Polyline {
                        points: clipped_seg,
                        stroke_width: s.spec.style.stroke_width,
                        color: Some(series_col),
                        dash: s.spec.style.dash,
                        arrow_start: false,
                        arrow_end: false,
                    });
                }
            }
        }

        if s.spec.style.mark != PlotMark::None || s.spec.style.only_marks {
            let r = 3.2_f32;
            let fill = if s.spec.style.mark == PlotMark::OpenCircle {
                Some(Color32::WHITE)
            } else {
                Some(series_col)
            };
            for &(xv, yv) in &s.raw_pts {
                if xv >= xmin && xv <= xmax && yv >= ymin && yv <= ymax {
                    let pt = to_px(xv, yv);
                    items.push(MathLayoutItem::Ellipse {
                        center: pt,
                        rx: r,
                        ry: r,
                        fill,
                        stroke_color: Some(series_col),
                        stroke_width: 1.2,
                    });
                }
            }
        }
    }

    // Overlay TikZ opsional di dalam axis
    for ov in &ax.overlays {
        match ov {
            TikzStmt::Node {
                coord: TikzCoord::Point(xv, yv),
                style,
                label,
                ..
            } => {
                let pt = to_px(*xv, *yv);
                emit_label(
                    &mut items,
                    pt,
                    label,
                    label_fs,
                    style.node_anchor,
                    style.text_color,
                    4.0,
                );
            }
            TikzStmt::DrawPath { style, segments } => {
                let mut cur = Pos2::ZERO;
                for seg in segments {
                    match seg {
                        PathSegment::MoveTo(TikzCoord::Point(xv, yv)) => cur = to_px(*xv, *yv),
                        PathSegment::LineTo(TikzCoord::Point(xv, yv), _) => {
                            let next = to_px(*xv, *yv);
                            items.push(MathLayoutItem::StyledLine {
                                start: cur,
                                end: next,
                                stroke_width: style.stroke_width,
                                color: style.draw_color,
                                dash: style.dash,
                                arrow_start: style.arrow_start,
                                arrow_end: style.arrow_end,
                            });
                            cur = next;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // 5. Title & Axis Labels
    if let Some(title) = &ax.title {
        let (clean, _) = clean_inline_label(title);
        let tw = clean.chars().count() as f32 * font_size * 0.48;
        items.push(MathLayoutItem::ColoredGlyph {
            text: clean,
            font_size: (font_size * 0.88).clamp(11.0, 26.0),
            is_italic: false,
            is_bold: true,
            pos: Pos2::new(plot_rect_min.x + (plot_w - tw) * 0.5, 4.0),
            color: axis_col,
        });
    }
    if let Some(xl) = &ax.xlabel {
        let (clean, is_math) = clean_inline_label(xl);
        let tw = clean.chars().count() as f32 * label_fs * 0.52;
        items.push(MathLayoutItem::ColoredGlyph {
            text: clean,
            font_size: label_fs,
            is_italic: is_math,
            is_bold: false,
            pos: Pos2::new(
                plot_rect_min.x + (plot_w - tw) * 0.5,
                plot_rect_max.y + tick_fs + 10.0,
            ),
            color: axis_col,
        });
    }
    if let Some(yl) = &ax.ylabel {
        let (clean, is_math) = clean_inline_label(yl);
        items.push(MathLayoutItem::ColoredGlyph {
            text: clean,
            font_size: label_fs,
            is_italic: is_math,
            is_bold: false,
            pos: Pos2::new(2.0, plot_rect_min.y + plot_h * 0.45),
            color: axis_col,
        });
    }

    // 6. Legend Box
    if !ax.legend_entries.is_empty() {
        let max_len = ax
            .legend_entries
            .iter()
            .map(|e| clean_inline_label(e).0.chars().count())
            .max()
            .unwrap_or(4);
        let leg_w = (max_len as f32 * tick_fs * 0.58 + 28.0).clamp(64.0, plot_w * 0.6);
        let line_h = tick_fs * 1.35;
        let leg_h = ax.legend_entries.len() as f32 * line_h + 8.0;
        let leg_x = plot_rect_max.x - leg_w - 8.0;
        let leg_y = plot_rect_min.y + 8.0;

        items.push(MathLayoutItem::Polygon {
            points: vec![
                Pos2::new(leg_x, leg_y),
                Pos2::new(leg_x + leg_w, leg_y),
                Pos2::new(leg_x + leg_w, leg_y + leg_h),
                Pos2::new(leg_x, leg_y + leg_h),
            ],
            fill: Some(Color32::from_rgba_unmultiplied(255, 255, 255, 220)),
            stroke_color: Some(Color32::from_rgb(175, 182, 194)),
            stroke_width: 0.9,
        });

        for (idx, raw_entry) in ax.legend_entries.iter().enumerate() {
            let ey = leg_y + 5.0 + idx as f32 * line_h;
            let sw_col = ax
                .plots
                .get(idx)
                .and_then(|p| p.style.draw_color)
                .unwrap_or(Color32::from_rgb(24, 110, 214));
            items.push(MathLayoutItem::StyledLine {
                start: Pos2::new(leg_x + 6.0, ey + tick_fs * 0.55),
                end: Pos2::new(leg_x + 20.0, ey + tick_fs * 0.55),
                stroke_width: 2.0,
                color: Some(sw_col),
                dash: ax
                    .plots
                    .get(idx)
                    .map(|p| p.style.dash)
                    .unwrap_or(MathLineDash::Solid),
                arrow_start: false,
                arrow_end: false,
            });
            let (clean_e, is_m) = clean_inline_label(raw_entry);
            items.push(MathLayoutItem::ColoredGlyph {
                text: clean_e,
                font_size: tick_fs,
                is_italic: is_m,
                is_bold: false,
                pos: Pos2::new(leg_x + 24.0, ey),
                color: Some(Color32::from_rgb(35, 38, 45)),
            });
        }
    }

    items
}

/// Menghitung tata letak 2D lengkap untuk diagram geometri (`tikzpicture`) atau grafik (`axis`),
/// menormalisasi bounding box sehingga semua elemen berada rapi pada `[6.0.., 6.0..]`.
pub fn layout_diagram(spec: &DiagramSpec, font_size: f32) -> (Vec<MathLayoutItem>, f32, f32) {
    let mut items = match spec {
        DiagramSpec::Tikz(pic) => layout_tikzpicture(pic, font_size),
        DiagramSpec::Axis(ax) => layout_axis_plot(ax, font_size),
    };
    if items.is_empty() {
        return (items, 40.0, 40.0);
    }

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for it in &items {
        match it {
            MathLayoutItem::Glyph {
                text,
                font_size: fs,
                pos,
                ..
            }
            | MathLayoutItem::ColoredGlyph {
                text,
                font_size: fs,
                pos,
                ..
            } => {
                let w = text.chars().count() as f32 * fs * 0.58;
                let h = fs * 1.25;
                min_x = min_x.min(pos.x);
                min_y = min_y.min(pos.y);
                max_x = max_x.max(pos.x + w);
                max_y = max_y.max(pos.y + h);
            }
            MathLayoutItem::Line { start, end, .. }
            | MathLayoutItem::StyledLine { start, end, .. } => {
                min_x = min_x.min(start.x).min(end.x);
                min_y = min_y.min(start.y).min(end.y);
                max_x = max_x.max(start.x).max(end.x);
                max_y = max_y.max(start.y).max(end.y);
            }
            MathLayoutItem::Polyline { points, .. } | MathLayoutItem::Polygon { points, .. } => {
                for p in points {
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }
            }
            MathLayoutItem::Ellipse { center, rx, ry, .. } => {
                min_x = min_x.min(center.x - rx);
                min_y = min_y.min(center.y - ry);
                max_x = max_x.max(center.x + rx);
                max_y = max_y.max(center.y + ry);
            }
        }
    }

    if !min_x.is_finite() || !min_y.is_finite() {
        return (items, 60.0, 60.0);
    }

    let pad = 10.0_f32;
    let dx = pad - min_x;
    let dy = pad - min_y;
    for it in &mut items {
        super::table::shift_item(it, dx, dy);
    }
    let w = (max_x - min_x + pad * 2.0).max(24.0);
    let h = (max_y - min_y + pad * 2.0).max(24.0);
    (items, w, h)
}
