use crate::css::{Unit, Value};
use crate::dom::NodeType;
use crate::style::{Display, StyledNode};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Rect { x, y, width, height }
    }

    pub fn expanded_by(self, edge: EdgeSizes) -> Rect {
        Rect {
            x: self.x - edge.left,
            y: self.y - edge.top,
            width: (self.width + edge.left + edge.right).max(0.0),
            height: (self.height + edge.top + edge.bottom).max(0.0),
        }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeSizes {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dimensions {
    pub content: Rect,
    pub padding: EdgeSizes,
    pub border: EdgeSizes,
    pub margin: EdgeSizes,
}

impl Dimensions {
    pub fn padding_box(&self) -> Rect {
        self.content.expanded_by(self.padding)
    }

    pub fn border_box(&self) -> Rect {
        self.padding_box().expanded_by(self.border)
    }

    pub fn margin_box(&self) -> Rect {
        self.border_box().expanded_by(self.margin)
    }
}

#[derive(Clone, Debug)]
pub enum BoxType<'a> {
    BlockNode(&'a StyledNode<'a>),
    InlineNode(&'a StyledNode<'a>),
    TextFragment {
        text: String,
        styled_node: &'a StyledNode<'a>,
    },
    #[allow(dead_code)]
    AnonymousBlock,
}

#[derive(Clone, Debug)]
pub struct LayoutBox<'a> {
    pub dimensions: Dimensions,
    pub box_type: BoxType<'a>,
    pub children: Vec<LayoutBox<'a>>,
}

impl<'a> LayoutBox<'a> {
    pub fn new(box_type: BoxType<'a>) -> Self {
        LayoutBox {
            dimensions: Dimensions::default(),
            box_type,
            children: Vec::new(),
        }
    }

    pub fn get_styled_node(&self) -> Option<&'a StyledNode<'a>> {
        match &self.box_type {
            BoxType::BlockNode(node) | BoxType::InlineNode(node) => Some(*node),
            BoxType::TextFragment { styled_node, .. } => Some(*styled_node),
            BoxType::AnonymousBlock => None,
        }
    }

    /// Performs standard CSS layout given the containing block dimensions.
    pub fn layout(&mut self, containing_block: Dimensions) {
        match self.box_type {
            BoxType::BlockNode(_) | BoxType::AnonymousBlock => {
                self.layout_block(containing_block);
            }
            BoxType::InlineNode(_) | BoxType::TextFragment { .. } => {
                // Handled during inline flow of parent block
            }
        }
    }

    fn layout_block(&mut self, containing_block: Dimensions) {
        self.calculate_block_width(containing_block);
        self.calculate_block_position(containing_block);
        self.layout_block_children();
        self.calculate_block_height();
    }

