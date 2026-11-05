use crate::dom::{ElementData, Node, NodeType};
use std::collections::HashMap;

/// HTML Parser that converts raw HTML text into a DOM tree.
pub struct HtmlParser {
    pos: usize,
    input: Vec<char>,
}

impl HtmlParser {
    pub fn parse(source: &str) -> Node {
        let chars: Vec<char> = source.chars().collect();
        let mut parser = HtmlParser {
            pos: 0,
            input: chars,
        };
        let nodes = parser.parse_nodes();

        // If the document has a single root node (like <html>), return it,
        // otherwise wrap in a synthetic <html> element.
        if nodes.len() == 1 && nodes[0].tag_name() == Some("html") {
            nodes.into_iter().next().unwrap()
        } else {
            Node::new_element("html".to_string(), HashMap::new(), nodes)
        }
    }

    fn parse_nodes(&mut self) -> Vec<Node> {
        let mut nodes = Vec::new();
        loop {
            self.consume_whitespace_and_comments();
            if self.eof() || self.starts_with("</") {
                break;
            }
            if let Some(node) = self.parse_node() {
                nodes.push(node);
            }
        }
        nodes
    }

    fn parse_node(&mut self) -> Option<Node> {
        if self.starts_with("<!--") {
            return Some(self.parse_comment());
        }

        if self.starts_with("<!") || self.starts_with("<?") {
            self.consume_declaration();
            return None;
        }

        if self.starts_with("<") {
            Some(self.parse_element())
        } else {
            Some(self.parse_text())
        }
    }

    fn parse_comment(&mut self) -> Node {
        self.consume_str("<!--");
        let start = self.pos;
        while !self.eof() && !self.starts_with("-->") {
            self.consume_char();
        }
        let comment_text: String = self.input[start..self.pos].iter().collect();
        if self.starts_with("-->") {
            self.consume_str("-->");
        }
        Node::new_comment(comment_text)
    }

    fn consume_declaration(&mut self) {
        while !self.eof() && self.curr_char() != '>' {
            self.consume_char();
        }
        if !self.eof() && self.curr_char() == '>' {
            self.consume_char();
        }
    }

    fn parse_text(&mut self) -> Node {
        let mut text = String::new();
        while !self.eof() && self.curr_char() != '<' {
            if self.curr_char() == '&' {
                text.push_str(&self.parse_entity());
            } else {
                text.push(self.consume_char());
            }
        }
        Node::new_text(text)
    }

    fn parse_entity(&mut self) -> String {
        self.consume_char(); // consume '&'
        let mut entity_name = String::new();
        while !self.eof() && self.curr_char() != ';' && !self.curr_char().is_whitespace() && entity_name.len() < 10 {
            entity_name.push(self.consume_char());
        }
        if !self.eof() && self.curr_char() == ';' {
            self.consume_char(); // consume ';'
        }

        Self::resolve_entity_name(&entity_name)
    }

