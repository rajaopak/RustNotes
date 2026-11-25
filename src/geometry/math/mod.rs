pub mod expr;
pub mod table;
pub mod tikz;

use eframe::egui::{self, Color32, Painter, Pos2, Stroke};

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Menghapus komentar baris `% ...` (kecuali `\%`) dari kode LaTeX.
pub(crate) fn strip_latex_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        let mut escaped = false;
        let mut cut = line.len();
        for (idx, ch) in line.char_indices() {
            if ch == '\\' {
                escaped = !escaped;
                continue;
            }
            if ch == '%' && !escaped {
                cut = idx;
                break;
            }
            escaped = false;
        }
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

/// Mengambil isi grup kurung kurawal `{...}` mulai pada indeks `start`, mengembalikan `(isi, panjang_konsumsi)`.
pub(crate) fn extract_braced_group(s: &str, start: usize) -> Option<(String, usize)> {
    let rest = s.get(start..)?;
    if !rest.starts_with('{') {
        return None;
    }
    let mut depth = 0usize;
    let mut inner = String::new();
    for (idx, ch) in rest.char_indices() {
        match ch {
            '{' => {
                if depth > 0 {
                    inner.push(ch);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((inner, idx + ch.len_utf8()));
                }
                inner.push(ch);
            }
            _ => inner.push(ch),
        }
    }
    None
}

/// Mengambil isi grup kurung siku `[...]` mulai pada indeks `start`, mengembalikan `(isi, panjang_konsumsi)`.
pub(crate) fn extract_bracket_group(s: &str, start: usize) -> Option<(String, usize)> {
    let rest = s.get(start..)?;
    if !rest.starts_with('[') {
        return None;
    }
    let mut depth = 0usize;
    let mut inner = String::new();
    for (idx, ch) in rest.char_indices() {
        match ch {
            '[' => {
                if depth > 0 {
                    inner.push(ch);
                }
                depth += 1;
            }
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some((inner, idx + ch.len_utf8()));
                }
                inner.push(ch);
            }
            _ => inner.push(ch),
        }
    }
    None
}

/// Pola garis vektor untuk tabel, geometri, dan grafik LaTeX.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathLineDash {
    Solid,
    Dashed,
    Dotted,
}

/// Representasi Token dari ekspresi LaTeX Matematika
#[derive(Clone, Debug, PartialEq)]
pub enum MathToken {
    Command(String), // e.g. "frac", "sqrt", "alpha", "sum"
    Char(char),      // e.g. 'x', '+', '=', '1'
    Superscript,     // '^'
    Subscript,       // '_'
    GroupOpen,       // '{'
    GroupClose,      // '}'
    BracketOpen,     // '['
    BracketClose,    // ']'
    ParenOpen,       // '('
    ParenClose,      // ')'
    EnvBlock(String),
}

/// Node Pohon Sintaks Abstrak (AST) Formula Matematika, Tabel, Grafik & Geometri
#[derive(Clone, Debug, PartialEq)]
pub enum MathNode {
    Text(String),
    Symbol(char),
    Frac {
        num: Vec<MathNode>,
        den: Vec<MathNode>,
    },
    SubSup {
        base: Option<Box<MathNode>>,
        sub: Option<Vec<MathNode>>,
        sup: Option<Vec<MathNode>>,
    },
    Radical {
        index: Option<Vec<MathNode>>,
        radicand: Vec<MathNode>,
    },
    LargeOp {
        op: char,
        sub: Option<Vec<MathNode>>,
        sup: Option<Vec<MathNode>>,
    },
    Delimited {
        open: char,
        close: char,
        content: Vec<MathNode>,
    },
    Space(f32),
    Table(table::TableSpec),
    Diagram(tikz::DiagramSpec),
}

/// Kesalahan saat parsing LaTeX matematika, tabel, grafik, atau geometri
#[derive(Clone, Debug, PartialEq)]
pub struct MathParseError {
    pub message: String,
    pub position: usize,
}

impl std::fmt::Display for MathParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} pada posisi {}", self.message, self.position)
    }
}

impl std::error::Error for MathParseError {}

/// Tokenizer untuk ekspresi LaTeX matematika serta blok `\begin{...}...\end{...}` (tabel, grafik, geometri).
pub fn tokenize_latex(input: &str) -> Vec<MathToken> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '\\' => {
                i += 1;
                if i >= chars.len() {
                    break;
                }
                let next_c = chars[i];
                if next_c.is_alphabetic() {
                    let mut cmd = String::new();
                    while i < chars.len() && chars[i].is_alphabetic() {
                        cmd.push(chars[i]);
                        i += 1;
                    }
                    if cmd == "text" || cmd == "mathrm" || cmd == "mathbf" || cmd == "textbf" {
                        tokens.push(MathToken::Command(cmd));
                        while i < chars.len() && chars[i].is_whitespace() {
                            i += 1;
                        }
                        if i < chars.len() && chars[i] == '{' {
                            tokens.push(MathToken::GroupOpen);
                            i += 1;
                            let mut text_content = String::new();
                            let mut depth = 1;
                            while i < chars.len() && depth > 0 {
                                if chars[i] == '{' {
                                    depth += 1;
                                    text_content.push('{');
                                } else if chars[i] == '}' {
                                    depth -= 1;
                                    if depth > 0 {
                                        text_content.push('}');
                                    }
                                } else {
                                    text_content.push(chars[i]);
                                }
                                i += 1;
                            }
                            tokens.push(MathToken::Command(format!("__raw_text:{}", text_content)));
                            tokens.push(MathToken::GroupClose);
                        }
                        continue;
                    }
                    tokens.push(MathToken::Command(cmd));
                } else {
                    match next_c {
                        '{' => tokens.push(MathToken::Char('{')),
                        '}' => tokens.push(MathToken::Char('}')),
                        ',' => tokens.push(MathToken::Command("thinspace".into())),
                        ';' => tokens.push(MathToken::Command("thickspace".into())),
                        ' ' => tokens.push(MathToken::Command("space".into())),
                        _ => tokens.push(MathToken::Char(next_c)),
                    }
                    i += 1;
                }
            }
            '^' => {
                tokens.push(MathToken::Superscript);
                i += 1;
            }
            '_' => {
                tokens.push(MathToken::Subscript);
                i += 1;
            }
            '{' => {
                tokens.push(MathToken::GroupOpen);
                i += 1;
            }
            '}' => {
                tokens.push(MathToken::GroupClose);
                i += 1;
            }
            '[' => {
                tokens.push(MathToken::BracketOpen);
                i += 1;
            }
            ']' => {
                tokens.push(MathToken::BracketClose);
                i += 1;
            }
            '(' => {
                tokens.push(MathToken::ParenOpen);
                i += 1;
            }
            ')' => {
                tokens.push(MathToken::ParenClose);
                i += 1;
            }
            _ => {
                tokens.push(MathToken::Char(c));
                i += 1;
            }
        }
    }

    tokens
}

fn find_matching_end_env(input: &str, env_name: &str, body_start: usize) -> Option<(usize, usize)> {
    let begin_marker = format!(r"\begin{{{env_name}}}");
    let end_marker = format!(r"\end{{{env_name}}}");
    let mut depth = 1usize;
    let mut pos = body_start;
    while pos < input.len() {
        let rem = &input[pos..];
        if rem.starts_with(&begin_marker) {
            depth += 1;
            pos += begin_marker.len();
        } else if rem.starts_with(&end_marker) {
            depth -= 1;
            if depth == 0 {
                return Some((pos, pos + end_marker.len()));
            }
            pos += end_marker.len();
        } else {
            let ch = rem.chars().next()?;
            pos += ch.len_utf8();
        }
    }
    None
}