    fn calculate_block_width(&mut self, containing_block: Dimensions) {
        let style = self.get_styled_node();

        let auto = Value::Keyword("auto".to_string());
        let zero = Value::Length(0.0, Unit::Px);

        let width = style.and_then(|s| s.value("width")).unwrap_or(&auto);

        let margin_left = style.and_then(|s| s.value("margin-left")).unwrap_or(&zero);
        let margin_right = style.and_then(|s| s.value("margin-right")).unwrap_or(&zero);

        let border_left = style.and_then(|s| s.value("border-left-width")).unwrap_or(&zero);
        let border_right = style.and_then(|s| s.value("border-right-width")).unwrap_or(&zero);

        let padding_left = style.and_then(|s| s.value("padding-left")).unwrap_or(&zero);
        let padding_right = style.and_then(|s| s.value("padding-right")).unwrap_or(&zero);

        let parent_width = containing_block.content.width.max(0.0);

        let parse_len = |v: &Value, base: f32| -> f32 {
            match v {
                Value::Length(px, Unit::Px) => *px,
                Value::Length(pt, Unit::Pt) => *pt * 1.333,
                Value::Length(em, Unit::Em) => *em * 16.0,
                Value::Length(rem, Unit::Rem) => *rem * 16.0,
                Value::Length(pct, Unit::Percent) => base * (*pct / 100.0),
                Value::Length(vh, Unit::Vh) => 600.0 * (*vh / 100.0),
                Value::Length(vw, Unit::Vw) => base * (*vw / 100.0),
                Value::Number(n) => *n,
                _ => 0.0,
            }
        };

        self.dimensions.border.left = parse_len(border_left, parent_width);
        self.dimensions.border.right = parse_len(border_right, parent_width);
        self.dimensions.padding.left = parse_len(padding_left, parent_width);
        self.dimensions.padding.right = parse_len(padding_right, parent_width);

        let total_non_content = self.dimensions.border.left
            + self.dimensions.border.right
            + self.dimensions.padding.left
            + self.dimensions.padding.right;

        let w_is_auto = matches!(width, Value::Keyword(k) if k == "auto");
        let ml_is_auto = matches!(margin_left, Value::Keyword(k) if k == "auto");
        let mr_is_auto = matches!(margin_right, Value::Keyword(k) if k == "auto");

        let specified_w = if w_is_auto { 0.0 } else { parse_len(width, parent_width) };
        let specified_ml = if ml_is_auto { 0.0 } else { parse_len(margin_left, parent_width) };
        let specified_mr = if mr_is_auto { 0.0 } else { parse_len(margin_right, parent_width) };

        let sum = specified_w + total_non_content + specified_ml + specified_mr;

        if !w_is_auto && sum > parent_width {
            // Overconstrained: treat right margin as auto
            self.dimensions.content.width = specified_w;
            self.dimensions.margin.left = specified_ml;
            self.dimensions.margin.right = parent_width - (specified_w + total_non_content + specified_ml);
        } else {
            let underflow = parent_width - sum;
            match (w_is_auto, ml_is_auto, mr_is_auto) {
                (false, false, false) => {
                    self.dimensions.content.width = specified_w;
                    self.dimensions.margin.left = specified_ml;
                    self.dimensions.margin.right = specified_mr + underflow;
                }
                (false, true, true) => {
                    // Margin auto centering (e.g. margin: 0 auto)
                    self.dimensions.content.width = specified_w;
                    self.dimensions.margin.left = (underflow / 2.0).max(0.0);
                    self.dimensions.margin.right = (underflow / 2.0).max(0.0);
                }
                (false, true, false) => {
                    self.dimensions.content.width = specified_w;
                    self.dimensions.margin.left = underflow;
                    self.dimensions.margin.right = specified_mr;
                }
                (false, false, true) => {
                    self.dimensions.content.width = specified_w;
                    self.dimensions.margin.left = specified_ml;
                    self.dimensions.margin.right = underflow;
                }
                (true, _, _) => {
                    self.dimensions.margin.left = specified_ml;
                    self.dimensions.margin.right = specified_mr;
                    self.dimensions.content.width = (underflow).max(0.0);
                }
            }
        }
    }

    fn calculate_block_position(&mut self, containing_block: Dimensions) {
        let style = self.get_styled_node();
        let zero = Value::Length(0.0, Unit::Px);

        let margin_top = style.and_then(|s| s.value("margin-top")).unwrap_or(&zero);
        let margin_bottom = style.and_then(|s| s.value("margin-bottom")).unwrap_or(&zero);

        let border_top = style.and_then(|s| s.value("border-top-width")).unwrap_or(&zero);
        let border_bottom = style.and_then(|s| s.value("border-bottom-width")).unwrap_or(&zero);

        let padding_top = style.and_then(|s| s.value("padding-top")).unwrap_or(&zero);
        let padding_bottom = style.and_then(|s| s.value("padding-bottom")).unwrap_or(&zero);

        let parse_len = |v: &Value| -> f32 {
            match v {
                Value::Length(px, Unit::Px) => *px,
                Value::Length(pt, Unit::Pt) => *pt * 1.333,
                Value::Length(em, Unit::Em) => *em * 16.0,
                Value::Length(rem, Unit::Rem) => *rem * 16.0,
                Value::Length(vh, Unit::Vh) => 600.0 * (*vh / 100.0),
                Value::Length(pct, Unit::Percent) => 600.0 * (*pct / 100.0),
                Value::Number(n) => *n,
                _ => 0.0,
            }
        };

        self.dimensions.margin.top = parse_len(margin_top);
        self.dimensions.margin.bottom = parse_len(margin_bottom);
        self.dimensions.border.top = parse_len(border_top);
        self.dimensions.border.bottom = parse_len(border_bottom);
        self.dimensions.padding.top = parse_len(padding_top);
        self.dimensions.padding.bottom = parse_len(padding_bottom);

        self.dimensions.content.x = containing_block.content.x
            + self.dimensions.margin.left
            + self.dimensions.border.left
            + self.dimensions.padding.left;

        self.dimensions.content.y = containing_block.content.height
            + containing_block.content.y
            + self.dimensions.margin.top
            + self.dimensions.border.top
            + self.dimensions.padding.top;
    }

    fn layout_block_children(&mut self) {
        let has_inline_children = self.children.iter().any(|c| match c.box_type {
            BoxType::InlineNode(_) | BoxType::TextFragment { .. } => true,
            _ => false,
        });

        if has_inline_children {
            self.layout_inline_children();
        } else {
            // Standard vertical block stack
            let mut y_offset = 0.0;
            for child in &mut self.children {
                child.layout(self.dimensions);
                y_offset += child.dimensions.margin_box().height;
                self.dimensions.content.height = y_offset;
            }
        }
    }

