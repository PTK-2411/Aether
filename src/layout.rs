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
    ImageNode {
        src: String,
        alt: String,
        styled_node: &'a StyledNode<'a>,
    },
    TextFragment {
        text: String,
        styled_node: &'a StyledNode<'a>,
    },
    #[allow(dead_code)]
    AnonymousBlock,
}

fn eval_calc(expr: &str, base: f32) -> f32 {
    let expr = expr.trim();
    if let Some(pos) = expr.rfind(" - ") {
        let left = eval_calc(&expr[..pos], base);
        let right = eval_calc(&expr[pos + 3..], base);
        return left - right;
    }
    if let Some(pos) = expr.rfind(" + ") {
        let left = eval_calc(&expr[..pos], base);
        let right = eval_calc(&expr[pos + 3..], base);
        return left + right;
    }
    if let Some(pos) = expr.rfind(" * ") {
        let left = eval_calc(&expr[..pos], base);
        let right = expr[pos + 3..].trim().parse::<f32>().unwrap_or(1.0);
        return left * right;
    }
    if let Some(pos) = expr.rfind(" / ") {
        let left = eval_calc(&expr[..pos], base);
        let right = expr[pos + 3..].trim().parse::<f32>().unwrap_or(1.0);
        if right != 0.0 {
            return left / right;
        }
        return left;
    }

    if let Some(val) = crate::css::parse_value_str(expr) {
        parse_len_value(&val, base)
    } else {
        0.0
    }
}