fn is_allowed_preamble_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "documentclass"
            | "usepackage"
            | "RequirePackage"
            | "pgfplotsset"
            | "usetikzlibrary"
            | "usepgfplotslibrary"
            | "tikzset"
            | "definecolor"
            | "title"
            | "author"
            | "date"
            | "pagestyle"
            | "thispagestyle"
            | "setlength"
            | "newcommand"
            | "renewcommand"
            | "geometry"
            | "graphicspath"
    )
}

fn consume_preamble_directive(s: &str, cmd_backslash_pos: usize) -> Option<usize> {
    let after_bs = s.get(cmd_backslash_pos + 1..)?;
    let cmd_len: usize = after_bs
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .map(char::len_utf8)
        .sum();
    if cmd_len == 0 {
        return None;
    }
    let cmd = &after_bs[..cmd_len];
    if !is_allowed_preamble_command(cmd) {
        return None;
    }
    let mut pos = cmd_backslash_pos + 1 + cmd_len;
    if s[pos..].starts_with('*') {
        pos += 1;
    }
    loop {
        while pos < s.len() && s[pos..].chars().next().is_some_and(char::is_whitespace) {
            pos += s[pos..].chars().next().unwrap().len_utf8();
        }
        if pos >= s.len() {
            break;
        }
        if s[pos..].starts_with('[') {
            if let Some((_, adv)) = extract_bracket_group(s, pos) {
                pos += adv;
                continue;
            }
            break;
        }
        if s[pos..].starts_with('{') {
            if let Some((_, adv)) = extract_braced_group(s, pos) {
                pos += adv;
                continue;
            }
            break;
        }
        break;
    }
    Some(pos)
}