    fn layout_inline_children(&mut self) {
        let max_width = self.dimensions.content.width.max(10.0);
        let mut cur_x = 0.0;
        let mut cur_y = 0.0;
        let mut line_height: f32 = 20.0;

        for child in &mut self.children {
            let font_size = child
                .get_styled_node()
                .map(|s| s.font_size(None))
                .unwrap_or(16.0);
            let item_line_height = font_size * 1.35;
            line_height = line_height.max(item_line_height);

            match &mut child.box_type {
                BoxType::TextFragment { text, .. } => {
                    let text_w = estimate_text_width(text, font_size);
                    if cur_x + text_w > max_width && cur_x > 0.0 {
                        // Wrap to new line
                        cur_x = 0.0;
                        cur_y += line_height;
                    }
                    child.dimensions.content.x = self.dimensions.content.x + cur_x;
                    child.dimensions.content.y = self.dimensions.content.y + cur_y;
                    child.dimensions.content.width = text_w;
                    child.dimensions.content.height = item_line_height;
                    cur_x += text_w;
                }
                BoxType::InlineNode(node) => {
                    // Check button or input or inline box
                    let is_button_or_input = node.node.tag_name() == Some("button")
                        || node.node.tag_name() == Some("input");

                    let pad_x = if is_button_or_input { 16.0 } else { 0.0 };
                    let pad_y = if is_button_or_input { 8.0 } else { 0.0 };

                    let content_w = estimate_text_width(&node.node.inner_text(), font_size) + pad_x;
                    let content_h = item_line_height + pad_y;

                    if cur_x + content_w > max_width && cur_x > 0.0 {
                        cur_x = 0.0;
                        cur_y += line_height;
                    }

                    child.dimensions.content.x = self.dimensions.content.x + cur_x;
                    child.dimensions.content.y = self.dimensions.content.y + cur_y;
                    child.dimensions.content.width = content_w;
                    child.dimensions.content.height = content_h;
                    cur_x += content_w + 6.0;
                }
                _ => {
                    child.layout(self.dimensions);
                    cur_y += child.dimensions.margin_box().height;
                    cur_x = 0.0;
                }
            }
        }

        self.dimensions.content.height = cur_y + line_height;
    }

    fn calculate_block_height(&mut self) {
        if let Some(style) = self.get_styled_node() {
            if let Some(h) = style.value("height") {
                if let Value::Length(px, Unit::Px) = h {
                    self.dimensions.content.height = *px;
                }
            }
        }
    }
}

/// Builds a tree of LayoutBoxes from the StyledNode tree.
pub fn build_layout_tree<'a>(root: &'a StyledNode<'a>) -> Option<LayoutBox<'a>> {
    if root.display() == Display::None {
        return None;
    }

    let mut root_box = LayoutBox::new(match root.display() {
        Display::Block => BoxType::BlockNode(root),
        Display::Inline | Display::InlineBlock => BoxType::InlineNode(root),
        Display::None => unreachable!(),
    });

    for child in &root.children {
        if child.display() == Display::None {
            continue;
        }

        match &child.node.node_type {
            NodeType::Text(text) => {
                let cleaned = clean_text_whitespace(text);
                if !cleaned.is_empty() {
                    // Split into words so wrapping works naturally
                    let words = split_text_into_words(&cleaned);
                    for word in words {
                        root_box.children.push(LayoutBox::new(BoxType::TextFragment {
                            text: word,
                            styled_node: child,
                        }));
                    }
                }
            }
            NodeType::Element(_) => {
                if let Some(child_box) = build_layout_tree(child) {
                    root_box.children.push(child_box);
                }
            }
            NodeType::Comment(_) => {}
        }
    }

    Some(root_box)
}

fn clean_text_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut prev_whitespace = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_whitespace {
                result.push(' ');
                prev_whitespace = true;
            }
        } else {
            result.push(c);
            prev_whitespace = false;
        }
    }
    result
}

fn split_text_into_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for c in s.chars() {
        if c.is_whitespace() {
            if !current.is_empty() {
                current.push(' ');
                words.push(current);
                current = String::new();
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Estimates text width using realistic proportional font metrics.
pub fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let mut width = 0.0;
    for c in text.chars() {
        let factor = match c {
            ' ' => 0.32,
            'i' | 'l' | 'j' | '!' | '.' | ',' | ':' | ';' | '\'' | '|' => 0.28,
            'f' | 'r' | 't' | '(' | ')' | '[' | ']' => 0.35,
            'm' | 'w' | 'M' | 'W' => 0.85,
            'A'..='Z' => 0.68,
            _ => 0.53,
        };
        width += factor * font_size;
    }
    width.max(2.0)
}