pub fn parse_len_value(v: &Value, base: f32) -> f32 {
    match v {
        Value::Length(px, Unit::Px) => *px,
        Value::Length(pt, Unit::Pt) => *pt * 1.333,
        Value::Length(em, Unit::Em) => *em * 16.0,
        Value::Length(rem, Unit::Rem) => *rem * 16.0,
        Value::Length(pct, Unit::Percent) => base * (*pct / 100.0),
        Value::Length(vh, Unit::Vh) => 600.0 * (*vh / 100.0),
        Value::Length(vw, Unit::Vw) => base * (*vw / 100.0),
        Value::Number(n) => *n,
        Value::Keyword(k) if k.starts_with("calc(") && k.ends_with(')') => {
            eval_calc(&k[5..k.len() - 1], base)
        }
        _ => 0.0,
    }
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
            BoxType::BlockNode(node) | BoxType::InlineNode(node) | BoxType::ImageNode { styled_node: node, .. } => Some(*node),
            BoxType::TextFragment { styled_node, .. } => Some(*styled_node),
            BoxType::AnonymousBlock => None,
        }
    }

    /// Recursively shifts this box and all its descendants by (dx, dy).
    pub fn translate(&mut self, dx: f32, dy: f32) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        self.dimensions.content.x += dx;
        self.dimensions.content.y += dy;
        for child in &mut self.children {
            child.translate(dx, dy);
        }
    }

    /// Performs standard CSS layout given the containing block dimensions.
    pub fn layout(&mut self, containing_block: Dimensions) {
        match self.box_type {
            BoxType::BlockNode(_) | BoxType::AnonymousBlock => {
                let display = self.get_styled_node().map(|s| s.display()).unwrap_or(crate::style::Display::Block);
                match display {
                    crate::style::Display::Flex => self.layout_flex(containing_block),
                    crate::style::Display::Grid => self.layout_grid(containing_block),
                    _ => self.layout_block(containing_block),
                }
            }
            BoxType::InlineNode(_) | BoxType::ImageNode { .. } | BoxType::TextFragment { .. } => {
                // Handled during inline flow of parent block
            }
        }
    }

    fn layout_block(&mut self, containing_block: Dimensions) {
        self.calculate_block_width(containing_block);
        self.calculate_block_position(containing_block);
        self.layout_block_children();
        self.calculate_block_height();
        self.apply_relative_and_absolute_positioning(containing_block);
    }

    fn layout_grid(&mut self, containing_block: Dimensions) {
        self.calculate_block_width(containing_block);
        self.calculate_block_position(containing_block);

        let parent_w = self.dimensions.content.width.max(10.0);
        let style = self.get_styled_node();

        let cols_spec = style.and_then(|s| s.value("grid-template-columns")).map(|v| match v {
            Value::Keyword(k) => k.as_str(),
            Value::StringLiteral(s) => s.as_str(),
            _ => "",
        }).unwrap_or("");

        let gap = style.and_then(|s| s.value("gap").or_else(|| s.value("column-gap"))).and_then(|v| match v {
            Value::Length(px, Unit::Px) => Some(*px),
            _ => None,
        }).unwrap_or(0.0);

        let column_widths = compute_grid_column_widths(cols_spec, parent_w, self.children.len(), gap);
        let num_cols = column_widths.len().max(1);

        let mut row_y = 0.0;
        let mut row_max_h: f32 = 0.0;

        let mut cell_idx = 0;
        for child in &mut self.children {
            let is_out_of_flow = child
                .get_styled_node()
                .and_then(|s| s.value("position"))
                .map_or(false, |v| matches!(v, Value::Keyword(k) if k == "absolute" || k == "fixed"));

            if is_out_of_flow {
                let cb = Dimensions {
                    content: Rect::new(self.dimensions.content.x, self.dimensions.content.y, parent_w, 0.0),
                    padding: EdgeSizes::default(),
                    border: EdgeSizes::default(),
                    margin: EdgeSizes::default(),
                };
                child.layout(cb);
                continue;
            }

            let col_idx = cell_idx % num_cols;
            if col_idx == 0 && cell_idx > 0 {
                row_y += row_max_h + gap;
                row_max_h = 0.0;
            }

            let col_w = column_widths[col_idx];
            let col_x: f32 = column_widths[..col_idx].iter().sum::<f32>() + (col_idx as f32 * gap);

            let cb = Dimensions {
                content: Rect::new(self.dimensions.content.x + col_x, self.dimensions.content.y + row_y, col_w, 0.0),
                padding: EdgeSizes::default(),
                border: EdgeSizes::default(),
                margin: EdgeSizes::default(),
            };
            child.layout(cb);
            row_max_h = row_max_h.max(child.dimensions.margin_box().height);
            cell_idx += 1;
        }

        self.dimensions.content.height = row_y + row_max_h;
        self.calculate_block_height();
        self.apply_relative_and_absolute_positioning(containing_block);
    }

    fn layout_flex(&mut self, containing_block: Dimensions) {
        self.calculate_block_width(containing_block);
        self.calculate_block_position(containing_block);

        let style = self.get_styled_node();
        let direction = style.and_then(|s| s.value("flex-direction")).map(|v| match v {
            Value::Keyword(k) => k.as_str(),
            _ => "row",
        }).unwrap_or("row");

        let gap = style.and_then(|s| s.value("gap").or_else(|| s.value("column-gap"))).and_then(|v| match v {
            Value::Length(px, Unit::Px) => Some(*px),
            _ => None,
        }).unwrap_or(0.0);

        if direction == "column" {
            let mut y_offset = 0.0;
            let mut flow_idx = 0;
            for child in &mut self.children {
                let is_out_of_flow = child
                    .get_styled_node()
                    .and_then(|s| s.value("position"))
                    .map_or(false, |v| matches!(v, Value::Keyword(k) if k == "absolute" || k == "fixed"));

                if is_out_of_flow {
                    let mut cb = self.dimensions;
                    cb.content.height = 0.0;
                    child.layout(cb);
                    continue;
                }

                if flow_idx > 0 {
                    y_offset += gap;
                }
                let mut cb = self.dimensions;
                cb.content.height = y_offset;
                child.layout(cb);
                y_offset += child.dimensions.margin_box().height;
                self.dimensions.content.height = y_offset;
                flow_idx += 1;
            }
        } else {
            let parent_w = self.dimensions.content.width.max(10.0);
            let mut cur_x = 0.0;
            let mut max_h: f32 = 0.0;
            let total_children = self.children.len().max(1) as f32;
            let total_gap = (self.children.len().saturating_sub(1) as f32) * gap;
            let usable_w = (parent_w - total_gap).max(10.0);
            let mut flow_idx = 0;

            for child in &mut self.children {
                let is_out_of_flow = child
                    .get_styled_node()
                    .and_then(|s| s.value("position"))
                    .map_or(false, |v| matches!(v, Value::Keyword(k) if k == "absolute" || k == "fixed"));

                if is_out_of_flow {
                    let cb = Dimensions {
                        content: Rect::new(self.dimensions.content.x, self.dimensions.content.y, parent_w, 0.0),
                        padding: EdgeSizes::default(),
                        border: EdgeSizes::default(),
                        margin: EdgeSizes::default(),
                    };
                    child.layout(cb);
                    continue;
                }

                if flow_idx > 0 {
                    cur_x += gap;
                }
                let item_w = child.get_styled_node().and_then(|s| s.value("width")).and_then(|w| match w {
                    Value::Length(px, Unit::Px) => Some(*px),
                    Value::Length(pct, Unit::Percent) => Some(usable_w * (*pct / 100.0)),
                    _ => None,
                });

                let available_w = item_w.unwrap_or_else(|| {
                    let remaining = (usable_w - cur_x).max(20.0);
                    remaining.min(usable_w / total_children)
                });

                let cb = Dimensions {
                    content: Rect::new(self.dimensions.content.x + cur_x, self.dimensions.content.y, available_w, 0.0),
                    padding: EdgeSizes::default(),
                    border: EdgeSizes::default(),
                    margin: EdgeSizes::default(),
                };
                child.layout(cb);
                let w = child.dimensions.margin_box().width.max(available_w);
                let h = child.dimensions.margin_box().height;
                cur_x += w;
                max_h = max_h.max(h);
                flow_idx += 1;
            }

            self.dimensions.content.height = max_h;
        }

        self.calculate_block_height();
        self.apply_relative_and_absolute_positioning(containing_block);
    }

    fn apply_relative_and_absolute_positioning(&mut self, containing_block: Dimensions) {
        if let Some(style) = self.get_styled_node() {
            if let Some(Value::Keyword(pos)) = style.value("position") {
                let parent_w = containing_block.content.width.max(0.0);
                let parent_h = if containing_block.content.height > 0.0 {
                    containing_block.content.height
                } else {
                    600.0
                };

                let old_x = self.dimensions.content.x;
                let old_y = self.dimensions.content.y;

                match pos.as_str() {
                    "relative" => {
                        let top = style.value("top").map(|v| parse_len_value(v, parent_h)).unwrap_or_else(|| {
                            style.value("bottom").map(|v| -parse_len_value(v, parent_h)).unwrap_or(0.0)
                        });
                        let left = style.value("left").map(|v| parse_len_value(v, parent_w)).unwrap_or_else(|| {
                            style.value("right").map(|v| -parse_len_value(v, parent_w)).unwrap_or(0.0)
                        });
                        self.dimensions.content.x += left;
                        self.dimensions.content.y += top;
                    }
                    "absolute" | "fixed" => {
                        if let Some(top_val) = style.value("top") {
                            self.dimensions.content.y = containing_block.content.y + parse_len_value(top_val, parent_h);
                        } else if let Some(bot_val) = style.value("bottom") {
                            let specified_b = parse_len_value(bot_val, parent_h);
                            self.dimensions.content.y = containing_block.content.y + parent_h - specified_b
                                - self.dimensions.content.height
                                - self.dimensions.padding.bottom
                                - self.dimensions.border.bottom
                                - self.dimensions.margin.bottom;
                        }
                        if let Some(left_val) = style.value("left") {
                            self.dimensions.content.x = containing_block.content.x + parse_len_value(left_val, parent_w);
                        } else if let Some(right_val) = style.value("right") {
                            let specified_r = parse_len_value(right_val, parent_w);
                            self.dimensions.content.x = containing_block.content.x + parent_w - specified_r
                                - self.dimensions.content.width
                                - self.dimensions.padding.right
                                - self.dimensions.border.right
                                - self.dimensions.margin.right;
                        }
                    }
                    _ => {}
                }

                let dx = self.dimensions.content.x - old_x;
                let dy = self.dimensions.content.y - old_y;
                if dx != 0.0 || dy != 0.0 {
                    for child in &mut self.children {
                        child.translate(dx, dy);
                    }
                }
            }
        }
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

        self.dimensions.border.left = parse_len_value(border_left, parent_width);
        self.dimensions.border.right = parse_len_value(border_right, parent_width);
        self.dimensions.padding.left = parse_len_value(padding_left, parent_width);
        self.dimensions.padding.right = parse_len_value(padding_right, parent_width);

        let total_non_content = self.dimensions.border.left
            + self.dimensions.border.right
            + self.dimensions.padding.left
            + self.dimensions.padding.right;

        let is_border_box = style.and_then(|s| s.value("box-sizing")).map_or(false, |v| match v {
            Value::Keyword(k) => k == "border-box",
            _ => false,
        });

        let w_is_auto = matches!(width, Value::Keyword(k) if k == "auto");
        let ml_is_auto = matches!(margin_left, Value::Keyword(k) if k == "auto");
        let mr_is_auto = matches!(margin_right, Value::Keyword(k) if k == "auto");

        let mut specified_w = if w_is_auto { 0.0 } else { parse_len_value(width, parent_width) };
        if is_border_box && !w_is_auto {
            specified_w = (specified_w - total_non_content).max(0.0);
        }

        let specified_ml = if ml_is_auto { 0.0 } else { parse_len_value(margin_left, parent_width) };
        let specified_mr = if mr_is_auto { 0.0 } else { parse_len_value(margin_right, parent_width) };

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

        if let Some(mw) = style.and_then(|s| s.value("max-width")) {
            let max_w = parse_len_value(mw, parent_width);
            if max_w > 0.0 {
                self.dimensions.content.width = self.dimensions.content.width.min(max_w);
            }
        }
        if let Some(mw) = style.and_then(|s| s.value("min-width")) {
            let min_w = parse_len_value(mw, parent_width);
            if min_w > 0.0 {
                self.dimensions.content.width = self.dimensions.content.width.max(min_w);
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

        self.dimensions.margin.top = parse_len_value(margin_top, 600.0);
        self.dimensions.margin.bottom = parse_len_value(margin_bottom, 600.0);
        self.dimensions.border.top = parse_len_value(border_top, 600.0);
        self.dimensions.border.bottom = parse_len_value(border_bottom, 600.0);
        self.dimensions.padding.top = parse_len_value(padding_top, 600.0);
        self.dimensions.padding.bottom = parse_len_value(padding_bottom, 600.0);

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
            BoxType::InlineNode(_) | BoxType::TextFragment { .. } | BoxType::ImageNode { .. } => true,
            _ => false,
        });

        if has_inline_children {
            self.layout_inline_children();
        } else {
            // Standard vertical block stack
            let mut y_offset = 0.0;
            for child in &mut self.children {
                let mut cb = self.dimensions;
                cb.content.height = y_offset;
                child.layout(cb);
                let is_out_of_flow = child
                    .get_styled_node()
                    .and_then(|s| s.value("position"))
                    .map_or(false, |v| matches!(v, Value::Keyword(k) if k == "absolute" || k == "fixed"));
                if !is_out_of_flow {
                    y_offset += child.dimensions.margin_box().height;
                    self.dimensions.content.height = y_offset;
                }
            }
        }
    }

    fn layout_inline_item(
        item: &mut LayoutBox<'a>,
        container_content: Rect,
        max_width: f32,
        cur_x: &mut f32,
        cur_y: &mut f32,
        line_height: &mut f32,
    ) {
        let is_out_of_flow = item
            .get_styled_node()
            .and_then(|s| s.value("position"))
            .map_or(false, |v| matches!(v, Value::Keyword(k) if k == "absolute" || k == "fixed"));

        if is_out_of_flow {
            let cb = Dimensions {
                content: Rect::new(container_content.x, container_content.y, max_width, 0.0),
                padding: EdgeSizes::default(),
                border: EdgeSizes::default(),
                margin: EdgeSizes::default(),
            };
            item.layout(cb);
            return;
        }
        let font_size = item.get_styled_node().map(|s| s.font_size(None)).unwrap_or(16.0);
        let item_line_height = item
            .get_styled_node()
            .map(|s| s.line_height(font_size, None))
            .unwrap_or(font_size * 1.35);
        *line_height = line_height.max(item_line_height);

        match &mut item.box_type {
            BoxType::TextFragment { text, .. } => {
                let text_w = estimate_text_width(text, font_size);
                let space_gap = (font_size * 0.38).max(5.0);

                if *cur_x + text_w > max_width && *cur_x > 0.0 {
                    *cur_x = 0.0;
                    *cur_y += *line_height;
                    *line_height = item_line_height;
                }

                item.dimensions.content.x = container_content.x + *cur_x;
                item.dimensions.content.y = container_content.y + *cur_y;
                item.dimensions.content.width = text_w;
                item.dimensions.content.height = item_line_height;
                *cur_x += text_w + space_gap;
            }
            BoxType::ImageNode { styled_node, .. } => {
                let specified_w = styled_node.node.get_attribute("width").and_then(|w| w.parse::<f32>().ok());
                let specified_h = styled_node.node.get_attribute("height").and_then(|h| h.parse::<f32>().ok());

                let css_w = styled_node.value("width").map(|w| parse_len_value(w, max_width));
                let css_h = styled_node.value("height").map(|h| parse_len_value(h, 600.0));

                let img_w = css_w.or(specified_w).unwrap_or(240.0).min(max_width);
                let img_h = css_h.or(specified_h).unwrap_or(160.0);

                if *cur_x + img_w > max_width && *cur_x > 0.0 {
                    *cur_x = 0.0;
                    *cur_y += *line_height;
                    *line_height = img_h;
                }

                item.dimensions.content.x = container_content.x + *cur_x;
                item.dimensions.content.y = container_content.y + *cur_y;
                item.dimensions.content.width = img_w;
                item.dimensions.content.height = img_h;
                *line_height = line_height.max(img_h);
                *cur_x += img_w + 10.0;
            }
            BoxType::InlineNode(node) => {
                let tag = node.node.tag_name().unwrap_or("");
                let is_button_or_input = tag == "button" || tag == "input";

                if is_button_or_input {
                    let pad_x = 16.0;
                    let pad_y = 8.0;
                    let content_w = estimate_text_width(&node.node.inner_text(), font_size) + pad_x;
                    let content_h = item_line_height + pad_y;

                    if *cur_x + content_w > max_width && *cur_x > 0.0 {
                        *cur_x = 0.0;
                        *cur_y += *line_height;
                        *line_height = content_h;
                    }

                    item.dimensions.content.x = container_content.x + *cur_x;
                    item.dimensions.content.y = container_content.y + *cur_y;
                    item.dimensions.content.width = content_w;
                    item.dimensions.content.height = content_h;
                    *cur_x += content_w + 6.0;
                    *line_height = line_height.max(content_h);
                } else {
                    let start_x = *cur_x;
                    let start_y = *cur_y;

                    for sub_child in &mut item.children {
                        Self::layout_inline_item(
                            sub_child,
                            container_content,
                            max_width,
                            cur_x,
                            cur_y,
                            line_height,
                        );
                    }

                    item.dimensions.content.x = container_content.x + start_x;
                    item.dimensions.content.y = container_content.y + start_y;
                    let measured_w = (*cur_x - start_x).max(0.0);
                    let measured_h = (*cur_y - start_y + *line_height).max(item_line_height);

                    let specified_w = item.get_styled_node().and_then(|s| s.value("width")).map(|w| parse_len_value(w, max_width));
                    let specified_h = item.get_styled_node().and_then(|s| s.value("height")).map(|h| parse_len_value(h, 600.0));

                    item.dimensions.content.width = specified_w.unwrap_or(measured_w);
                    item.dimensions.content.height = specified_h.unwrap_or(measured_h);
                }
            }
            BoxType::BlockNode(_) | BoxType::AnonymousBlock => {
                if *cur_x > 0.0 {
                    *cur_x = 0.0;
                    *cur_y += *line_height;
                    *line_height = 0.0;
                }
                let mut cb = Dimensions {
                    content: Rect::new(container_content.x, container_content.y, max_width, 0.0),
                    padding: EdgeSizes::default(),
                    border: EdgeSizes::default(),
                    margin: EdgeSizes::default(),
                };
                cb.content.height = *cur_y;
                item.layout(cb);
                *cur_y += item.dimensions.margin_box().height;
                *cur_x = 0.0;
                *line_height = 0.0;
            }
        }
    }

    fn layout_inline_children(&mut self) {
        let max_width = self.dimensions.content.width.max(10.0);
        let mut cur_x = 0.0;
        let mut cur_y = 0.0;
        let mut line_height: f32 = 20.0;

        for child in &mut self.children {
            Self::layout_inline_item(
                child,
                self.dimensions.content,
                max_width,
                &mut cur_x,
                &mut cur_y,
                &mut line_height,
            );
        }

        self.dimensions.content.height = cur_y + line_height;
    }

    fn calculate_block_height(&mut self) {
        if let Some(style) = self.get_styled_node() {
            if let Some(h) = style.value("height") {
                let px = parse_len_value(h, 600.0);
                if px > 0.0 {
                    self.dimensions.content.height = px;
                }
            }
            if let Some(min_h) = style.value("min-height") {
                let px = parse_len_value(min_h, 600.0);
                if px > 0.0 {
                    self.dimensions.content.height = self.dimensions.content.height.max(px);
                }
            }
            if let Some(max_h) = style.value("max-height") {
                let px = parse_len_value(max_h, 600.0);
                if px > 0.0 {
                    self.dimensions.content.height = self.dimensions.content.height.min(px);
                }
            }
        }
    }
}

