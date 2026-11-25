const MAX_EXPR_DEPTH: usize = 64;

/// Mengevaluasi ekspresi matematika konstan tanpa variabel.
pub fn eval_const_expr(expr: &str) -> Option<f64> {
    eval_math_expr(expr, 0.0)
}

/// Mengevaluasi ekspresi matematika PGFPlots/LaTeX secara aman untuk satu variabel `x` (atau `t`).
/// Mendukung `+`, `-`, `*`, `/`, `^`, perkalian implisit (`2x`, `3(x+1)`), konstanta (`pi`, `e`),
/// serta fungsi matematika standar dan konversi sudut PGFPlots (`sin(deg(x))` maupun `sin(x)`).
pub fn eval_math_expr(expr: &str, x: f64) -> Option<f64> {
    eval_math_expr_2d(expr, x, 0.0)
}

/// Mengevaluasi ekspresi matematika PGFPlots/LaTeX dua variabel `(x, y)` (digunakan untuk grafik 3D `\addplot3`).
pub fn eval_math_expr_2d(expr: &str, x: f64, y: f64) -> Option<f64> {
    let normalized = preprocess_expr(expr);
    if normalized.is_empty() {
        return None;
    }
    let tokens = tokenize_expr(&normalized)?;
    if tokens.is_empty() {
        return None;
    }
    let mut parser = ExprParser {
        tokens: &tokens,
        pos: 0,
        x,
        y,
    };
    let val = parser.parse_add_sub(0)?;
    if parser.pos < parser.tokens.len() || !val.is_finite() {
        return None;
    }
    Some(val)
}

fn preprocess_expr(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    // Hapus satuan umum TikZ/LaTeX jika tidak mengganggu variabel
    for unit in ["pt", "cm", "mm", "em", "ex"] {
        if let Some(stripped) = s.strip_suffix(unit)
            && stripped
                .trim()
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | ' '))
        {
            s = stripped.trim().to_string();
        }
    }
    s = s
        .replace(r"\cdot", "*")
        .replace(r"\times", "*")
        .replace(r"\div", "/")
        .replace(r"\left(", "(")
        .replace(r"\right)", ")")
        .replace(r"\left[", "(")
        .replace(r"\right]", ")")
        .replace(r"\pi", "pi")
        .replace(r"\sin", "sin")
        .replace(r"\cos", "cos")
        .replace(r"\tan", "tan")
        .replace(r"\arcsin", "asin")
        .replace(r"\arccos", "acos")
        .replace(r"\arctan", "atan")
        .replace(r"\asin", "asin")
        .replace(r"\acos", "acos")
        .replace(r"\atan", "atan")
        .replace(r"\sinh", "sinh")
        .replace(r"\cosh", "cosh")
        .replace(r"\tanh", "tanh")
        .replace(r"\sqrt", "sqrt")
        .replace(r"\abs", "abs")
        .replace(r"\ln", "ln")
        .replace(r"\log", "log10")
        .replace(r"\exp", "exp")
        .replace(r"\x", "x")
        .replace(r"\y", "y")
        .replace(r"\t", "t")
        .replace('{', "(")
        .replace('}', ")");
    s
}

#[derive(Clone, Debug, PartialEq)]
enum ExprTok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
}

fn tokenize_expr(input: &str) -> Option<Vec<ExprTok>> {
    let mut out = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || c == '.' {
            let start = i;
            let mut has_dot = c == '.';
            i += 1;
            while i < chars.len() {
                let ch = chars[i];
                if ch.is_ascii_digit() {
                    i += 1;
                } else if ch == '.' && !has_dot {
                    has_dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            // Dukungan notasi ilmiah seperti 1e-3 atau 2.5E4
            if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
                let save = i;
                i += 1;
                if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                    i += 1;
                }
                let exp_digits_start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i == exp_digits_start {
                    i = save;
                }
            }
            let num_str: String = chars[start..i].iter().collect();
            let v: f64 = num_str.parse().ok()?;
            out.push(ExprTok::Num(v));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            out.push(ExprTok::Ident(ident.to_ascii_lowercase()));
        } else {
            let tok = match c {
                '+' => ExprTok::Plus,
                '-' => ExprTok::Minus,
                '*' => ExprTok::Star,
                '/' => ExprTok::Slash,
                '^' => ExprTok::Caret,
                '(' | '[' => ExprTok::LParen,
                ')' | ']' => ExprTok::RParen,
                _ => return None,
            };
            out.push(tok);
            i += 1;
        }
    }
    Some(out)
}

struct ExprParser<'a> {
    tokens: &'a [ExprTok],
    pos: usize,
    x: f64,
    y: f64,
}

impl<'a> ExprParser<'a> {
    fn peek(&self) -> Option<&ExprTok> {
        self.tokens.get(self.pos)
    }

    fn starts_implicit_mul(&self) -> bool {
        matches!(
            self.peek(),
            Some(ExprTok::Num(_) | ExprTok::Ident(_) | ExprTok::LParen)
        )
    }

    fn parse_add_sub(&mut self, depth: usize) -> Option<f64> {
        if depth > MAX_EXPR_DEPTH {
            return None;
        }
        let mut lhs = self.parse_mul_div(depth + 1)?;
        while let Some(tok) = self.peek() {
            match tok {
                ExprTok::Plus => {
                    self.pos += 1;
                    let rhs = self.parse_mul_div(depth + 1)?;
                    lhs += rhs;
                }
                ExprTok::Minus => {
                    self.pos += 1;
                    let rhs = self.parse_mul_div(depth + 1)?;
                    lhs -= rhs;
                }
                _ => break,
            }
        }
        lhs.is_finite().then_some(lhs)
    }