    pub fn decode_string_entities(s: &str) -> String {
        if !s.contains('&') {
            return s.to_string();
        }
        let mut result = String::with_capacity(s.len());
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '&' {
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && chars[end] != ';' && !chars[end].is_whitespace() && (end - start) < 10 {
                    end += 1;
                }
                if end < chars.len() && chars[end] == ';' {
                    let entity_name: String = chars[start..end].iter().collect();
                    let decoded = Self::resolve_entity_name(&entity_name);
                    result.push_str(&decoded);
                    i = end + 1;
                    continue;
                }
            }
            result.push(chars[i]);
            i += 1;
        }
        result
    }

    fn resolve_entity_name(entity_name: &str) -> String {
        match entity_name {
            "amp" => "&".to_string(),
            "lt" => "<".to_string(),
            "gt" => ">".to_string(),
            "quot" => "\"".to_string(),
            "apos" => "'".to_string(),
            "nbsp" => " ".to_string(),
            "cent" => "¢".to_string(),
            "pound" => "£".to_string(),
            "yen" => "¥".to_string(),
            "euro" => "€".to_string(),
            "copy" => "©".to_string(),
            "reg" => "®".to_string(),
            "ndash" => "–".to_string(),
            "mdash" => "—".to_string(),
            "bull" => "•".to_string(),
            "hellip" => "…".to_string(),
            "rsquo" => "’".to_string(),
            "lsquo" => "‘".to_string(),
            "rdquo" => "”".to_string(),
            "ldquo" => "“".to_string(),
            "times" => "×".to_string(),
            "divide" => "÷".to_string(),
            "deg" => "°".to_string(),
            _ => {
                if let Some(stripped) = entity_name.strip_prefix('#') {
                    if let Some(hex) = stripped.strip_prefix('x') {
                        if let Ok(code) = u32::from_str_radix(hex, 16) {
                            if let Some(c) = char::from_u32(code) {
                                return c.to_string();
                            }
                        }
                    } else if let Ok(code) = stripped.parse::<u32>() {
                        if let Some(c) = char::from_u32(code) {
                            return c.to_string();
                        }
                    }
                }
                format!("&{};", entity_name)
            }
        }
    }

    fn parse_element(&mut self) -> Node {
        self.consume_char(); // consume '<'
        let tag_name = self.parse_tag_name();
        let attributes = self.parse_attributes();

        let is_self_closing = if self.starts_with("/>") {
            self.consume_str("/>");
            true
        } else if self.curr_char() == '>' {
            self.consume_char();
            false
        } else {
            false
        };

        let is_void_tag = is_void_element(&tag_name);

        if is_self_closing || is_void_tag {
            return Node {
                children: Vec::new(),
                node_type: NodeType::Element(ElementData {
                    tag_name,
                    attributes,
                }),
            };
        }

        // Special raw text elements: <style>, <script>, <title>
        if tag_name == "style" || tag_name == "script" || tag_name == "title" {
            let content = self.parse_raw_content(&tag_name);
            let child = Node::new_text(content);
            return Node {
                children: vec![child],
                node_type: NodeType::Element(ElementData {
                    tag_name,
                    attributes,
                }),
            };
        }

        let children = self.parse_nodes();

        // Consume closing tag </tag_name>
        if self.starts_with("</") {
            let save_pos = self.pos;
            self.consume_str("</");
            let close_tag = self.parse_tag_name();
            self.consume_whitespace();
            if self.curr_char() == '>' {
                self.consume_char();
            }
            if close_tag != tag_name {
                self.pos = save_pos;
            }
        }

        Node {
            children,
            node_type: NodeType::Element(ElementData {
                tag_name,
                attributes,
            }),
        }
    }

    fn parse_raw_content(&mut self, tag_name: &str) -> String {
        let start = self.pos;
        while !self.eof() {
            if self.starts_with("</") {
                let check_pos = self.pos + 2;
                let tag_len = tag_name.len();
                if check_pos + tag_len <= self.input.len() {
                    let maybe_tag: String = self.input[check_pos..check_pos + tag_len].iter().collect();
                    if maybe_tag.eq_ignore_ascii_case(tag_name) {
                        let mut after_pos = check_pos + tag_len;
                        while after_pos < self.input.len() && self.input[after_pos].is_whitespace() {
                            after_pos += 1;
                        }
                        if after_pos < self.input.len() && self.input[after_pos] == '>' {
                            let content: String = self.input[start..self.pos].iter().collect();
                            self.pos = after_pos + 1;
                            return content;
                        }
                    }
                }
            }
            self.consume_char();
        }
        self.input[start..self.pos].iter().collect()
    }

    fn parse_tag_name(&mut self) -> String {
        self.consume_while(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            .to_ascii_lowercase()
    }

    fn parse_attributes(&mut self) -> HashMap<String, String> {
        let mut attributes = HashMap::new();
        loop {
            self.consume_whitespace();
            if self.eof() || self.curr_char() == '>' || self.starts_with("/>") {
                break;
            }
            let (name, val) = self.parse_attribute();
            if !name.is_empty() {
                attributes.insert(name.to_ascii_lowercase(), val);
            }
        }
        attributes
    }

    fn parse_attribute(&mut self) -> (String, String) {
        let name = self.consume_while(|c| {
            !c.is_whitespace() && c != '=' && c != '>' && c != '/'
        });
        self.consume_whitespace();
        let value = if !self.eof() && self.curr_char() == '=' {
            self.consume_char();
            self.consume_whitespace();
            self.parse_attribute_value()
        } else {
            String::new()
        };
        (name, value)
    }

    fn parse_attribute_value(&mut self) -> String {
        if self.eof() {
            return String::new();
        }
        let quote = self.curr_char();
        let raw = if quote == '"' || quote == '\'' {
            self.consume_char();
            let val = self.consume_while(|c| c != quote);
            if !self.eof() && self.curr_char() == quote {
                self.consume_char();
            }
            val
        } else {
            self.consume_while(|c| !c.is_whitespace() && c != '>')
        };
        Self::decode_string_entities(&raw)
    }

    fn consume_whitespace(&mut self) {
        self.consume_while(char::is_whitespace);
    }

    fn consume_whitespace_and_comments(&mut self) {
        loop {
            self.consume_whitespace();
            if self.starts_with("<!--") {
                self.parse_comment();
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
            if self.input[self.pos + i].to_ascii_lowercase() != c.to_ascii_lowercase() {
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

/// Checks if a tag is void (must not have closing tag or children in standard HTML).
fn is_void_element(tag: &str) -> bool {
    matches!(
        tag,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}