fn compute_grid_column_widths(spec: &str, parent_w: f32, child_count: usize, gap: f32) -> Vec<f32> {
    let mut tokens: Vec<String> = Vec::new();
    let spec_trimmed = spec.trim();

    // Check repeat(N, ...)
    if let Some(repeat_pos) = spec_trimmed.find("repeat(") {
        if let Some(end_pos) = spec_trimmed[repeat_pos..].find(')') {
            let inner = &spec_trimmed[repeat_pos + 7..repeat_pos + end_pos];
            if let Some(comma) = inner.find(',') {
                let n_str = inner[..comma].trim();
                let val_str = inner[comma + 1..].trim();
                if let Ok(n) = n_str.parse::<usize>() {
                    for _ in 0..n {
                        tokens.push(val_str.to_string());
                    }
                }
            }
        }
    }

    if tokens.is_empty() {
        if !spec_trimmed.is_empty() {
            tokens = spec_trimmed.split_whitespace().map(|s| s.to_string()).collect();
        } else {
            if child_count > 1 {
                tokens = vec!["1fr".to_string(), "1fr".to_string()];
            } else {
                tokens = vec!["1fr".to_string()];
            }
        }
    }

    let num_cols = tokens.len().max(1);
    let total_gaps = (num_cols.saturating_sub(1) as f32) * gap;
    let available_w = (parent_w - total_gaps).max(0.0);

    let mut fixed_w = 0.0;
    let mut fr_sum = 0.0;
    enum ColType { Fixed(f32), Fr(f32) }
    let mut col_types = Vec::new();

    for t in &tokens {
        let t_clean = t.trim().to_ascii_lowercase();
        if t_clean.ends_with("fr") {
            let fr_val = t_clean.strip_suffix("fr").unwrap().trim().parse::<f32>().unwrap_or(1.0);
            fr_sum += fr_val;
            col_types.push(ColType::Fr(fr_val));
        } else if t_clean.ends_with("px") {
            let px = t_clean.strip_suffix("px").unwrap().trim().parse::<f32>().unwrap_or(100.0);
            fixed_w += px;
            col_types.push(ColType::Fixed(px));
        } else if t_clean.ends_with("rem") || t_clean.ends_with("em") {
            let n = t_clean.strip_suffix("rem").or_else(|| t_clean.strip_suffix("em")).unwrap().trim().parse::<f32>().unwrap_or(12.0);
            let px = n * 16.0;
            fixed_w += px;
            col_types.push(ColType::Fixed(px));
        } else if t_clean == "min-content" || t_clean == "max-content" || t_clean == "auto" {
            let w = 220.0_f32.min(available_w * 0.28).max(120.0);
            fixed_w += w;
            col_types.push(ColType::Fixed(w));
        } else {
            let w = available_w / num_cols as f32;
            fixed_w += w;
            col_types.push(ColType::Fixed(w));
        }
    }

    let remaining_w = (available_w - fixed_w).max(0.0);
    col_types.into_iter().map(|ct| match ct {
        ColType::Fixed(w) => w,
        ColType::Fr(fr) => {
            if fr_sum > 0.0 {
                remaining_w * (fr / fr_sum)
            } else {
                available_w / num_cols as f32
            }
        }
    }).collect()
}

