use eframe::egui::{Color32, Painter, Pos2, Stroke};

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
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
}

/// Node Pohon Sintaks Abstrak (AST) Formula Matematika
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
}

/// Kesalahan saat parsing LaTeX matematika
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

/// Tokenizer sederhana untuk ekspresi LaTeX
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
                    if cmd == "text" || cmd == "mathrm" || cmd == "mathbf" {
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
                    // Simbol seperti \{, \}, \,, \;, dll.
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

/// Parser rekursif untuk mengubah token LaTeX menjadi AST MathNode
pub fn parse_latex_math(input: &str) -> Result<Vec<MathNode>, MathParseError> {
    let tokens = tokenize_latex(input);
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
                "text" | "mathrm" | "mathbf" => {
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
            // Periksa jika langsung diikuti superscript: e.g. x_i^2
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

/// Item elemen visual hasil perhitungan tata letak 2D formula matematika
#[derive(Clone, Debug, PartialEq)]
pub enum MathLayoutItem {
    Glyph {
        text: String,
        font_size: f32,
        is_italic: bool,
        pos: Pos2,
    },
    Line {
        start: Pos2,
        end: Pos2,
        stroke_width: f32,
    },
}

/// Tata letak 2D lengkap untuk formula matematika
#[derive(Clone, Debug, PartialEq)]
pub struct MathLayout {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
    pub items: Vec<MathLayoutItem>,
}

impl MathLayout {
    /// Menghitung tata letak 2D langsung dari string LaTeX
    pub fn compute_from_str(latex: &str, font_size: f32) -> Self {
        let nodes = parse_latex_math(latex).unwrap_or_default();
        Self::compute(&nodes, font_size)
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

                    // Posisikan pembilang (numerator) di atas garis
                    let num_x = cur_x + (frac_w - num_layout.width) * 0.5;
                    let num_y = bar_y - num_layout.height - 2.0;
                    for mut it in num_layout.items {
                        match &mut it {
                            MathLayoutItem::Glyph { pos, .. } => {
                                pos.x += num_x;
                                pos.y += num_y;
                            }
                            MathLayoutItem::Line { start, end, .. } => {
                                start.x += num_x;
                                start.y += num_y;
                                end.x += num_x;
                                end.y += num_y;
                            }
                        }
                        items.push(it);
                    }

                    // Garis pecahan horizontal
                    items.push(MathLayoutItem::Line {
                        start: Pos2::new(cur_x + 1.0, bar_y),
                        end: Pos2::new(cur_x + frac_w - 1.0, bar_y),
                        stroke_width: bar_thickness,
                    });

                    // Posisikan penyebut (denominator) di bawah garis
                    let den_x = cur_x + (frac_w - den_layout.width) * 0.5;
                    let den_y = bar_y + 4.0;
                    for mut it in den_layout.items {
                        match &mut it {
                            MathLayoutItem::Glyph { pos, .. } => {
                                pos.x += den_x;
                                pos.y += den_y;
                            }
                            MathLayoutItem::Line { start, end, .. } => {
                                start.x += den_x;
                                start.y += den_y;
                                end.x += den_x;
                                end.y += den_y;
                            }
                        }
                        items.push(it);
                    }

                    cur_x += frac_w + 3.0;
                }
                MathNode::SubSup { base, sub, sup } => {
                    let base_w = if let Some(b) = base {
                        let b_layout = MathLayout::compute(&[*b.clone()], font_size);
                        let w = b_layout.width;
                        for mut it in b_layout.items {
                            match &mut it {
                                MathLayoutItem::Glyph { pos, .. } => pos.x += cur_x,
                                MathLayoutItem::Line { start, end, .. } => {
                                    start.x += cur_x;
                                    end.x += cur_x;
                                }
                            }
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
                            match &mut it {
                                MathLayoutItem::Glyph { pos, .. } => {
                                    pos.x += sup_x;
                                    pos.y += sup_y;
                                }
                                MathLayoutItem::Line { start, end, .. } => {
                                    start.x += sup_x;
                                    start.y += sup_y;
                                    end.x += sup_x;
                                    end.y += sup_y;
                                }
                            }
                            items.push(it);
                        }
                    }

                    if let Some(sb) = sub {
                        let sub_layout = MathLayout::compute(sb, script_size);
                        script_w = script_w.max(sub_layout.width);
                        let sub_x = cur_x + base_w + 1.0;
                        let sub_y = font_size * 0.45;
                        for mut it in sub_layout.items {
                            match &mut it {
                                MathLayoutItem::Glyph { pos, .. } => {
                                    pos.x += sub_x;
                                    pos.y += sub_y;
                                }
                                MathLayoutItem::Line { start, end, .. } => {
                                    start.x += sub_x;
                                    start.y += sub_y;
                                    end.x += sub_x;
                                    end.y += sub_y;
                                }
                            }
                            items.push(it);
                        }
                    }

                    cur_x += base_w + script_w + 2.0;
                }
                MathNode::Radical { index: _, radicand } => {
                    let rad_layout = MathLayout::compute(radicand, font_size);
                    let bar_thickness = (font_size * 0.07).clamp(1.2, 3.0);
                    let rad_w = rad_layout.width + 12.0;

                    // Gambar simbol akar '√'
                    let sym_x = cur_x;
                    items.push(MathLayoutItem::Glyph {
                        text: "√".into(),
                        font_size: font_size * 1.15,
                        is_italic: false,
                        pos: Pos2::new(sym_x, -1.0),
                    });

                    // Garis horizontal penutup akar di atas isi radikan
                    let line_y = -font_size * 0.15;
                    items.push(MathLayoutItem::Line {
                        start: Pos2::new(sym_x + font_size * 0.55, line_y),
                        end: Pos2::new(sym_x + rad_w, line_y),
                        stroke_width: bar_thickness,
                    });

                    // Posisikan isi radikan di dalam akar
                    let cont_x = sym_x + font_size * 0.55 + 2.0;
                    for mut it in rad_layout.items {
                        match &mut it {
                            MathLayoutItem::Glyph { pos, .. } => pos.x += cont_x,
                            MathLayoutItem::Line { start, end, .. } => {
                                start.x += cont_x;
                                end.x += cont_x;
                            }
                        }
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
                        match &mut it {
                            MathLayoutItem::Glyph { pos, .. } => pos.x += off_x,
                            MathLayoutItem::Line { start, end, .. } => {
                                start.x += off_x;
                                end.x += off_x;
                            }
                        }
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
            }
        }

        let total_w = cur_x.max(font_size * 0.5);
        let total_h = font_size * 1.3;

        Self {
            width: total_w,
            height: total_h,
            ascent: base_ascent,
            descent: base_descent,
            items,
        }
    }

    /// Render tata letak matematika langsung ke canvas `egui::Painter`
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

        for item in &self.items {
            match item {
                MathLayoutItem::Glyph {
                    text,
                    font_size,
                    is_italic: _,
                    pos: gpos,
                } => {
                    let scaled_font = (*font_size * zoom).clamp(4.0, 160.0);
                    let font_id = egui::FontId::new(scaled_font, egui::FontFamily::Proportional);
                    let item_screen_pos = Pos2::new(pos.x + gpos.x * zoom, pos.y + gpos.y * zoom);
                    let final_pos = rot_pt(item_screen_pos);

                    let galley = painter.layout_no_wrap(text.clone(), font_id, color);
                    let mut text_shape = egui::epaint::TextShape::new(final_pos, galley, color);
                    if rotation.abs() > 1e-4 {
                        text_shape.angle = rotation;
                    }
                    painter.add(text_shape);
                }
                MathLayoutItem::Line {
                    start,
                    end,
                    stroke_width,
                } => {
                    let p1 = Pos2::new(pos.x + start.x * zoom, pos.y + start.y * zoom);
                    let p2 = Pos2::new(pos.x + end.x * zoom, pos.y + end.y * zoom);
                    let final_p1 = rot_pt(p1);
                    let final_p2 = rot_pt(p2);
                    painter.line_segment(
                        [final_p1, final_p2],
                        Stroke::new((*stroke_width * zoom).max(1.0), color),
                    );
                }
            }
        }
    }

    /// Menghasilkan markup elemen vektor SVG untuk formula matematika
    pub fn to_svg(&self, x: f32, y: f32, color: &crate::model::ColorData, rotation: f32) -> String {
        let mut out = String::new();
        let text_col = format!(
            "rgba({},{},{},{:.3})",
            color.r,
            color.g,
            color.b,
            color.a as f32 / 255.0
        );

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
                        "<text x=\"{gx:.1}\" y=\"{gy:.1}\" font-family=\"serif\" font-size=\"{font_size:.1}\" fill=\"{text_col}\"{italic_attr}>{}</text>\n",
                        escape_xml(text)
                    ));
                }
                MathLayoutItem::Line {
                    start,
                    end,
                    stroke_width,
                } => {
                    let x1 = x + start.x;
                    let y1 = y + start.y;
                    let x2 = x + end.x;
                    let y2 = y + end.y;
                    out.push_str(&format!(
                        "<line x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\" stroke=\"{text_col}\" stroke-width=\"{stroke_width:.1}\"/>\n"
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