    fn parse_mul_div(&mut self, depth: usize) -> Option<f64> {
        if depth > MAX_EXPR_DEPTH {
            return None;
        }
        let mut lhs = self.parse_unary(depth + 1)?;
        while let Some(tok) = self.peek() {
            match tok {
                ExprTok::Star => {
                    self.pos += 1;
                    let rhs = self.parse_unary(depth + 1)?;
                    lhs *= rhs;
                }
                ExprTok::Slash => {
                    self.pos += 1;
                    let rhs = self.parse_unary(depth + 1)?;
                    if rhs.abs() < 1e-12 {
                        return None;
                    }
                    lhs /= rhs;
                }
                _ if self.starts_implicit_mul() => {
                    let rhs = self.parse_unary(depth + 1)?;
                    lhs *= rhs;
                }
                _ => break,
            }
        }
        lhs.is_finite().then_some(lhs)
    }

    fn parse_unary(&mut self, depth: usize) -> Option<f64> {
        if depth > MAX_EXPR_DEPTH {
            return None;
        }
        match self.peek() {
            Some(ExprTok::Plus) => {
                self.pos += 1;
                self.parse_unary(depth + 1)
            }
            Some(ExprTok::Minus) => {
                self.pos += 1;
                let v = self.parse_unary(depth + 1)?;
                Some(-v)
            }
            _ => self.parse_power(depth + 1),
        }
    }

    fn parse_power(&mut self, depth: usize) -> Option<f64> {
        if depth > MAX_EXPR_DEPTH {
            return None;
        }
        let base = self.parse_primary(depth + 1)?;
        if matches!(self.peek(), Some(ExprTok::Caret)) {
            self.pos += 1;
            let exp = self.parse_unary(depth + 1)?;
            let v = base.powf(exp);
            return v.is_finite().then_some(v);
        }
        Some(base)
    }

    fn parse_primary(&mut self, depth: usize) -> Option<f64> {
        if depth > MAX_EXPR_DEPTH {
            return None;
        }
        match self.peek()?.clone() {
            ExprTok::Num(v) => {
                self.pos += 1;
                Some(v)
            }
            ExprTok::LParen => {
                self.pos += 1;
                let v = self.parse_add_sub(depth + 1)?;
                if !matches!(self.peek(), Some(ExprTok::RParen)) {
                    return None;
                }
                self.pos += 1;
                Some(v)
            }
            ExprTok::Ident(name) => {
                self.pos += 1;
                if matches!(self.peek(), Some(ExprTok::LParen)) {
                    self.pos += 1;
                    // Deteksi pola PGFPlots `sin(deg(arg))` / `cos(deg(arg))` / `tan(deg(arg))`
                    let is_trig = matches!(name.as_str(), "sin" | "cos" | "tan");
                    let inner_deg = if is_trig
                        && matches!(self.peek(), Some(ExprTok::Ident(sub)) if sub == "deg")
                        && matches!(self.tokens.get(self.pos + 1), Some(ExprTok::LParen))
                    {
                        self.pos += 2;
                        let arg = self.parse_add_sub(depth + 1)?;
                        if !matches!(self.peek(), Some(ExprTok::RParen)) {
                            return None;
                        }
                        self.pos += 1;
                        Some(arg)
                    } else {
                        None
                    };
                    let arg = if let Some(v) = inner_deg {
                        v
                    } else {
                        self.parse_add_sub(depth + 1)?
                    };
                    if !matches!(self.peek(), Some(ExprTok::RParen)) {
                        return None;
                    }
                    self.pos += 1;
                    let res = apply_func(&name, arg)?;
                    res.is_finite().then_some(res)
                } else {
                    match name.as_str() {
                        "x" | "t" => Some(self.x),
                        "y" => Some(self.y),
                        "pi" => Some(std::f64::consts::PI),
                        "e" => Some(std::f64::consts::E),
                        // Tangani variabel berimpit seperti "ax" tidak valid; "xx" dll. tidak umum, tapi dukung konstanta & x/y/t
                        _ => None,
                    }
                }
            }
            _ => None,
        }
    }
}

fn apply_func(name: &str, arg: f64) -> Option<f64> {
    let val = match name {
        "sin" => arg.sin(),
        "cos" => arg.cos(),
        "tan" => {
            let c = arg.cos();
            if c.abs() < 1e-6 {
                return None;
            }
            arg.tan()
        }
        "asin" => arg.asin(),
        "acos" => arg.acos(),
        "atan" => arg.atan(),
        "sinh" => arg.sinh(),
        "cosh" => arg.cosh(),
        "tanh" => arg.tanh(),
        "sqrt" => {
            if arg < 0.0 {
                return None;
            }
            arg.sqrt()
        }
        "abs" => arg.abs(),
        "ln" | "log" => {
            if arg <= 0.0 {
                return None;
            }
            arg.ln()
        }
        "log10" => {
            if arg <= 0.0 {
                return None;
            }
            arg.log10()
        }
        "exp" => arg.exp(),
        "floor" => arg.floor(),
        "ceil" => arg.ceil(),
        "deg" => arg.to_degrees(),
        "rad" => arg.to_radians(),
        _ => return None,
    };
    val.is_finite().then_some(val)
}