/// Builds a tree of LayoutBoxes from the StyledNode tree.
pub fn build_layout_tree<'a>(root: &'a StyledNode<'a>) -> Option<LayoutBox<'a>> {
    if root.display() == Display::None {
        return None;
    }

    let mut root_box = LayoutBox::new(match root.display() {
        Display::Block | Display::Flex | Display::Grid => BoxType::BlockNode(root),
        Display::Inline | Display::InlineBlock => BoxType::InlineNode(root),
        Display::None => unreachable!(),
    });

    for child in &root.children {
        if child.display() == Display::None {
            continue;
        }

        match &child.node.node_type {
            NodeType::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    let cleaned = clean_text_whitespace(text);
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
                if child.node.tag_name() == Some("img") {
                    let src = child.node.get_attribute("src").unwrap_or("").to_string();
                    let alt = child.node.get_attribute("alt").unwrap_or("").to_string();
                    root_box.children.push(LayoutBox::new(BoxType::ImageNode {
                        src,
                        alt,
                        styled_node: child,
                    }));
                } else if let Some(child_box) = build_layout_tree(child) {
                    root_box.children.push(child_box);
                }
            }
            NodeType::Comment(_) => {}
        }
    }

    Some(root_box)
}

fn is_invisible_or_control(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200B}'
                | '\u{200C}'
                | '\u{200D}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{00AD}'
        )
}

