use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub parts: Vec<SelectorPart>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectorPart {
    Simple(SimpleSelector),
    DescendantCombinator, // space
    ChildCombinator,      // >
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SimpleSelector {
    pub tag_name: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub universal: bool,
}

pub type Specificity = (usize, usize, usize);

impl Selector {
    pub fn specificity(&self) -> Specificity {
        let mut id_count = 0;
        let mut class_count = 0;
        let mut tag_count = 0;

        for part in &self.parts {
            if let SelectorPart::Simple(s) = part {
                if s.id.is_some() {
                    id_count += 1;
                }
                class_count += s.classes.len();
                if s.tag_name.is_some() {
                    tag_count += 1;
                }
            }
        }
        (id_count, class_count, tag_count)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Keyword(String),
    Length(f32, Unit),
    ColorValue(CssColor),
    Number(f32),
    StringLiteral(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Px,
    Em,
    Rem,
    Percent,
    Pt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CssColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl CssColor {
    pub const TRANSPARENT: CssColor = CssColor { r: 0, g: 0, b: 0, a: 0 };
    #[allow(dead_code)]
    pub const BLACK: CssColor = CssColor { r: 0, g: 0, b: 0, a: 255 };
    #[allow(dead_code)]
    pub const WHITE: CssColor = CssColor { r: 255, g: 255, b: 255, a: 255 };

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        CssColor { r, g, b, a }
    }
}

impl fmt::Display for CssColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rgba({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }
}

pub struct CssParser {
    pos: usize,
    input: Vec<char>,
}

impl CssParser {
    pub fn parse(source: &str) -> Stylesheet {
        let mut parser = CssParser {
            pos: 0,
            input: source.chars().collect(),
        };
        parser.parse_stylesheet()
    }

    fn parse_stylesheet(&mut self) -> Stylesheet {
        let mut rules = Vec::new();
        while !self.eof() {
            self.consume_whitespace_and_comments();
            if self.eof() {
                break;
            }
            if self.starts_with("@") {
                // Skip @media, @import, @keyframes for simplicity
                self.skip_at_rule();
                continue;
            }
            if let Some(rule) = self.parse_rule() {
                rules.push(rule);
            }
        }
        Stylesheet { rules }
    }

    fn skip_at_rule(&mut self) {
        while !self.eof() && self.curr_char() != '{' && self.curr_char() != ';' {
            self.consume_char();
        }
        if !self.eof() && self.curr_char() == '{' {
            self.consume_char();
            let mut depth = 1;
            while !self.eof() && depth > 0 {
                let c = self.consume_char();
                if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                }
            }
        } else if !self.eof() && self.curr_char() == ';' {
            self.consume_char();
        }
    }

    fn parse_rule(&mut self) -> Option<Rule> {
        let selectors = self.parse_selectors();
        if selectors.is_empty() {
            // Error recovery: skip to next closing brace
            while !self.eof() && self.curr_char() != '}' {
                self.consume_char();
            }
            if !self.eof() && self.curr_char() == '}' {
                self.consume_char();
            }
            return None;
        }

        self.consume_whitespace_and_comments();
        if self.curr_char() != '{' {
            return None;
        }
        self.consume_char(); // consume '{'

        let declarations = self.parse_declarations();

        self.consume_whitespace_and_comments();
        if self.curr_char() == '}' {
            self.consume_char();
        }

        Some(Rule {
            selectors,
            declarations,
        })
    }

    fn parse_selectors(&mut self) -> Vec<Selector> {
        let mut selectors = Vec::new();
        loop {
            self.consume_whitespace_and_comments();
            if self.eof() || self.curr_char() == '{' {
                break;
            }
            if let Some(sel) = self.parse_selector() {
                selectors.push(sel);
            }
            self.consume_whitespace_and_comments();
            if self.curr_char() == ',' {
                self.consume_char();
            } else {
                break;
            }
        }
        selectors
    }

    fn parse_selector(&mut self) -> Option<Selector> {
        let mut parts = Vec::new();
        let mut had_whitespace_before = false;

        loop {
            let space = self.consume_whitespace_count();
            if space > 0 && !parts.is_empty() {
                had_whitespace_before = true;
            }

            if self.eof() || self.curr_char() == '{' || self.curr_char() == ',' {
                break;
            }

            if self.curr_char() == '>' {
                self.consume_char();
                parts.push(SelectorPart::ChildCombinator);
                had_whitespace_before = false;
                continue;
            }

            if had_whitespace_before {
                parts.push(SelectorPart::DescendantCombinator);
                had_whitespace_before = false;
            }

            if let Some(simple) = self.parse_simple_selector() {
                parts.push(SelectorPart::Simple(simple));
            } else {
                break;
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(Selector { parts })
        }
    }

    fn parse_simple_selector(&mut self) -> Option<SimpleSelector> {
        let mut selector = SimpleSelector::default();
        let mut parsed_any = false;

        if self.curr_char() == '*' {
            self.consume_char();
            selector.universal = true;
            parsed_any = true;
        } else if is_identifier_char(self.curr_char()) {
            selector.tag_name = Some(self.parse_identifier().to_ascii_lowercase());
            parsed_any = true;
        }

        while !self.eof() {
            match self.curr_char() {
                '#' => {
                    self.consume_char();
                    selector.id = Some(self.parse_identifier());
                    parsed_any = true;
                }
                '.' => {
                    self.consume_char();
                    selector.classes.push(self.parse_identifier());
                    parsed_any = true;
                }
                ':' => {
                    // pseudo-class like :hover
                    self.consume_char();
                    let _pseudo = self.parse_identifier();
                    // for now ignore pseudo-classes or could be recorded
                    parsed_any = true;
                }
                _ => break,
            }
        }

        if parsed_any {
            Some(selector)
        } else {
            None
        }
    }

    fn parse_declarations(&mut self) -> Vec<Declaration> {
        let mut declarations = Vec::new();
        loop {
            self.consume_whitespace_and_comments();
            if self.eof() || self.curr_char() == '}' {
                break;
            }
            let (name, value_str) = self.parse_declaration_pair();
            if !name.is_empty() && !value_str.is_empty() {
                self.expand_and_add(&name, &value_str, &mut declarations);
            }
            self.consume_whitespace_and_comments();
            if self.curr_char() == ';' {
                self.consume_char();
            }
        }
        declarations
    }

    fn parse_declaration_pair(&mut self) -> (String, String) {
        let name = self.parse_identifier().to_ascii_lowercase();
        self.consume_whitespace();
        if self.curr_char() != ':' {
            return (String::new(), String::new());
        }
        self.consume_char(); // consume ':'
        self.consume_whitespace();

        let mut value = String::new();
        while !self.eof() && self.curr_char() != ';' && self.curr_char() != '}' {
            value.push(self.consume_char());
        }
        (name, value.trim().to_string())
    }

    fn expand_and_add(&self, name: &str, raw_value: &str, out: &mut Vec<Declaration>) {
        let tokens: Vec<&str> = raw_value.split_whitespace().collect();
        if tokens.is_empty() {
            return;
        }

        // Shorthand for margin & padding: 1, 2, 3, or 4 values
        if name == "margin" || name == "padding" {
            let (top, right, bottom, left) = match tokens.len() {
                1 => (tokens[0], tokens[0], tokens[0], tokens[0]),
                2 => (tokens[0], tokens[1], tokens[0], tokens[1]),
                3 => (tokens[0], tokens[1], tokens[2], tokens[1]),
                _ => (tokens[0], tokens[1], tokens[2], tokens[3]),
            };
            for (side, val) in [("-top", top), ("-right", right), ("-bottom", bottom), ("-left", left)] {
                if let Some(v) = parse_value_str(val) {
                    out.push(Declaration {
                        name: format!("{}{}", name, side),
                        value: v,
                    });
                }
            }
            return;
        }

        // Shorthand for border: e.g. "1px solid #ccc"
        if name == "border" {
            for token in tokens {
                if let Some(v) = parse_value_str(token) {
                    match &v {
                        Value::Length(..) => {
                            for side in ["top", "right", "bottom", "left"] {
                                out.push(Declaration {
                                    name: format!("border-{}-width", side),
                                    value: v.clone(),
                                });
                            }
                        }
                        Value::ColorValue(..) => {
                            for side in ["top", "right", "bottom", "left"] {
                                out.push(Declaration {
                                    name: format!("border-{}-color", side),
                                    value: v.clone(),
                                });
                            }
                        }
                        Value::Keyword(k) if matches!(k.as_str(), "solid" | "dashed" | "dotted" | "none") => {
                            for side in ["top", "right", "bottom", "left"] {
                                out.push(Declaration {
                                    name: format!("border-{}-style", side),
                                    value: v.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            return;
        }

        // Shorthand for background: e.g. "#fff"
        if name == "background" {
            if let Some(v) = parse_value_str(raw_value) {
                out.push(Declaration {
                    name: "background-color".to_string(),
                    value: v,
                });
            }
            return;
        }

        if let Some(v) = parse_value_str(raw_value) {
            out.push(Declaration {
                name: name.to_string(),
                value: v,
            });
        }
    }

    fn parse_identifier(&mut self) -> String {
        self.consume_while(is_identifier_char)
    }

    fn consume_whitespace(&mut self) {
        self.consume_while(char::is_whitespace);
    }

    fn consume_whitespace_count(&mut self) -> usize {
        let start = self.pos;
        self.consume_whitespace();
        self.pos - start
    }

    fn consume_whitespace_and_comments(&mut self) {
        loop {
            self.consume_whitespace();
            if self.starts_with("/*") {
                self.consume_str("/*");
                while !self.eof() && !self.starts_with("*/") {
                    self.consume_char();
                }
                if self.starts_with("*/") {
                    self.consume_str("*/");
                }
            } else {
                break;
            }
        }
    }

    fn starts_with(&self, prefix: &str) -> bool {
        let prefix_chars: Vec<char> = prefix.chars().collect();
        if self.pos + prefix_chars.len() > self.input.len() {
            return false;
        }
        for (i, &c) in prefix_chars.iter().enumerate() {
            if self.input[self.pos + i] != c {
                return false;
            }
        }
        true
    }

    fn curr_char(&self) -> char {
        if self.pos < self.input.len() {
            self.input[self.pos]
        } else {
            '\0'
        }
    }

    fn consume_char(&mut self) -> char {
        let c = self.curr_char();
        if self.pos < self.input.len() {
            self.pos += 1;
        }
        c
    }

    fn consume_str(&mut self, s: &str) {
        for _ in s.chars() {
            self.consume_char();
        }
    }

    fn consume_while<F>(&mut self, test: F) -> String
    where
        F: Fn(char) -> bool,
    {
        let mut result = String::new();
        while !self.eof() && test(self.curr_char()) {
            result.push(self.consume_char());
        }
        result
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }
}

fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

pub fn parse_value_str(s: &str) -> Option<Value> {
    let s = s.trim();

    // Check color: hex or rgb/rgba or named
    if let Some(c) = parse_color(s) {
        return Some(Value::ColorValue(c));
    }

    // Check length with unit
    if let Some(val) = parse_length(s) {
        return Some(val);
    }

    // Check pure number
    if let Ok(num) = s.parse::<f32>() {
        return Some(Value::Number(num));
    }

    // Check string literal: "..." or '...'
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        return Some(Value::StringLiteral(s[1..s.len() - 1].to_string()));
    }

    // Keyword
    Some(Value::Keyword(s.to_ascii_lowercase()))
}

fn parse_length(s: &str) -> Option<Value> {
    let s_lower = s.to_ascii_lowercase();
    let (num_str, unit) = if let Some(n) = s_lower.strip_suffix("px") {
        (n, Unit::Px)
    } else if let Some(n) = s_lower.strip_suffix("em") {
        (n, Unit::Em)
    } else if let Some(n) = s_lower.strip_suffix("rem") {
        (n, Unit::Rem)
    } else if let Some(n) = s_lower.strip_suffix('%') {
        (n, Unit::Percent)
    } else if let Some(n) = s_lower.strip_suffix("pt") {
        (n, Unit::Pt)
    } else {
        return None;
    };

    num_str.trim().parse::<f32>().ok().map(|num| Value::Length(num, unit))
}

pub fn parse_color(s: &str) -> Option<CssColor> {
    let s = s.trim();
    if s.starts_with('#') {
        let hex = &s[1..];
        match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                Some(CssColor::new(r, g, b, 255))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(CssColor::new(r, g, b, 255))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(CssColor::new(r, g, b, a))
            }
            _ => None,
        }
    } else if s.starts_with("rgb(") && s.ends_with(')') {
        let inner = &s[4..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            Some(CssColor::new(r, g, b, 255))
        } else {
            None
        }
    } else if s.starts_with("rgba(") && s.ends_with(')') {
        let inner = &s[5..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 4 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            let a_f = parts[3].parse::<f32>().ok()?;
            let a = (a_f * 255.0).clamp(0.0, 255.0) as u8;
            Some(CssColor::new(r, g, b, a))
        } else {
            None
        }
    } else {
        match s.to_ascii_lowercase().as_str() {
            "transparent" => Some(CssColor::TRANSPARENT),
            "black" => Some(CssColor::new(0, 0, 0, 255)),
            "white" => Some(CssColor::new(255, 255, 255, 255)),
            "red" => Some(CssColor::new(230, 50, 50, 255)),
            "green" => Some(CssColor::new(40, 167, 69, 255)),
            "blue" => Some(CssColor::new(26, 115, 232, 255)),
            "yellow" => Some(CssColor::new(241, 196, 15, 255)),
            "purple" => Some(CssColor::new(142, 68, 173, 255)),
            "orange" => Some(CssColor::new(230, 126, 34, 255)),
            "gray" | "grey" => Some(CssColor::new(128, 128, 128, 255)),
            "darkgray" | "darkgrey" => Some(CssColor::new(60, 64, 67, 255)),
            "lightgray" | "lightgrey" => Some(CssColor::new(220, 220, 220, 255)),
            "navy" => Some(CssColor::new(0, 0, 128, 255)),
            "teal" => Some(CssColor::new(0, 128, 128, 255)),
            "silver" => Some(CssColor::new(192, 192, 192, 255)),
            "cyan" | "aqua" => Some(CssColor::new(0, 255, 255, 255)),
            "magenta" | "fuchsia" => Some(CssColor::new(255, 0, 255, 255)),
            _ => None,
        }
    }
}