fn validate_preamble(preamble: &str) -> Result<(), MathParseError> {
    let mut i = 0usize;
    while i < preamble.len() {
        let ch = preamble[i..].chars().next().unwrap();
        if ch.is_whitespace() {
            i += ch.len_utf8();
            continue;
        }
        if ch == '\\' {
            if let Some(next_i) = consume_preamble_directive(preamble, i) {
                i = next_i;
                continue;
            }
            let cmd: String = preamble[i + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
            return Err(MathParseError {
                message: format!("Perintah LaTeX tidak didukung: '\\{cmd}'"),
                position: i,
            });
        }
        return Err(MathParseError {
            message: "Teks tidak valid pada preambul sebelum \\begin{document}".into(),
            position: i,
        });
    }
    Ok(())
}

fn unwrap_trivial_containers(input: &str) -> Result<String, MathParseError> {
    let mut s = strip_latex_comments(input).trim().to_string();

    if let Some(doc_start) = s.find(r"\begin{document}") {
        validate_preamble(s[..doc_start].trim())?;
        let mut body_start = doc_start + r"\begin{document}".len();
        while body_start < s.len()
            && s[body_start..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        {
            body_start += s[body_start..].chars().next().unwrap().len_utf8();
        }
        let (end_start, end_after) =
            find_matching_end_env(&s, "document", body_start).ok_or_else(|| MathParseError {
                message: r"\begin{document} tidak memiliki \end{document}".into(),
                position: doc_start,
            })?;
        let trailing = s[end_after..].trim();
        if !trailing.is_empty() {
            return Err(MathParseError {
                message: "Konten tidak valid setelah \\end{document}".into(),
                position: end_after,
            });
        }
        s = s[body_start..end_start].trim().to_string();
    }

    // Kupas direktif konfigurasi/preambul opsional yang berada sebelum konten utama (baik di luar maupun awal document)
    let mut cursor = 0usize;
    while cursor < s.len() {
        let trimmed_off = s.len() - s[cursor..].trim_start().len();
        cursor = trimmed_off;
        if s[cursor..].starts_with('\\')
            && let Some(next_c) = consume_preamble_directive(&s, cursor)
        {
            cursor = next_c;
        } else {
            break;
        }
    }
    if cursor > 0 {
        s = s[cursor..].trim().to_string();
    }

    loop {
        let mut unwrapped = false;
        for env in ["center", "table", "figure"] {
            let prefix = format!(r"\begin{{{env}}}");
            if s.starts_with(&prefix) {
                let mut body_start = prefix.len();
                if s[body_start..].trim_start().starts_with('[') {
                    let trim_l = s.len() - s[body_start..].trim_start().len();
                    if let Some((_, adv)) = extract_bracket_group(&s, trim_l) {
                        body_start = trim_l + adv;
                    }
                }
                if let Some((end_start, end_after)) = find_matching_end_env(&s, env, body_start)
                    && s[end_after..].trim().is_empty()
                {
                    s = s[body_start..end_start].trim().to_string();
                    unwrapped = true;
                    break;
                }
            }
        }
        if !unwrapped {
            break;
        }
    }
    s = s.replace(r"\centering", "").replace(r"\maketitle", "");
    Ok(s.trim().to_string())
}

fn parse_env_block(env_name: &str, rest: &str) -> Result<(MathNode, usize), MathParseError> {
    let mut cursor = 0usize;
    while cursor < rest.len() && rest[cursor..].chars().next().unwrap().is_whitespace() {
        cursor += 1;
    }

    // Parameter wajib atau opsional sesuai jenis lingkungan
    let (col_spec_opt, bracket_opts_opt) = if matches!(env_name, "tabular" | "tabular*" | "array") {
        let mut opt_col = None;
        if env_name == "tabular*"
            && rest[cursor..].starts_with('{')
            && let Some((_, adv)) = extract_braced_group(rest, cursor)
        {
            cursor += adv;
            while cursor < rest.len() && rest[cursor..].chars().next().unwrap().is_whitespace() {
                cursor += 1;
            }
        }
        if rest[cursor..].starts_with('[')
            && let Some((_, adv)) = extract_bracket_group(rest, cursor)
        {
            cursor += adv;
            while cursor < rest.len() && rest[cursor..].chars().next().unwrap().is_whitespace() {
                cursor += 1;
            }
        }
        if rest[cursor..].starts_with('{')
            && let Some((cs, adv)) = extract_braced_group(rest, cursor)
        {
            cursor += adv;
            opt_col = Some(cs);
        }
        (opt_col, None)
    } else {
        let mut b_opt = None;
        if rest[cursor..].starts_with('[')
            && let Some((o, adv)) = extract_bracket_group(rest, cursor)
        {
            cursor += adv;
            b_opt = Some(o);
        }
        (None, b_opt)
    };

    let (body_end, total_end) =
        find_matching_end_env(rest, env_name, cursor).ok_or_else(|| MathParseError {
            message: format!("\\begin{{{env_name}}} tidak memiliki \\end{{{env_name}}}"),
            position: cursor,
        })?;
    let body = &rest[cursor..body_end];

    let node = if table::TableEnvKind::from_env_name(env_name).is_some() {
        MathNode::Table(table::parse_table_env(
            env_name,
            col_spec_opt.as_deref(),
            body,
        )?)
    } else if env_name == "tikzpicture" {
        MathNode::Diagram(tikz::DiagramSpec::Tikz(tikz::parse_tikzpicture_env(
            bracket_opts_opt.as_deref(),
            body,
        )?))
    } else if env_name == "axis" {
        MathNode::Diagram(tikz::DiagramSpec::Axis(Box::new(tikz::parse_axis_env(
            bracket_opts_opt.as_deref(),
            body,
        )?)))
    } else {
        return Err(MathParseError {
            message: format!("Lingkungan LaTeX tidak didukung: '\\begin{{{env_name}}}'"),
            position: 0,
        });
    };

    Ok((node, total_end))
}

/// Parser untuk mengubah input LaTeX (rumus matematika, tabel/matriks, grafik, maupun geometri) menjadi AST `MathNode`.
pub fn parse_latex_math(input: &str) -> Result<Vec<MathNode>, MathParseError> {
    let cleaned = unwrap_trivial_containers(input)?;
    if !cleaned.contains(r"\begin{") {
        return parse_inline_math_segment(&cleaned);
    }

    let mut nodes = Vec::new();
    let mut i = 0usize;
    while i < cleaned.len() {
        if let Some(rel_idx) = cleaned[i..].find(r"\begin{") {
            let prefix = cleaned[i..i + rel_idx].trim();
            if !prefix.is_empty() {
                nodes.extend(parse_inline_math_segment(prefix)?);
            }
            let brace_start = i + rel_idx + r"\begin".len();
            let (env_name, adv) =
                extract_braced_group(&cleaned, brace_start).ok_or_else(|| MathParseError {
                    message: "Sintaks \\begin{...} tidak lengkap".into(),
                    position: brace_start,
                })?;
            let after_env_header = brace_start + adv;
            let (env_node, consumed) =
                parse_env_block(env_name.trim(), &cleaned[after_env_header..])?;
            nodes.push(env_node);
            i = after_env_header + consumed;
        } else {
            let tail = cleaned[i..].trim();
            if !tail.is_empty() {
                nodes.extend(parse_inline_math_segment(tail)?);
            }
            break;
        }
    }

    Ok(nodes)
}

fn parse_inline_math_segment(input: &str) -> Result<Vec<MathNode>, MathParseError> {
    let clean = input.trim().trim_matches('$');
    let tokens = tokenize_latex(clean);
    let mut cursor = 0;
    let mut nodes = Vec::new();

    while cursor < tokens.len() {
        let node = parse_single_item(&tokens, &mut cursor)?;
        nodes.push(node);
    }

    // Gabungkan subscript dan superscript yang menempel pada node sebelumnya
    let mut consolidated = Vec::new();
    let mut i = 0;
    while i < nodes.len() {
        let cur = nodes[i].clone();
        if i + 1 < nodes.len()
            && let MathNode::SubSup {
                base: None,
                sub,
                sup,
            } = &nodes[i + 1]
        {
            let combined = MathNode::SubSup {
                base: Some(Box::new(cur)),
                sub: sub.clone(),
                sup: sup.clone(),
            };
            consolidated.push(combined);
            i += 2;
            continue;
        }
        consolidated.push(cur);
        i += 1;
    }

    Ok(consolidated)
}

fn parse_single_item(tokens: &[MathToken], cursor: &mut usize) -> Result<MathNode, MathParseError> {
    if *cursor >= tokens.len() {
        return Err(MathParseError {
            message: "Akhir ekspresi tidak terduga".into(),
            position: *cursor,
        });
    }

    match &tokens[*cursor] {
        MathToken::Command(cmd) => {
            let cmd_name = cmd.as_str();
            *cursor += 1;
            match cmd_name {
                "frac" | "dfrac" => {
                    let num = parse_group(tokens, cursor)?;
                    let den = parse_group(tokens, cursor)?;
                    Ok(MathNode::Frac { num, den })
                }
                "sqrt" => {
                    let index =
                        if *cursor < tokens.len() && tokens[*cursor] == MathToken::BracketOpen {
                            *cursor += 1;
                            let idx_nodes = parse_until_bracket_close(tokens, cursor)?;
                            Some(idx_nodes)
                        } else {
                            None
                        };
                    let radicand = parse_group(tokens, cursor)?;
                    Ok(MathNode::Radical { index, radicand })
                }
                "sum" => Ok(MathNode::LargeOp {
                    op: '∑',
                    sub: None,
                    sup: None,
                }),
                "prod" => Ok(MathNode::LargeOp {
                    op: '∏',
                    sub: None,
                    sup: None,
                }),
                "int" => Ok(MathNode::LargeOp {
                    op: '∫',
                    sub: None,
                    sup: None,
                }),
                "iint" => Ok(MathNode::LargeOp {
                    op: '∬',
                    sub: None,
                    sup: None,
                }),
                "oint" => Ok(MathNode::LargeOp {
                    op: '∮',
                    sub: None,
                    sup: None,
                }),
                "left" => {
                    if *cursor >= tokens.len() {
                        return Err(MathParseError {
                            message: "\\left tanpa kurung penutup".into(),
                            position: *cursor,
                        });
                    }
                    let open_char = match &tokens[*cursor] {
                        MathToken::ParenOpen => '(',
                        MathToken::BracketOpen => '[',
                        MathToken::Char(c) => *c,
                        _ => '(',
                    };
                    *cursor += 1;
                    let content = parse_until_right(tokens, cursor)?;
                    let close_char = if *cursor < tokens.len() {
                        let c = match &tokens[*cursor] {
                            MathToken::ParenClose => ')',
                            MathToken::BracketClose => ']',
                            MathToken::Char(c) => *c,
                            _ => ')',
                        };
                        *cursor += 1;
                        c
                    } else {
                        ')'
                    };
                    Ok(MathNode::Delimited {
                        open: open_char,
                        close: close_char,
                        content,
                    })
                }
                "right" => Err(MathParseError {
                    message: "\\right tanpa pasangan \\left".into(),
                    position: *cursor,
                }),
                "text" | "mathrm" | "mathbf" | "textbf" => {
                    let inner = parse_group(tokens, cursor)?;
                    let mut s = String::new();
                    for n in inner {
                        match n {
                            MathNode::Text(t) => {
                                if let Some(raw) = t.strip_prefix("__raw_text:") {
                                    s.push_str(raw);
                                } else {
                                    s.push_str(&t);
                                }
                            }
                            MathNode::Symbol(c) => s.push(c),
                            _ => {}
                        }
                    }
                    Ok(MathNode::Text(s))
                }
                "quad" => Ok(MathNode::Space(1.0)),
                "qquad" => Ok(MathNode::Space(2.0)),
                "thinspace" => Ok(MathNode::Space(0.3)),
                "thickspace" => Ok(MathNode::Space(0.6)),
                "space" => Ok(MathNode::Space(0.4)),
                cmd if cmd.starts_with("__raw_text:") => Ok(MathNode::Text(cmd.to_string())),
                _ => {
                    if let Some(ch) = latex_command_to_char(cmd_name) {
                        Ok(MathNode::Symbol(ch))
                    } else if is_standard_function_name(cmd_name) {
                        Ok(MathNode::Text(cmd_name.to_string()))
                    } else {
                        Err(MathParseError {
                            message: format!("Perintah LaTeX tidak didukung: '\\{}'", cmd_name),
                            position: *cursor,
                        })
                    }
                }
            }
        }
        MathToken::Superscript => {
            *cursor += 1;
            let sup = parse_group(tokens, cursor)?;
            Ok(MathNode::SubSup {
                base: None,
                sub: None,
                sup: Some(sup),
            })
        }
        MathToken::Subscript => {
            *cursor += 1;
            let sub = parse_group(tokens, cursor)?;
            let sup = if *cursor < tokens.len() && tokens[*cursor] == MathToken::Superscript {
                *cursor += 1;
                Some(parse_group(tokens, cursor)?)
            } else {
                None
            };
            Ok(MathNode::SubSup {
                base: None,
                sub: Some(sub),
                sup,
            })
        }
        MathToken::GroupOpen => {
            *cursor += 1;
            let nodes = parse_until_group_close(tokens, cursor)?;
            Ok(MathNode::Delimited {
                open: '\0',
                close: '\0',
                content: nodes,
            })
        }
        MathToken::GroupClose => Err(MathParseError {
            message: "Tutup kurung kurawal '}' tanpa pasangan".into(),
            position: *cursor,
        }),
        MathToken::ParenOpen => {
            *cursor += 1;
            let content = parse_until_paren_close(tokens, cursor)?;
            Ok(MathNode::Delimited {
                open: '(',
                close: ')',
                content,
            })
        }
        MathToken::ParenClose => {
            *cursor += 1;
            Ok(MathNode::Symbol(')'))
        }
        MathToken::BracketOpen => {
            *cursor += 1;
            let content = parse_until_bracket_close(tokens, cursor)?;
            Ok(MathNode::Delimited {
                open: '[',
                close: ']',
                content,
            })
        }
        MathToken::BracketClose => {
            *cursor += 1;
            Ok(MathNode::Symbol(']'))
        }
        MathToken::Char(c) => {
            let ch = *c;
            *cursor += 1;
            Ok(MathNode::Symbol(ch))
        }
        MathToken::EnvBlock(_) => {
            *cursor += 1;
            Ok(MathNode::Space(0.0))
        }
    }
}

fn parse_group(tokens: &[MathToken], cursor: &mut usize) -> Result<Vec<MathNode>, MathParseError> {
    if *cursor >= tokens.len() {
        return Err(MathParseError {
            message: "Diperlukan argumen atau grup '{...}'".into(),
            position: *cursor,
        });
    }

    if tokens[*cursor] == MathToken::GroupOpen {
        *cursor += 1;
        parse_until_group_close(tokens, cursor)
    } else {
        let single = parse_single_item(tokens, cursor)?;
        Ok(vec![single])
    }
}

fn parse_until_group_close(
    tokens: &[MathToken],
    cursor: &mut usize,
) -> Result<Vec<MathNode>, MathParseError> {
    let mut nodes = Vec::new();
    while *cursor < tokens.len() {
        if tokens[*cursor] == MathToken::GroupClose {
            *cursor += 1;
            return Ok(nodes);
        }
        let node = parse_single_item(tokens, cursor)?;
        nodes.push(node);
    }
    Err(MathParseError {
        message: "Kurung kurawal '{' tidak memiliki pasangan penutup '}'".into(),
        position: *cursor,
    })
}

fn parse_until_paren_close(
    tokens: &[MathToken],
    cursor: &mut usize,
) -> Result<Vec<MathNode>, MathParseError> {
    let mut nodes = Vec::new();
    while *cursor < tokens.len() {
        if tokens[*cursor] == MathToken::ParenClose {
            *cursor += 1;
            return Ok(nodes);
        }
        let node = parse_single_item(tokens, cursor)?;
        nodes.push(node);
    }
    Ok(nodes)
}

fn parse_until_bracket_close(
    tokens: &[MathToken],
    cursor: &mut usize,
) -> Result<Vec<MathNode>, MathParseError> {
    let mut nodes = Vec::new();
    while *cursor < tokens.len() {
        if tokens[*cursor] == MathToken::BracketClose {
            *cursor += 1;
            return Ok(nodes);
        }
        let node = parse_single_item(tokens, cursor)?;
        nodes.push(node);
    }
    Ok(nodes)
}

fn parse_until_right(
    tokens: &[MathToken],
    cursor: &mut usize,
) -> Result<Vec<MathNode>, MathParseError> {
    let mut nodes = Vec::new();
    while *cursor < tokens.len() {
        if let MathToken::Command(cmd) = &tokens[*cursor]
            && cmd == "right"
        {
            *cursor += 1;
            return Ok(nodes);
        }
        let node = parse_single_item(tokens, cursor)?;
        nodes.push(node);
    }
    Err(MathParseError {
        message: "\\left tidak memiliki pasangan \\right".into(),
        position: *cursor,
    })
}

/// Pemetaan perintah LaTeX ke karakter Unicode matematis
pub fn latex_command_to_char(cmd: &str) -> Option<char> {
    match cmd {
        // Huruf Yunani kecil
        "alpha" => Some('α'),
        "beta" => Some('β'),
        "gamma" => Some('γ'),
        "delta" => Some('δ'),
        "epsilon" | "varepsilon" => Some('ε'),
        "zeta" => Some('ζ'),
        "eta" => Some('η'),
        "theta" | "vartheta" => Some('θ'),
        "iota" => Some('ι'),
        "kappa" => Some('κ'),
        "lambda" => Some('λ'),
        "mu" => Some('μ'),
        "nu" => Some('ν'),
        "xi" => Some('ξ'),
        "pi" | "varpi" => Some('π'),
        "rho" | "varrho" => Some('ρ'),
        "sigma" | "varsigma" => Some('σ'),
        "tau" => Some('τ'),
        "upsilon" => Some('υ'),
        "phi" | "varphi" => Some('φ'),
        "chi" => Some('χ'),
        "psi" => Some('ψ'),
        "omega" => Some('ω'),

        // Huruf Yunani besar
        "Gamma" => Some('Γ'),
        "Delta" => Some('Δ'),
        "Theta" => Some('Θ'),
        "Lambda" => Some('Λ'),
        "Xi" => Some('Ξ'),
        "Pi" => Some('Π'),
        "Sigma" => Some('Σ'),
        "Upsilon" => Some('Υ'),
        "Phi" => Some('Φ'),
        "Psi" => Some('Ψ'),
        "Omega" => Some('Ω'),

        // Operator & Simbol Matematika
        "pm" => Some('±'),
        "mp" => Some('∓'),
        "times" => Some('×'),
        "div" => Some('÷'),
        "cdot" => Some('·'),
        "circ" => Some('∘'),
        "bullet" => Some('•'),
        "star" => Some('⋆'),
        "leq" | "le" => Some('≤'),
        "geq" | "ge" => Some('≥'),
        "neq" | "ne" => Some('≠'),
        "approx" => Some('≈'),
        "equiv" => Some('≡'),
        "sim" => Some('∼'),
        "propto" => Some('∝'),
        "infty" => Some('∞'),
        "partial" => Some('∂'),
        "nabla" => Some('∇'),
        "hbar" => Some('ℏ'),
        "ell" => Some('ℓ'),
        "in" => Some('∈'),
        "notin" => Some('∉'),
        "subset" => Some('⊂'),
        "subseteq" => Some('⊆'),
        "supset" => Some('⊃'),
        "supseteq" => Some('⊇'),
        "cup" => Some('∪'),
        "cap" => Some('∩'),
        "emptyset" => Some('∅'),
        "to" | "rightarrow" => Some('→'),
        "leftarrow" => Some('←'),
        "leftrightarrow" => Some('↔'),
        "Rightarrow" | "implies" => Some('⇒'),
        "Leftarrow" => Some('⇐'),
        "Leftrightarrow" | "iff" => Some('⇔'),
        "mapsto" => Some('↦'),
        "forall" => Some('∀'),
        "exists" => Some('∃'),
        "nexists" => Some('∄'),
        "dots" | "cdots" | "ldots" => Some('…'),
        "langle" => Some('⟨'),
        "rangle" => Some('⟩'),
        "mid" => Some('∣'),
        "parallel" => Some('∥'),
        "perp" => Some('⊥'),
        "angle" => Some('∠'),
        "therefore" => Some('∴'),
        "because" => Some('∵'),
        "vee" | "lor" => Some('∨'),
        "wedge" | "land" => Some('∧'),
        "neg" | "lnot" => Some('¬'),
        "top" => Some('⊤'),
        "bot" => Some('⊥'),
        "aleph" => Some('ℵ'),
        "cong" => Some('≅'),
        "prime" => Some('′'),
        "dag" | "dagger" => Some('†'),
        "ddag" | "ddagger" => Some('‡'),
        "degree" => Some('°'),
        _ => None,
    }
}

/// Menentukan apakah perintah merupakan nama fungsi matematika standar (dicetak tegak/roman)
#[inline]
pub fn is_standard_function_name(cmd: &str) -> bool {
    matches!(
        cmd,
        "sin"
            | "cos"
            | "tan"
            | "sec"
            | "csc"
            | "cot"
            | "arcsin"
            | "arccos"
            | "arctan"
            | "sinh"
            | "cosh"
            | "tanh"
            | "coth"
            | "sech"
            | "csch"
            | "ln"
            | "log"
            | "exp"
            | "lg"
            | "det"
            | "dim"
            | "gcd"
            | "deg"
            | "hom"
            | "ker"
            | "arg"
            | "max"
            | "min"
            | "sup"
            | "inf"
            | "lim"
    )
}

/// Item elemen visual hasil perhitungan tata letak 2D formula, tabel, grafik, atau geometri LaTeX
#[derive(Clone, Debug, PartialEq)]
pub enum MathLayoutItem {
    Glyph {
        text: String,
        font_size: f32,
        is_italic: bool,
        pos: Pos2,
    },
    ColoredGlyph {
        text: String,
        font_size: f32,
        is_italic: bool,
        is_bold: bool,
        pos: Pos2,
        color: Option<Color32>,
    },
    Line {
        start: Pos2,
        end: Pos2,
        stroke_width: f32,
    },
    StyledLine {
        start: Pos2,
        end: Pos2,
        stroke_width: f32,
        color: Option<Color32>,
        dash: MathLineDash,
        arrow_start: bool,
        arrow_end: bool,
    },
    Polyline {
        points: Vec<Pos2>,
        stroke_width: f32,
        color: Option<Color32>,
        dash: MathLineDash,
        arrow_start: bool,
        arrow_end: bool,
    },
    Polygon {
        points: Vec<Pos2>,
        fill: Option<Color32>,
        stroke_color: Option<Color32>,
        stroke_width: f32,
    },
    Ellipse {
        center: Pos2,
        rx: f32,
        ry: f32,
        fill: Option<Color32>,
        stroke_color: Option<Color32>,
        stroke_width: f32,
    },
}

/// Tata letak 2D lengkap untuk formula matematika, tabel, grafik, dan geometri LaTeX
#[derive(Clone, Debug, PartialEq)]
pub struct MathLayout {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
    pub items: Vec<MathLayoutItem>,
}

fn compute_items_vertical_bounds(items: &[MathLayoutItem]) -> Option<(f32, f32)> {
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for it in items {
        match it {
            MathLayoutItem::Glyph { font_size, pos, .. }
            | MathLayoutItem::ColoredGlyph { font_size, pos, .. } => {
                min_y = min_y.min(pos.y);
                max_y = max_y.max(pos.y + font_size * 1.25);
            }
            MathLayoutItem::Line { start, end, .. }
            | MathLayoutItem::StyledLine { start, end, .. } => {
                min_y = min_y.min(start.y).min(end.y);
                max_y = max_y.max(start.y).max(end.y);
            }
            MathLayoutItem::Polyline { points, .. } | MathLayoutItem::Polygon { points, .. } => {
                for p in points {
                    min_y = min_y.min(p.y);
                    max_y = max_y.max(p.y);
                }
            }
            MathLayoutItem::Ellipse { center, ry, .. } => {
                min_y = min_y.min(center.y - ry);
                max_y = max_y.max(center.y + ry);
            }
        }
    }
    (min_y.is_finite() && max_y.is_finite()).then_some((min_y, max_y))
}

impl MathLayout {
    /// Menghitung tata letak 2D langsung dari string LaTeX
    pub fn compute_from_str(latex: &str, font_size: f32) -> Self {
        let nodes = parse_latex_math(latex).unwrap_or_default();
        Self::compute(&nodes, font_size)
    }

    /// Menghitung tata letak 2D pada ukuran font dasar lalu menskalakan seluruh geometri secara linier eksak.
    pub fn compute_scaled(latex: &str, font_size: f32, scale: f32) -> Self {
        let mut layout = Self::compute_from_str(latex, font_size);
        layout.scale_uniform(scale);
        layout
    }

    /// Menskalakan seluruh dimensi dan primitif item tata letak secara proporsional seragam terhadap `(0, 0)`.
    pub fn scale_uniform(&mut self, scale: f32) {
        let s = if scale.is_finite() && scale > 1e-4 {
            scale
        } else {
            1.0
        };
        if (s - 1.0).abs() < 1e-6 {
            return;
        }
        self.width *= s;
        self.height *= s;
        self.ascent *= s;
        self.descent *= s;
        for item in &mut self.items {
            match item {
                MathLayoutItem::Glyph { font_size, pos, .. }
                | MathLayoutItem::ColoredGlyph { font_size, pos, .. } => {
                    *font_size *= s;
                    pos.x *= s;
                    pos.y *= s;
                }
                MathLayoutItem::Line {
                    start,
                    end,
                    stroke_width,
                }
                | MathLayoutItem::StyledLine {
                    start,
                    end,
                    stroke_width,
                    ..
                } => {
                    start.x *= s;
                    start.y *= s;
                    end.x *= s;
                    end.y *= s;
                    *stroke_width *= s;
                }
                MathLayoutItem::Polyline {
                    points,
                    stroke_width,
                    ..
                }
                | MathLayoutItem::Polygon {
                    points,
                    stroke_width,
                    ..
                } => {
                    for p in points {
                        p.x *= s;
                        p.y *= s;
                    }
                    *stroke_width *= s;
                }
                MathLayoutItem::Ellipse {
                    center,
                    rx,
                    ry,
                    stroke_width,
                    ..
                } => {
                    center.x *= s;
                    center.y *= s;
                    *rx *= s;
                    *ry *= s;
                    *stroke_width *= s;
                }
            }
        }
    }

    /// Menghitung tata letak 2D langsung dari string LaTeX dengan validasi syntax penuh
    pub fn compute_from_str_checked(latex: &str, font_size: f32) -> Result<Self, MathParseError> {
        let nodes = parse_latex_math(latex)?;
        Ok(Self::compute(&nodes, font_size))
    }

    /// Menghitung tata letak 2D dari daftar MathNode pada ukuran font tertentu
    pub fn compute(nodes: &[MathNode], font_size: f32) -> Self {
        let mut items = Vec::new();
        let mut cur_x = 0.0_f32;
        let base_ascent = font_size * 0.8;
        let base_descent = font_size * 0.25;
        let mut has_block_node = false;
        let mut block_max_h = 0.0_f32;

        for node in nodes {
            match node {
                MathNode::Text(t) => {
                    let w = t.chars().count() as f32 * font_size * 0.58;
                    items.push(MathLayoutItem::Glyph {
                        text: t.clone(),
                        font_size,
                        is_italic: false,
                        pos: Pos2::new(cur_x, 0.0),
                    });
                    cur_x += w;
                }
                MathNode::Symbol(c) => {
                    let s = c.to_string();
                    let is_alpha = c.is_ascii_alphabetic();
                    let w = if is_alpha {
                        font_size * 0.55
                    } else if "+-=<>±≤≥≠≈→←".contains(*c) {
                        font_size * 0.85
                    } else {
                        font_size * 0.65
                    };
                    items.push(MathLayoutItem::Glyph {
                        text: s,
                        font_size,
                        is_italic: is_alpha,
                        pos: Pos2::new(cur_x, 0.0),
                    });
                    cur_x += w;
                }
                MathNode::Frac { num, den } => {
                    let sub_font = font_size * 0.85;
                    let num_layout = MathLayout::compute(num, sub_font);
                    let den_layout = MathLayout::compute(den, sub_font);

                    let frac_w = num_layout.width.max(den_layout.width) + 8.0;
                    let bar_y = font_size * 0.35;
                    let bar_thickness = (font_size * 0.07).clamp(1.2, 3.5);

                    let num_x = cur_x + (frac_w - num_layout.width) * 0.5;
                    let num_y = bar_y - num_layout.height - 2.0;
                    for mut it in num_layout.items {
                        table::shift_item(&mut it, num_x, num_y);
                        items.push(it);
                    }

                    items.push(MathLayoutItem::Line {
                        start: Pos2::new(cur_x + 1.0, bar_y),
                        end: Pos2::new(cur_x + frac_w - 1.0, bar_y),
                        stroke_width: bar_thickness,
                    });

                    let den_x = cur_x + (frac_w - den_layout.width) * 0.5;
                    let den_y = bar_y + 4.0;
                    for mut it in den_layout.items {
                        table::shift_item(&mut it, den_x, den_y);
                        items.push(it);
                    }

                    cur_x += frac_w + 3.0;
                }
                MathNode::SubSup { base, sub, sup } => {
                    let base_w = if let Some(b) = base {
                        let b_layout = MathLayout::compute(&[*b.clone()], font_size);
                        let w = b_layout.width;
                        for mut it in b_layout.items {
                            table::shift_item(&mut it, cur_x, 0.0);
                            items.push(it);
                        }
                        w
                    } else {
                        0.0
                    };

                    let script_size = font_size * 0.7;
                    let mut script_w = 0.0_f32;

                    if let Some(sp) = sup {
                        let sup_layout = MathLayout::compute(sp, script_size);
                        script_w = script_w.max(sup_layout.width);
                        let sup_x = cur_x + base_w + 1.0;
                        let sup_y = -font_size * 0.45;
                        for mut it in sup_layout.items {
                            table::shift_item(&mut it, sup_x, sup_y);
                            items.push(it);
                        }
                    }

                    if let Some(sb) = sub {
                        let sub_layout = MathLayout::compute(sb, script_size);
                        script_w = script_w.max(sub_layout.width);
                        let sub_x = cur_x + base_w + 1.0;
                        let sub_y = font_size * 0.45;
                        for mut it in sub_layout.items {
                            table::shift_item(&mut it, sub_x, sub_y);
                            items.push(it);
                        }
                    }

                    cur_x += base_w + script_w + 2.0;
                }
                MathNode::Radical { index: _, radicand } => {
                    let rad_layout = MathLayout::compute(radicand, font_size);
                    let bar_thickness = (font_size * 0.07).clamp(1.2, 3.0);
                    let rad_w = rad_layout.width + 12.0;

                    let sym_x = cur_x;
                    items.push(MathLayoutItem::Glyph {
                        text: "√".into(),
                        font_size: font_size * 1.15,
                        is_italic: false,
                        pos: Pos2::new(sym_x, -1.0),
                    });

                    let line_y = -font_size * 0.15;
                    items.push(MathLayoutItem::Line {
                        start: Pos2::new(sym_x + font_size * 0.55, line_y),
                        end: Pos2::new(sym_x + rad_w, line_y),
                        stroke_width: bar_thickness,
                    });

                    let cont_x = sym_x + font_size * 0.55 + 2.0;
                    for mut it in rad_layout.items {
                        table::shift_item(&mut it, cont_x, 0.0);
                        items.push(it);
                    }

                    cur_x += rad_w + 3.0;
                }
                MathNode::LargeOp { op, sub: _, sup: _ } => {
                    let op_size = font_size * 1.35;
                    let op_w = op_size * 0.75;
                    items.push(MathLayoutItem::Glyph {
                        text: op.to_string(),
                        font_size: op_size,
                        is_italic: false,
                        pos: Pos2::new(cur_x, -font_size * 0.1),
                    });
                    cur_x += op_w + 2.0;
                }
                MathNode::Delimited {
                    open,
                    close,
                    content,
                } => {
                    let inner_layout = MathLayout::compute(content, font_size);
                    if *open != '\0' {
                        items.push(MathLayoutItem::Glyph {
                            text: open.to_string(),
                            font_size,
                            is_italic: false,
                            pos: Pos2::new(cur_x, 0.0),
                        });
                        cur_x += font_size * 0.45;
                    }

                    let off_x = cur_x;
                    for mut it in inner_layout.items {
                        table::shift_item(&mut it, off_x, 0.0);
                        items.push(it);
                    }
                    cur_x += inner_layout.width;

                    if *close != '\0' {
                        items.push(MathLayoutItem::Glyph {
                            text: close.to_string(),
                            font_size,
                            is_italic: false,
                            pos: Pos2::new(cur_x, 0.0),
                        });
                        cur_x += font_size * 0.45;
                    }
                }
                MathNode::Space(factor) => {
                    cur_x += font_size * 0.5 * factor;
                }
                MathNode::Table(spec) => {
                    has_block_node = true;
                    let (t_items, tw, th) = table::layout_table(spec, font_size);
                    let off_y = if items.is_empty() {
                        0.0
                    } else {
                        -(th - font_size) * 0.45
                    };
                    for mut it in t_items {
                        table::shift_item(&mut it, cur_x, off_y);
                        items.push(it);
                    }
                    cur_x += tw + 4.0;
                    block_max_h = block_max_h.max(th);
                }
                MathNode::Diagram(spec) => {
                    has_block_node = true;
                    let (d_items, dw, dh) = tikz::layout_diagram(spec, font_size);
                    for mut it in d_items {
                        table::shift_item(&mut it, cur_x, 0.0);
                        items.push(it);
                    }
                    cur_x += dw + 4.0;
                    block_max_h = block_max_h.max(dh);
                }
            }
        }

        if has_block_node && let Some((min_y, max_y)) = compute_items_vertical_bounds(&items) {
            if min_y < 0.0 {
                let shift_y = -min_y + 2.0;
                for it in &mut items {
                    table::shift_item(it, 0.0, shift_y);
                }
                block_max_h = block_max_h.max(max_y - min_y + 4.0);
            } else {
                block_max_h = block_max_h.max(max_y + 2.0);
            }
        }

        let total_w = cur_x.max(font_size * 0.5);
        let total_h = if has_block_node {
            block_max_h.max(font_size * 1.3)
        } else {
            font_size * 1.3
        };

        Self {
            width: total_w,
            height: total_h,
            ascent: base_ascent,
            descent: base_descent,
            items,
        }
    }

    /// Render tata letak matematika, tabel, grafik, dan geometri langsung ke canvas `egui::Painter`
    pub fn render(&self, painter: &Painter, pos: Pos2, zoom: f32, rotation: f32, color: Color32) {
        let center = Pos2::new(
            pos.x + self.width * zoom * 0.5,
            pos.y + self.height * zoom * 0.5,
        );
        let cos_a = rotation.cos();
        let sin_a = rotation.sin();
        let rot_pt = |pt: Pos2| -> Pos2 {
            if rotation.abs() < 1e-4 {
                pt
            } else {
                let dx = pt.x - center.x;
                let dy = pt.y - center.y;
                Pos2::new(
                    center.x + dx * cos_a - dy * sin_a,
                    center.y + dx * sin_a + dy * cos_a,
                )
            }
        };
        let to_screen = |local: Pos2| -> Pos2 {
            rot_pt(Pos2::new(pos.x + local.x * zoom, pos.y + local.y * zoom))
        };

        let draw_styled_seg = |p1: Pos2, p2: Pos2, sw: f32, col: Color32, dash: MathLineDash| {
            let stroke = Stroke::new(sw, col);
            match dash {
                MathLineDash::Solid => {
                    painter.line_segment([p1, p2], stroke);
                }
                MathLineDash::Dashed | MathLineDash::Dotted => {
                    let d = p2 - p1;
                    let len = d.length();
                    if len <= 1e-3 {
                        return;
                    }
                    let (dash_len, gap_len) = if dash == MathLineDash::Dashed {
                        ((5.0 * zoom).clamp(3.0, 14.0), (3.5 * zoom).clamp(2.5, 10.0))
                    } else {
                        ((1.8 * zoom).clamp(1.2, 5.0), (3.2 * zoom).clamp(2.0, 9.0))
                    };
                    let u = d / len;
                    let mut dist = 0.0_f32;
                    while dist < len {
                        let seg_end = (dist + dash_len).min(len);
                        painter.line_segment([p1 + u * dist, p1 + u * seg_end], stroke);
                        dist += dash_len + gap_len;
                    }
                }
            }
        };

        let draw_arrowhead = |tip: Pos2, from: Pos2, sw: f32, col: Color32| {
            let dir = tip - from;
            let len = dir.length();
            if len <= 1e-3 {
                return;
            }
            let u = dir / len;
            let perp = egui::Vec2::new(-u.y, u.x);
            let head_len = (7.5 * zoom).max(sw * 3.0).clamp(5.0, 14.0);
            let head_half_w = head_len * 0.42;
            let base = tip - u * head_len;
            painter.add(egui::Shape::convex_polygon(
                vec![tip, base + perp * head_half_w, base - perp * head_half_w],
                col,
                Stroke::NONE,
            ));
        };

        for item in &self.items {
            match item {
                MathLayoutItem::Glyph {
                    text,
                    font_size,
                    is_italic,
                    pos: gpos,
                } => {
                    let scaled_font = (*font_size * zoom).clamp(2.0, 400.0);
                    let font_id = egui::FontId::new(scaled_font, egui::FontFamily::Proportional);
                    let final_pos = to_screen(*gpos);
                    let mut job = egui::text::LayoutJob::default();
                    job.append(
                        text,
                        0.0,
                        egui::TextFormat {
                            font_id,
                            color,
                            italics: *is_italic,
                            ..Default::default()
                        },
                    );
                    let galley = painter.layout_job(job);
                    let mut text_shape = egui::epaint::TextShape::new(final_pos, galley, color);
                    if rotation.abs() > 1e-4 {
                        text_shape.angle = rotation;
                    }
                    painter.add(text_shape);
                }
                MathLayoutItem::ColoredGlyph {
                    text,
                    font_size,
                    is_italic,
                    is_bold,
                    pos: gpos,
                    color: item_col,
                } => {
                    let eff_col = item_col.unwrap_or(color);
                    let scaled_font = (*font_size * zoom).clamp(2.0, 400.0);
                    let (fam, needs_faux_bold) = painter.fonts(|fonts| {
                        let reg = fonts.families();
                        crate::model::FontFamilyPreset::SansSerif
                            .resolve_egui_font_family(*is_bold, |f| reg.contains(f))
                    });
                    let font_id = egui::FontId::new(scaled_font, fam);
                    let final_pos = to_screen(*gpos);
                    let mut job = egui::text::LayoutJob::default();
                    job.append(
                        text,
                        0.0,
                        egui::TextFormat {
                            font_id,
                            color: eff_col,
                            italics: *is_italic,
                            ..Default::default()
                        },
                    );
                    let galley = painter.layout_job(job);
                    let mut text_shape =
                        egui::epaint::TextShape::new(final_pos, galley.clone(), eff_col);
                    if rotation.abs() > 1e-4 {
                        text_shape.angle = rotation;
                    }
                    painter.add(text_shape);
                    if needs_faux_bold {
                        let bd = (scaled_font * 0.035).clamp(0.5, 2.0);
                        let mut bshape = egui::epaint::TextShape::new(
                            final_pos + egui::Vec2::new(bd * cos_a, bd * sin_a),
                            galley,
                            eff_col,
                        );
                        if rotation.abs() > 1e-4 {
                            bshape.angle = rotation;
                        }
                        painter.add(bshape);
                    }
                }
                MathLayoutItem::Line {
                    start,
                    end,
                    stroke_width,
                } => {
                    painter.line_segment(
                        [to_screen(*start), to_screen(*end)],
                        Stroke::new((*stroke_width * zoom).max(1.0), color),
                    );
                }
                MathLayoutItem::StyledLine {
                    start,
                    end,
                    stroke_width,
                    color: item_col,
                    dash,
                    arrow_start,
                    arrow_end,
                } => {
                    let eff_col = item_col.unwrap_or(color);
                    let sw = (*stroke_width * zoom).max(0.8);
                    let p1 = to_screen(*start);
                    let p2 = to_screen(*end);
                    draw_styled_seg(p1, p2, sw, eff_col, *dash);
                    if *arrow_end {
                        draw_arrowhead(p2, p1, sw, eff_col);
                    }
                    if *arrow_start {
                        draw_arrowhead(p1, p2, sw, eff_col);
                    }
                }
                MathLayoutItem::Polyline {
                    points,
                    stroke_width,
                    color: item_col,
                    dash,
                    arrow_start,
                    arrow_end,
                } => {
                    if points.len() < 2 {
                        continue;
                    }
                    let eff_col = item_col.unwrap_or(color);
                    let sw = (*stroke_width * zoom).max(0.8);
                    let screen_pts: Vec<Pos2> = points.iter().map(|&p| to_screen(p)).collect();
                    for w in screen_pts.windows(2) {
                        draw_styled_seg(w[0], w[1], sw, eff_col, *dash);
                    }
                    if *arrow_end {
                        let n = screen_pts.len();
                        draw_arrowhead(screen_pts[n - 1], screen_pts[n - 2], sw, eff_col);
                    }
                    if *arrow_start {
                        draw_arrowhead(screen_pts[0], screen_pts[1], sw, eff_col);
                    }
                }
                MathLayoutItem::Polygon {
                    points,
                    fill,
                    stroke_color,
                    stroke_width,
                } => {
                    if points.len() < 3 {
                        continue;
                    }
                    let screen_pts: Vec<Pos2> = points.iter().map(|&p| to_screen(p)).collect();
                    if let Some(fill_col) = fill {
                        painter.add(egui::Shape::convex_polygon(
                            screen_pts.clone(),
                            *fill_col,
                            Stroke::NONE,
                        ));
                    }
                    if let Some(sc) = stroke_color {
                        let stroke = Stroke::new((*stroke_width * zoom).max(0.4), *sc);
                        for i in 0..screen_pts.len() {
                            let j = (i + 1) % screen_pts.len();
                            painter.line_segment([screen_pts[i], screen_pts[j]], stroke);
                        }
                    } else if fill.is_none() {
                        let stroke = Stroke::new((*stroke_width * zoom).max(0.4), color);
                        for i in 0..screen_pts.len() {
                            let j = (i + 1) % screen_pts.len();
                            painter.line_segment([screen_pts[i], screen_pts[j]], stroke);
                        }
                    }
                }
                MathLayoutItem::Ellipse {
                    center: ec,
                    rx,
                    ry,
                    fill,
                    stroke_color,
                    stroke_width,
                } => {
                    let segs = 36;
                    let mut pts = Vec::with_capacity(segs);
                    for k in 0..segs {
                        let a = (k as f32 / segs as f32) * std::f32::consts::TAU;
                        pts.push(to_screen(Pos2::new(
                            ec.x + *rx * a.cos(),
                            ec.y + *ry * a.sin(),
                        )));
                    }
                    if let Some(fill_col) = fill {
                        painter.add(egui::Shape::convex_polygon(
                            pts.clone(),
                            *fill_col,
                            Stroke::NONE,
                        ));
                    }
                    if let Some(sc) = stroke_color {
                        let stroke = Stroke::new((*stroke_width * zoom).max(0.8), *sc);
                        for i in 0..pts.len() {
                            let j = (i + 1) % pts.len();
                            painter.line_segment([pts[i], pts[j]], stroke);
                        }
                    }
                }
            }
        }
    }

    /// Menghasilkan markup elemen vektor SVG untuk formula matematika, tabel, grafik, dan geometri
    pub fn to_svg(&self, x: f32, y: f32, color: &crate::model::ColorData, rotation: f32) -> String {
        let mut out = String::new();
        let fmt_color = |c: Color32| -> String {
            format!(
                "rgba({},{},{},{:.3})",
                c.r(),
                c.g(),
                c.b(),
                c.a() as f32 / 255.0
            )
        };
        let default_col_c = Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
        let default_col = fmt_color(default_col_c);

        let has_rot = rotation.abs() > 1e-4;
        if has_rot {
            let cx = x + self.width * 0.5;
            let cy = y + self.height * 0.5;
            out.push_str(&format!(
                "<g transform=\"rotate({:.2} {:.1} {:.1})\">\n",
                rotation.to_degrees(),
                cx,
                cy
            ));
        }

        let dash_attr = |dash: MathLineDash| -> &'static str {
            match dash {
                MathLineDash::Solid => "",
                MathLineDash::Dashed => " stroke-dasharray=\"5,4\"",
                MathLineDash::Dotted => " stroke-dasharray=\"2,3\"",
            }
        };

        let emit_svg_arrow = |out: &mut String, tip: Pos2, from: Pos2, sw: f32, col_str: &str| {
            let dir = tip - from;
            let len = dir.length();
            if len <= 1e-3 {
                return;
            }
            let u = dir / len;
            let perp = egui::Vec2::new(-u.y, u.x);
            let head_len = 7.5_f32.max(sw * 3.0).clamp(5.0, 14.0);
            let half_w = head_len * 0.42;
            let base = tip - u * head_len;
            let p_a = base + perp * half_w;
            let p_b = base - perp * half_w;
            out.push_str(&format!(
                "<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"{col_str}\"/>\n",
                x + tip.x,
                y + tip.y,
                x + p_a.x,
                y + p_a.y,
                x + p_b.x,
                y + p_b.y,
            ));
        };

        for item in &self.items {
            match item {
                MathLayoutItem::Glyph {
                    text,
                    font_size,
                    is_italic,
                    pos,
                } => {
                    let gx = x + pos.x;
                    let gy = y + pos.y + font_size * 0.8;
                    let italic_attr = if *is_italic {
                        " font-style=\"italic\""
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "<text x=\"{gx:.1}\" y=\"{gy:.1}\" font-family=\"serif\" font-size=\"{font_size:.1}\" fill=\"{default_col}\"{italic_attr}>{}</text>\n",
                        escape_xml(text)
                    ));
                }
                MathLayoutItem::ColoredGlyph {
                    text,
                    font_size,
                    is_italic,
                    is_bold,
                    pos,
                    color: item_col,
                } => {
                    let gx = x + pos.x;
                    let gy = y + pos.y + font_size * 0.8;
                    let col_str = item_col
                        .map(fmt_color)
                        .unwrap_or_else(|| default_col.clone());
                    let italic_attr = if *is_italic {
                        " font-style=\"italic\""
                    } else {
                        ""
                    };
                    let bold_attr = if *is_bold {
                        " font-weight=\"bold\""
                    } else {
                        ""
                    };
                    let ff = if *is_italic { "serif" } else { "sans-serif" };
                    out.push_str(&format!(
                        "<text x=\"{gx:.1}\" y=\"{gy:.1}\" font-family=\"{ff}\" font-size=\"{font_size:.1}\" fill=\"{col_str}\"{italic_attr}{bold_attr}>{}</text>\n",
                        escape_xml(text)
                    ));
                }
                MathLayoutItem::Line {
                    start,
                    end,
                    stroke_width,
                } => {
                    out.push_str(&format!(
                        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{default_col}\" stroke-width=\"{stroke_width:.1}\"/>\n",
                        x + start.x,
                        y + start.y,
                        x + end.x,
                        y + end.y
                    ));
                }
                MathLayoutItem::StyledLine {
                    start,
                    end,
                    stroke_width,
                    color: item_col,
                    dash,
                    arrow_start,
                    arrow_end,
                } => {
                    let col_str = item_col
                        .map(fmt_color)
                        .unwrap_or_else(|| default_col.clone());
                    let da = dash_attr(*dash);
                    out.push_str(&format!(
                        "<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{col_str}\" stroke-width=\"{stroke_width:.1}\"{da}/>\n",
                        x + start.x,
                        y + start.y,
                        x + end.x,
                        y + end.y
                    ));
                    if *arrow_end {
                        emit_svg_arrow(&mut out, *end, *start, *stroke_width, &col_str);
                    }
                    if *arrow_start {
                        emit_svg_arrow(&mut out, *start, *end, *stroke_width, &col_str);
                    }
                }
                MathLayoutItem::Polyline {
                    points,
                    stroke_width,
                    color: item_col,
                    dash,
                    arrow_start,
                    arrow_end,
                } => {
                    if points.len() < 2 {
                        continue;
                    }
                    let col_str = item_col
                        .map(fmt_color)
                        .unwrap_or_else(|| default_col.clone());
                    let da = dash_attr(*dash);
                    let pts_str: String = points
                        .iter()
                        .map(|p| format!("{:.1},{:.1}", x + p.x, y + p.y))
                        .collect::<Vec<_>>()
                        .join(" ");
                    out.push_str(&format!(
                        "<polyline points=\"{pts_str}\" fill=\"none\" stroke=\"{col_str}\" stroke-width=\"{stroke_width:.1}\"{da}/>\n"
                    ));
                    if *arrow_end {
                        let n = points.len();
                        emit_svg_arrow(
                            &mut out,
                            points[n - 1],
                            points[n - 2],
                            *stroke_width,
                            &col_str,
                        );
                    }
                    if *arrow_start {
                        emit_svg_arrow(&mut out, points[0], points[1], *stroke_width, &col_str);
                    }
                }
                MathLayoutItem::Polygon {
                    points,
                    fill,
                    stroke_color,
                    stroke_width,
                } => {
                    if points.len() < 3 {
                        continue;
                    }
                    let fill_str = fill.map(fmt_color).unwrap_or_else(|| "none".into());
                    let stroke_str = stroke_color.map(fmt_color).unwrap_or_else(|| {
                        if fill.is_none() {
                            default_col.clone()
                        } else {
                            "none".into()
                        }
                    });
                    let pts_str: String = points
                        .iter()
                        .map(|p| format!("{:.1},{:.1}", x + p.x, y + p.y))
                        .collect::<Vec<_>>()
                        .join(" ");
                    out.push_str(&format!(
                        "<polygon points=\"{pts_str}\" fill=\"{fill_str}\" stroke=\"{stroke_str}\" stroke-width=\"{stroke_width:.1}\"/>\n"
                    ));
                }
                MathLayoutItem::Ellipse {
                    center,
                    rx,
                    ry,
                    fill,
                    stroke_color,
                    stroke_width,
                } => {
                    let fill_str = fill.map(fmt_color).unwrap_or_else(|| "none".into());
                    let stroke_str = stroke_color.map(fmt_color).unwrap_or_else(|| {
                        if fill.is_none() {
                            default_col.clone()
                        } else {
                            "none".into()
                        }
                    });
                    out.push_str(&format!(
                        "<ellipse cx=\"{:.1}\" cy=\"{:.1}\" rx=\"{rx:.1}\" ry=\"{ry:.1}\" fill=\"{fill_str}\" stroke=\"{stroke_str}\" stroke-width=\"{stroke_width:.1}\"/>\n",
                        x + center.x,
                        y + center.y
                    ));
                }
            }
        }

        if has_rot {
            out.push_str("</g>\n");
        }

        out
    }
}
