use super::{
    MathLayout, MathLayoutItem, MathLineDash, MathParseError, extract_braced_group,
    extract_bracket_group, strip_latex_comments,
};
use eframe::egui::Pos2;

const MAX_TABLE_ROWS: usize = 100;
const MAX_TABLE_COLS: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableColAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableBorder {
    None,
    Single,
    Double,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HLineSpec {
    Full { thickness_mult: f32, double: bool },
    Partial { start_col: usize, end_col: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableCell {
    pub span: usize,
    pub align: Option<TableColAlign>,
    pub right_border: TableBorder,
    pub content: String,
    pub is_bold: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    pub bottom_hlines: Vec<HLineSpec>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableEnvKind {
    Tabular,
    Array,
    Matrix,
    PMatrix,
    BMatrix,
    BracesMatrix,
    VMatrix,
    DoubleVMatrix,
    Cases,
}

impl TableEnvKind {
    pub fn from_env_name(name: &str) -> Option<Self> {
        match name {
            "tabular" | "tabular*" => Some(Self::Tabular),
            "array" => Some(Self::Array),
            "matrix" => Some(Self::Matrix),
            "pmatrix" => Some(Self::PMatrix),
            "bmatrix" => Some(Self::BMatrix),
            "Bmatrix" => Some(Self::BracesMatrix),
            "vmatrix" => Some(Self::VMatrix),
            "Vmatrix" => Some(Self::DoubleVMatrix),
            "cases" => Some(Self::Cases),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableSpec {
    pub kind: TableEnvKind,
    pub col_aligns: Vec<TableColAlign>,
    /// Panjangnya `col_aligns.len() + 1` (batas vertikal kiri tiap kolom & kanan kolom terakhir).
    pub col_borders: Vec<TableBorder>,
    pub top_hlines: Vec<HLineSpec>,
    pub rows: Vec<TableRow>,
}

fn parse_col_spec(spec: &str) -> Result<(Vec<TableColAlign>, Vec<TableBorder>), MathParseError> {
    let mut aligns = Vec::new();
    let mut borders = vec![TableBorder::None];
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '|' => {
                if let Some(last) = borders.last_mut() {
                    *last = match *last {
                        TableBorder::None => TableBorder::Single,
                        TableBorder::Single | TableBorder::Double => TableBorder::Double,
                    };
                }
                i += 1;
            }
            'l' | 'p' | 'm' | 'b' => {
                aligns.push(TableColAlign::Left);
                borders.push(TableBorder::None);
                i += 1;
                if matches!(c, 'p' | 'm' | 'b') && chars.get(i) == Some(&'{') {
                    let rest: String = chars[i..].iter().collect();
                    if let Some((_, adv)) = extract_braced_group(&rest, 0) {
                        i += adv;
                    }
                }
            }
            'c' => {
                aligns.push(TableColAlign::Center);
                borders.push(TableBorder::None);
                i += 1;
            }
            'r' => {
                aligns.push(TableColAlign::Right);
                borders.push(TableBorder::None);
                i += 1;
            }
            '@' | '!' => {
                i += 1;
                let rest: String = chars[i..].iter().collect();
                if let Some((_, adv)) = extract_braced_group(&rest, 0) {
                    i += adv;
                }
            }
            '*' => {
                i += 1;
                let rest: String = chars[i..].iter().collect();
                if let Some((cnt_s, adv1)) = extract_braced_group(&rest, 0) {
                    i += adv1;
                    let rest2: String = chars[i..].iter().collect();
                    if let Some((sub_spec, adv2)) = extract_braced_group(&rest2, 0) {
                        i += adv2;
                        let n = cnt_s
                            .trim()
                            .parse::<usize>()
                            .unwrap_or(1)
                            .min(MAX_TABLE_COLS);
                        let (sub_a, sub_b) = parse_col_spec(&sub_spec)?;
                        for _ in 0..n {
                            for (idx, a) in sub_a.iter().enumerate() {
                                if aligns.len() >= MAX_TABLE_COLS {
                                    break;
                                }
                                if idx == 0
                                    && let Some(lb) = borders.last_mut()
                                    && *lb == TableBorder::None
                                {
                                    *lb = sub_b[0];
                                }
                                aligns.push(*a);
                                borders.push(sub_b[idx + 1]);
                            }
                        }
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
        if aligns.len() > MAX_TABLE_COLS {
            return Err(MathParseError {
                message: "Jumlah kolom tabel melebihi batas maksimum".into(),
                position: i,
            });
        }
    }
    Ok((aligns, borders))
}

fn split_top_level_rows(body: &str) -> Vec<String> {
    let mut rows = Vec::new();
    let mut cur = String::new();
    let bytes = body.as_bytes();
    let mut i = 0;
    let mut brace_depth = 0usize;
    let mut env_depth = 0usize;

    while i < bytes.len() {
        let ch = body[i..].chars().next().unwrap();
        let ch_len = ch.len_utf8();
        if ch == '\\' {
            if body[i..].starts_with(r"\\") && brace_depth == 0 && env_depth == 0 {
                rows.push(std::mem::take(&mut cur));
                i += 2;
                // Lewati jarak baris opsional seperti \\[4pt]
                while i < bytes.len() && body[i..].starts_with(' ') {
                    i += 1;
                }
                if i < bytes.len()
                    && body.as_bytes()[i] == b'['
                    && let Some((_, adv)) = extract_bracket_group(body, i)
                {
                    i += adv;
                }
                continue;
            }
            if body[i..].starts_with(r"\begin") {
                env_depth += 1;
            } else if body[i..].starts_with(r"\end") {
                env_depth = env_depth.saturating_sub(1);
            }
        } else if ch == '{' {
            brace_depth += 1;
        } else if ch == '}' {
            brace_depth = brace_depth.saturating_sub(1);
        }
        cur.push(ch);
        i += ch_len;
    }
    if !cur.trim().is_empty() {
        rows.push(cur);
    }
    rows
}

fn split_cells(row_str: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut brace_depth = 0usize;
    let mut env_depth = 0usize;
    let mut i = 0;

    while i < row_str.len() {
        let ch = row_str[i..].chars().next().unwrap();
        let ch_len = ch.len_utf8();
        if ch == '\\' {
            if row_str[i..].starts_with(r"\&") {
                cur.push('&');
                i += 2;
                continue;
            }
            if row_str[i..].starts_with(r"\begin") {
                env_depth += 1;
            } else if row_str[i..].starts_with(r"\end") {
                env_depth = env_depth.saturating_sub(1);
            }
        } else if ch == '{' {
            brace_depth += 1;
        } else if ch == '}' {
            brace_depth = brace_depth.saturating_sub(1);
        } else if ch == '&' && brace_depth == 0 && env_depth == 0 {
            cells.push(std::mem::take(&mut cur));
            i += 1;
            continue;
        }
        cur.push(ch);
        i += ch_len;
    }
    cells.push(cur);
    cells
}

fn extract_leading_rules(mut s: &str) -> (Vec<HLineSpec>, &str) {
    let mut rules = Vec::new();
    loop {
        s = s.trim_start();
        if let Some(rest) = s.strip_prefix(r"\hline") {
            if let Some(HLineSpec::Full { double, .. }) = rules.last_mut()
                && !*double
            {
                *double = true;
            } else {
                rules.push(HLineSpec::Full {
                    thickness_mult: 1.0,
                    double: false,
                });
            }
            s = rest;
        } else if let Some(rest) = s.strip_prefix(r"\toprule") {
            rules.push(HLineSpec::Full {
                thickness_mult: 1.6,
                double: false,
            });
            s = rest;
        } else if let Some(rest) = s.strip_prefix(r"\midrule") {
            rules.push(HLineSpec::Full {
                thickness_mult: 1.0,
                double: false,
            });
            s = rest;
        } else if let Some(rest) = s.strip_prefix(r"\bottomrule") {
            rules.push(HLineSpec::Full {
                thickness_mult: 1.6,
                double: false,
            });
            s = rest;
        } else if let Some(rest) = s.strip_prefix(r"\cline") {
            if let Some((range_str, adv)) = extract_braced_group(rest, 0) {
                let parts: Vec<&str> = range_str.split('-').collect();
                if parts.len() == 2
                    && let (Ok(a), Ok(b)) = (
                        parts[0].trim().parse::<usize>(),
                        parts[1].trim().parse::<usize>(),
                    )
                    && a >= 1
                    && b >= a
                {
                    rules.push(HLineSpec::Partial {
                        start_col: a - 1,
                        end_col: b - 1,
                    });
                }
                s = &rest[adv..];
            } else {
                break;
            }
        } else {
            break;
        }
    }
    (rules, s)
}

fn strip_trailing_rules(mut s: &str) -> (&str, Vec<HLineSpec>) {
    let mut trailing = Vec::new();
    loop {
        let trimmed = s.trim_end();
        if let Some(idx) = trimmed.rfind(r"\hline")
            && trimmed[idx + 6..].trim().is_empty()
        {
            trailing.push(HLineSpec::Full {
                thickness_mult: 1.0,
                double: false,
            });
            s = &trimmed[..idx];
            continue;
        }
        if let Some(idx) = trimmed.rfind(r"\bottomrule")
            && trimmed[idx + 11..].trim().is_empty()
        {
            trailing.push(HLineSpec::Full {
                thickness_mult: 1.6,
                double: false,
            });
            s = &trimmed[..idx];
            continue;
        }
        if let Some(idx) = trimmed.rfind(r"\midrule")
            && trimmed[idx + 8..].trim().is_empty()
        {
            trailing.push(HLineSpec::Full {
                thickness_mult: 1.0,
                double: false,
            });
            s = &trimmed[..idx];
            continue;
        }
        s = trimmed;
        break;
    }
    trailing.reverse();
    (s, trailing)
}

fn parse_single_cell(raw: &str) -> TableCell {
    let mut s = raw.trim().to_string();
    let mut span = 1usize;
    let mut align = None;
    let mut right_border = TableBorder::None;
    let mut is_bold = false;

    if let Some(after_mc) = s.strip_prefix(r"\multicolumn")
        && let Some((span_str, adv1)) = extract_braced_group(after_mc, 0)
        && let Some((spec_str, adv2)) = extract_braced_group(&after_mc[adv1..], 0)
        && let Some((inner, _)) = extract_braced_group(&after_mc[adv1 + adv2..], 0)
    {
        span = span_str.trim().parse::<usize>().unwrap_or(1).max(1);
        if spec_str.contains('r') {
            align = Some(TableColAlign::Right);
        } else if spec_str.contains('c') {
            align = Some(TableColAlign::Center);
        } else if spec_str.contains('l') {
            align = Some(TableColAlign::Left);
        }
        let pipe_cnt = spec_str.chars().filter(|&c| c == '|').count();
        if pipe_cnt >= 2 {
            right_border = TableBorder::Double;
        } else if pipe_cnt == 1 && !spec_str.trim().starts_with('|') || pipe_cnt >= 2 {
            right_border = TableBorder::Single;
        }
        s = inner.trim().to_string();
    }

    if let Some(after_bf) = s
        .strip_prefix(r"\textbf")
        .or_else(|| s.strip_prefix(r"\mathbf"))
        && let Some((inner, adv)) = extract_braced_group(after_bf, 0)
        && after_bf[adv..].trim().is_empty()
    {
        is_bold = true;
        s = inner.trim().to_string();
    }

    TableCell {
        span,
        align,
        right_border,
        content: s,
        is_bold,
    }
}

/// Mem-parse lingkungan tabel / matriks LaTeX (`tabular`, `array`, `pmatrix`, `cases`, dll.).
pub fn parse_table_env(
    env_name: &str,
    col_spec_opt: Option<&str>,
    body: &str,
) -> Result<TableSpec, MathParseError> {
    let kind = TableEnvKind::from_env_name(env_name).ok_or_else(|| MathParseError {
        message: format!("Lingkungan tabel tidak dikenal: '{env_name}'"),
        position: 0,
    })?;

    let clean_body = strip_latex_comments(body);
    let raw_rows = split_top_level_rows(&clean_body);
    if raw_rows.len() > MAX_TABLE_ROWS {
        return Err(MathParseError {
            message: "Jumlah baris tabel melebihi batas maksimum".into(),
            position: 0,
        });
    }

    let (mut col_aligns, mut col_borders) = if let Some(cs) = col_spec_opt {
        parse_col_spec(cs)?
    } else {
        (Vec::new(), Vec::new())
    };

    let mut top_hlines = Vec::new();
    let mut rows: Vec<TableRow> = Vec::new();

    for raw in raw_rows {
        let (leading_rules, rest_after_lead) = extract_leading_rules(&raw);
        let (cell_part, trailing_rules) = strip_trailing_rules(rest_after_lead);

        if !leading_rules.is_empty() {
            if let Some(prev) = rows.last_mut() {
                prev.bottom_hlines.extend(leading_rules);
            } else {
                top_hlines.extend(leading_rules);
            }
        }

        if !cell_part.trim().is_empty() {
            let parsed_cells: Vec<TableCell> = split_cells(cell_part)
                .into_iter()
                .map(|c| parse_single_cell(&c))
                .collect();
            rows.push(TableRow {
                cells: parsed_cells,
                bottom_hlines: trailing_rules,
            });
        } else if !trailing_rules.is_empty() {
            if let Some(prev) = rows.last_mut() {
                prev.bottom_hlines.extend(trailing_rules);
            } else {
                top_hlines.extend(trailing_rules);
            }
        }
    }

    let max_cols = rows
        .iter()
        .map(|r| r.cells.iter().map(|c| c.span).sum::<usize>())
        .max()
        .unwrap_or(1)
        .clamp(1, MAX_TABLE_COLS);

    let default_align = if kind == TableEnvKind::Cases {
        TableColAlign::Left
    } else {
        TableColAlign::Center
    };
    while col_aligns.len() < max_cols {
        col_aligns.push(default_align);
    }
    while col_borders.len() < col_aligns.len() + 1 {
        col_borders.push(TableBorder::None);
    }

    Ok(TableSpec {
        kind,
        col_aligns,
        col_borders,
        top_hlines,
        rows,
    })
}

struct RenderedCell {
    items: Vec<MathLayoutItem>,
    width: f32,
    height: f32,
    align: TableColAlign,
    span: usize,
    right_border: TableBorder,
}

fn layout_cell_content(
    content: &str,
    font_size: f32,
    is_bold: bool,
    force_math: bool,
) -> (Vec<MathLayoutItem>, f32, f32) {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return (Vec::new(), 0.0, font_size * 1.25);
    }

    // Jika dibungkus $...$, render sebagai rumus matematika penuh
    if let Some(math_inner) = trimmed.strip_prefix('$').and_then(|s| s.strip_suffix('$')) {
        let ml = MathLayout::compute_from_str(math_inner, font_size);
        return (ml.items, ml.width, ml.height);
    }

    let has_math_syntax = force_math
        || trimmed.contains('\\')
        || trimmed.contains('^')
        || trimmed.contains('_')
        || trimmed.contains('$');

    if !has_math_syntax && !is_bold {
        let w = trimmed.chars().count() as f32 * font_size * 0.56;
        let h = font_size * 1.3;
        return (
            vec![MathLayoutItem::ColoredGlyph {
                text: trimmed.to_string(),
                font_size,
                is_italic: false,
                is_bold: false,
                pos: Pos2::ZERO,
                color: None,
            }],
            w,
            h,
        );
    }

    if !trimmed.contains('\\') && !trimmed.contains('^') && !trimmed.contains('_') && is_bold {
        let w = trimmed.chars().count() as f32 * font_size * 0.6;
        let h = font_size * 1.3;
        return (
            vec![MathLayoutItem::ColoredGlyph {
                text: trimmed.to_string(),
                font_size,
                is_italic: false,
                is_bold: true,
                pos: Pos2::ZERO,
                color: None,
            }],
            w,
            h,
        );
    }

    // Campuran teks & math `$...$` di dalam sel tabel
    if trimmed.contains('$') {
        let mut items = Vec::new();
        let mut cur_x = 0.0_f32;
        let mut max_h = font_size * 1.3;
        let mut in_math = false;
        for part in trimmed.split('$') {
            if !part.is_empty() {
                let (sub_items, sub_w, sub_h) =
                    layout_cell_content(part, font_size, is_bold, in_math);
                max_h = max_h.max(sub_h);
                for mut it in sub_items {
                    shift_item(&mut it, cur_x, 0.0);
                    items.push(it);
                }
                cur_x += sub_w;
            }
            in_math = !in_math;
        }
        return (items, cur_x, max_h);
    }

    let ml = MathLayout::compute_from_str(trimmed, font_size);
    if is_bold {
        let mut bold_items = Vec::with_capacity(ml.items.len());
        for it in ml.items {
            match it {
                MathLayoutItem::Glyph {
                    text,
                    font_size: fs,
                    is_italic,
                    pos,
                } => bold_items.push(MathLayoutItem::ColoredGlyph {
                    text,
                    font_size: fs,
                    is_italic,
                    is_bold: true,
                    pos,
                    color: None,
                }),
                other => bold_items.push(other),
            }
        }
        (bold_items, ml.width, ml.height)
    } else {
        (ml.items, ml.width, ml.height)
    }
}

pub(crate) fn shift_item(item: &mut MathLayoutItem, dx: f32, dy: f32) {
    match item {
        MathLayoutItem::Glyph { pos, .. } | MathLayoutItem::ColoredGlyph { pos, .. } => {
            pos.x += dx;
            pos.y += dy;
        }
        MathLayoutItem::Line { start, end, .. } | MathLayoutItem::StyledLine { start, end, .. } => {
            start.x += dx;
            start.y += dy;
            end.x += dx;
            end.y += dy;
        }
        MathLayoutItem::Polyline { points, .. } | MathLayoutItem::Polygon { points, .. } => {
            for p in points {
                p.x += dx;
                p.y += dy;
            }
        }
        MathLayoutItem::Ellipse { center, .. } => {
            center.x += dx;
            center.y += dy;
        }
    }
}

/// Menghitung tata letak 2D lengkap untuk tabel & matriks LaTeX.
pub fn layout_table(spec: &TableSpec, font_size: f32) -> (Vec<MathLayoutItem>, f32, f32) {
    let is_matrix_like = !matches!(spec.kind, TableEnvKind::Tabular);
    let has_brackets = matches!(
        spec.kind,
        TableEnvKind::PMatrix
            | TableEnvKind::BMatrix
            | TableEnvKind::BracesMatrix
            | TableEnvKind::VMatrix
            | TableEnvKind::DoubleVMatrix
            | TableEnvKind::Cases
    );

    let pad_x = if is_matrix_like {
        (font_size * 0.42).max(6.0)
    } else {
        (font_size * 0.55).max(8.0)
    };
    let pad_y = (font_size * 0.30).max(5.0);
    let bracket_w = if has_brackets {
        (font_size * 0.55).max(9.0)
    } else {
        0.0
    };
    let num_cols = spec.col_aligns.len().max(1);

    let mut rendered_rows: Vec<Vec<RenderedCell>> = Vec::with_capacity(spec.rows.len());
    let mut col_widths = vec![(font_size * 1.1).max(16.0); num_cols];

    for row in &spec.rows {
        let mut r_cells = Vec::new();
        let mut col_idx = 0usize;
        for cell in &row.cells {
            let (items, w, h) =
                layout_cell_content(&cell.content, font_size, cell.is_bold, is_matrix_like);
            let default_align = spec
                .col_aligns
                .get(col_idx)
                .copied()
                .unwrap_or(TableColAlign::Center);
            let eff_align = cell.align.unwrap_or(default_align);
            let span = cell
                .span
                .max(1)
                .min(num_cols.saturating_sub(col_idx).max(1));
            let needed = w + pad_x * 2.0;
            if span == 1 && col_idx < col_widths.len() {
                col_widths[col_idx] = col_widths[col_idx].max(needed);
            }
            r_cells.push(RenderedCell {
                items,
                width: w,
                height: h,
                align: eff_align,
                span,
                right_border: cell.right_border,
            });
            col_idx += span;
        }
        rendered_rows.push(r_cells);
    }

    // Distribusikan lebar berlebih sel \multicolumn ke kolom yang dicakupnya
    for row in &rendered_rows {
        let mut c = 0usize;
        for cell in row {
            if cell.span > 1 && c + cell.span <= col_widths.len() {
                let sum_w: f32 = col_widths[c..c + cell.span].iter().sum();
                let needed = cell.width + pad_x * 2.0;
                if needed > sum_w {
                    let add_each = (needed - sum_w) / (cell.span as f32);
                    for cw in &mut col_widths[c..c + cell.span] {
                        *cw += add_each;
                    }
                }
            }
            c += cell.span;
        }
    }

    let mut col_x = Vec::with_capacity(num_cols + 1);
    let mut acc_x = bracket_w;
    col_x.push(acc_x);
    for w in &col_widths {
        acc_x += *w;
        col_x.push(acc_x);
    }
    let table_left = col_x[0];
    let table_right = *col_x.last().unwrap_or(&table_left);

    let mut row_heights = Vec::with_capacity(rendered_rows.len());
    for row in &rendered_rows {
        let max_h = row
            .iter()
            .map(|c| c.height)
            .fold(font_size * 1.25, f32::max);
        row_heights.push(max_h + pad_y * 2.0);
    }

    let base_rule_w = (font_size * 0.065).clamp(1.0, 2.5);
    let mut items = Vec::new();
    let double_gap = 2.6_f32;
    let mut cur_y = 2.0_f32;

    let emit_hlines = |rules: &[HLineSpec], y_base: &mut f32, target: &mut Vec<MathLayoutItem>| {
        for (idx, rule) in rules.iter().enumerate() {
            match rule {
                HLineSpec::Full {
                    thickness_mult,
                    double,
                } => {
                    let sw = base_rule_w * *thickness_mult;
                    target.push(MathLayoutItem::StyledLine {
                        start: Pos2::new(table_left, *y_base),
                        end: Pos2::new(table_right, *y_base),
                        stroke_width: sw,
                        color: None,
                        dash: MathLineDash::Solid,
                        arrow_start: false,
                        arrow_end: false,
                    });
                    if *double {
                        *y_base += double_gap;
                        target.push(MathLayoutItem::StyledLine {
                            start: Pos2::new(table_left, *y_base),
                            end: Pos2::new(table_right, *y_base),
                            stroke_width: sw,
                            color: None,
                            dash: MathLineDash::Solid,
                            arrow_start: false,
                            arrow_end: false,
                        });
                    }
                    if idx + 1 < rules.len() {
                        *y_base += double_gap;
                    }
                }
                HLineSpec::Partial { start_col, end_col } => {
                    if *start_col < num_cols && *end_col >= *start_col {
                        let c1 = *start_col;
                        let c2 = (*end_col).min(num_cols - 1);
                        target.push(MathLayoutItem::StyledLine {
                            start: Pos2::new(col_x[c1], *y_base),
                            end: Pos2::new(col_x[c2 + 1], *y_base),
                            stroke_width: base_rule_w,
                            color: None,
                            dash: MathLineDash::Solid,
                            arrow_start: false,
                            arrow_end: false,
                        });
                    }
                }
            }
        }
    };

    let table_top = cur_y;
    emit_hlines(&spec.top_hlines, &mut cur_y, &mut items);

    type RowSpanMeta = (f32, f32, Vec<(usize, usize, TableBorder)>);
    let mut row_spans_y: Vec<RowSpanMeta> = Vec::new();
    for (r_idx, row) in rendered_rows.into_iter().enumerate() {
        let rh = row_heights[r_idx];
        let row_top = cur_y;
        let row_bottom = cur_y + rh;
        let mut c_idx = 0usize;
        let mut row_cell_spans = Vec::new();

        for cell in row {
            let end_col = (c_idx + cell.span).min(num_cols);
            let cell_left = col_x[c_idx];
            let cell_right = col_x[end_col];
            let cell_w = (cell_right - cell_left).max(1.0);

            let inner_x = match cell.align {
                TableColAlign::Left => cell_left + pad_x,
                TableColAlign::Center => cell_left + (cell_w - cell.width) * 0.5,
                TableColAlign::Right => (cell_right - pad_x - cell.width).max(cell_left),
            };
            let inner_y = row_top + (rh - cell.height) * 0.5;
            for mut it in cell.items {
                shift_item(&mut it, inner_x, inner_y);
                items.push(it);
            }
            row_cell_spans.push((c_idx, end_col, cell.right_border));
            c_idx = end_col;
        }

        row_spans_y.push((row_top, row_bottom, row_cell_spans));
        cur_y = row_bottom;
        if let Some(raw_row) = spec.rows.get(r_idx) {
            emit_hlines(&raw_row.bottom_hlines, &mut cur_y, &mut items);
        }
    }
    let table_bottom = cur_y;

    // Garis vertikal tabel per baris (menghormati sel \multicolumn agar garis dalam tidak menembus teks span)
    for &(ry_top, ry_bot, ref spans) in &row_spans_y {
        for (b_idx, &border) in spec.col_borders.iter().enumerate() {
            if b_idx >= col_x.len() {
                break;
            }
            let override_border = spans
                .iter()
                .find(|&&(_, end_c, rb)| end_c == b_idx && rb != TableBorder::None)
                .map(|&(_, _, rb)| rb);
            let is_cut_by_span = spans.iter().any(|&(s, e, _)| b_idx > s && b_idx < e);
            if is_cut_by_span {
                continue;
            }
            let eff_border = override_border.unwrap_or(border);
            let bx = col_x[b_idx];
            match eff_border {
                TableBorder::None => {}
                TableBorder::Single => items.push(MathLayoutItem::Line {
                    start: Pos2::new(bx, ry_top),
                    end: Pos2::new(bx, ry_bot),
                    stroke_width: base_rule_w,
                }),
                TableBorder::Double => {
                    items.push(MathLayoutItem::Line {
                        start: Pos2::new(bx - double_gap * 0.5, ry_top),
                        end: Pos2::new(bx - double_gap * 0.5, ry_bot),
                        stroke_width: base_rule_w,
                    });
                    items.push(MathLayoutItem::Line {
                        start: Pos2::new(bx + double_gap * 0.5, ry_top),
                        end: Pos2::new(bx + double_gap * 0.5, ry_bot),
                        stroke_width: base_rule_w,
                    });
                }
            }
        }
    }

    // Kurung pembatas matriks vektor yang mengikuti tinggi matriks
    if has_brackets {
        let right_b_x = table_right + bracket_w;
        let b_sw = (font_size * 0.08).clamp(1.3, 3.0);
        let top_y = table_top + 1.0;
        let bot_y = (table_bottom - 1.0).max(top_y + 10.0);
        let mid_y = (top_y + bot_y) * 0.5;
        let lip = (bracket_w * 0.55).clamp(4.0, 9.0);

        match spec.kind {
            TableEnvKind::PMatrix => {
                // Kurung lengkung mulus '(' dan ')'
                let mut left_pts = Vec::with_capacity(13);
                let mut right_pts = Vec::with_capacity(13);
                for k in 0..=12 {
                    let t = k as f32 / 12.0;
                    let y = top_y + (bot_y - top_y) * t;
                    let curve = (t * std::f32::consts::PI).sin() * lip * 0.85;
                    left_pts.push(Pos2::new(bracket_w - 2.0 - curve, y));
                    right_pts.push(Pos2::new(table_right + 2.0 + curve, y));
                }
                items.push(MathLayoutItem::Polyline {
                    points: left_pts,
                    stroke_width: b_sw,
                    color: None,
                    dash: MathLineDash::Solid,
                    arrow_start: false,
                    arrow_end: false,
                });
                items.push(MathLayoutItem::Polyline {
                    points: right_pts,
                    stroke_width: b_sw,
                    color: None,
                    dash: MathLineDash::Solid,
                    arrow_start: false,
                    arrow_end: false,
                });
            }
            TableEnvKind::BMatrix => {
                // Kurung siku '[' dan ']'
                items.push(MathLayoutItem::Polyline {
                    points: vec![
                        Pos2::new(bracket_w - 2.0, top_y),
                        Pos2::new(bracket_w - 2.0 - lip, top_y),
                        Pos2::new(bracket_w - 2.0 - lip, bot_y),
                        Pos2::new(bracket_w - 2.0, bot_y),
                    ],
                    stroke_width: b_sw,
                    color: None,
                    dash: MathLineDash::Solid,
                    arrow_start: false,
                    arrow_end: false,
                });
                items.push(MathLayoutItem::Polyline {
                    points: vec![
                        Pos2::new(table_right + 2.0, top_y),
                        Pos2::new(table_right + 2.0 + lip, top_y),
                        Pos2::new(table_right + 2.0 + lip, bot_y),
                        Pos2::new(table_right + 2.0, bot_y),
                    ],
                    stroke_width: b_sw,
                    color: None,
                    dash: MathLineDash::Solid,
                    arrow_start: false,
                    arrow_end: false,
                });
            }
            TableEnvKind::BracesMatrix | TableEnvKind::Cases => {
                // Kurung kurawal kiri '{'
                let lx = bracket_w - 3.0;
                items.push(MathLayoutItem::Polyline {
                    points: vec![
                        Pos2::new(lx + lip * 0.45, top_y),
                        Pos2::new(lx, top_y + 4.0),
                        Pos2::new(lx, mid_y - 3.0),
                        Pos2::new(lx - lip * 0.55, mid_y),
                        Pos2::new(lx, mid_y + 3.0),
                        Pos2::new(lx, bot_y - 4.0),
                        Pos2::new(lx + lip * 0.45, bot_y),
                    ],
                    stroke_width: b_sw,
                    color: None,
                    dash: MathLineDash::Solid,
                    arrow_start: false,
                    arrow_end: false,
                });
                if spec.kind == TableEnvKind::BracesMatrix {
                    let rx = table_right + 3.0;
                    items.push(MathLayoutItem::Polyline {
                        points: vec![
                            Pos2::new(rx - lip * 0.45, top_y),
                            Pos2::new(rx, top_y + 4.0),
                            Pos2::new(rx, mid_y - 3.0),
                            Pos2::new(rx + lip * 0.55, mid_y),
                            Pos2::new(rx, mid_y + 3.0),
                            Pos2::new(rx, bot_y - 4.0),
                            Pos2::new(rx - lip * 0.45, bot_y),
                        ],
                        stroke_width: b_sw,
                        color: None,
                        dash: MathLineDash::Solid,
                        arrow_start: false,
                        arrow_end: false,
                    });
                }
            }
            TableEnvKind::VMatrix => {
                items.push(MathLayoutItem::Line {
                    start: Pos2::new(bracket_w * 0.5, top_y),
                    end: Pos2::new(bracket_w * 0.5, bot_y),
                    stroke_width: b_sw,
                });
                items.push(MathLayoutItem::Line {
                    start: Pos2::new(right_b_x - bracket_w * 0.5, top_y),
                    end: Pos2::new(right_b_x - bracket_w * 0.5, bot_y),
                    stroke_width: b_sw,
                });
            }
            TableEnvKind::DoubleVMatrix => {
                let lx = bracket_w * 0.5;
                let rx = right_b_x - bracket_w * 0.5;
                for ox in [-1.8_f32, 1.8] {
                    items.push(MathLayoutItem::Line {
                        start: Pos2::new(lx + ox, top_y),
                        end: Pos2::new(lx + ox, bot_y),
                        stroke_width: b_sw * 0.85,
                    });
                    items.push(MathLayoutItem::Line {
                        start: Pos2::new(rx + ox, top_y),
                        end: Pos2::new(rx + ox, bot_y),
                        stroke_width: b_sw * 0.85,
                    });
                }
            }
            _ => {}
        }
    }

    let total_w = (table_right
        + if has_brackets && spec.kind != TableEnvKind::Cases {
            bracket_w
        } else {
            2.0
        })
    .max(24.0);
    let total_h = (table_bottom + 2.0).max(font_size * 1.4);
    (items, total_w, total_h)
}