fn clean_text_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut prev_whitespace = false;
    for c in s.chars() {
        if is_invisible_or_control(c) {
            continue;
        }
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
        if is_invisible_or_control(c) {
            continue;
        }
        if c.is_whitespace() {
            if !current.is_empty() {
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
            ' ' => 0.35,
            'i' | 'l' | 'j' | '!' | '.' | ',' | ':' | ';' | '\'' | '|' | '’' => 0.30,
            'f' | 'r' | 't' | '(' | ')' | '[' | ']' | '-' | '"' => 0.40,
            'm' | 'w' | 'M' | 'W' => 0.95,
            'A'..='Z' => 0.75,
            '0'..='9' => 0.62,
            _ => 0.60,
        };
        width += factor * font_size;
    }
    width.max(2.0)
}

/// DEVTOOLS: Recursively searches a LayoutBox tree for the box representing a target DOM Node.
pub fn find_layout_box_for_node<'a, 'b>(
    root: &'b LayoutBox<'a>,
    target: &crate::dom::Node,
) -> Option<&'b LayoutBox<'a>> {
    if let Some(styled) = root.get_styled_node() {
        if std::ptr::eq(styled.node, target) {
            return Some(root);
        }
    }
    for child in &root.children {
        if let Some(found) = find_layout_box_for_node(child, target) {
            return Some(found);
        }
    }
    None
}

/// DEVTOOLS: Computes the layout dimensions of a target DOM node within a given DOM tree and stylesheet.
pub fn query_node_dimensions(
    dom: &crate::dom::Node,
    stylesheet: &crate::css::Stylesheet,
    target: &crate::dom::Node,
    width: f32,
) -> Option<Dimensions> {
    let styled_tree = crate::style::style_tree(dom, stylesheet);
    if let Some(mut layout_box) = build_layout_tree(&styled_tree) {
        let containing_block = Dimensions {
            content: Rect::new(0.0, 0.0, width.max(300.0), 0.0),
            padding: EdgeSizes::default(),
            border: EdgeSizes::default(),
            margin: EdgeSizes::default(),
        };
        layout_box.layout(containing_block);
        return find_layout_box_for_node(&layout_box, target).map(|b| b.dimensions);
    }
    None
}
