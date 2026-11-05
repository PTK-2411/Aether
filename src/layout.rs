use crate::css::{Unit, Value};
use crate::dom::NodeType;
use crate::style::{Display, StyledNode};
use crate::text_metrics::{self, Face};
use std::cell::Cell;

thread_local! {
    static VIEWPORT: (Cell<f32>, Cell<f32>) = (const { Cell::new(1000.0) }, const { Cell::new(800.0) });
}

/// Viewport size used for `vh`/`vw` units and `position: fixed` containing blocks.
pub fn set_viewport(width: f32, height: f32) {
    VIEWPORT.with(|(w, h)| {
        w.set(width.max(1.0));
        h.set(height.max(1.0));
    });
}

pub fn viewport_size() -> (f32, f32) {
    VIEWPORT.with(|(w, h)| (w.get(), h.get()))
}

/// Height the document actually needs: the lowest edge of any laid-out box.
/// Out-of-flow and overflowing boxes sit outside the root's own height, so
/// using the root box alone leaves the bottom of the page unreachable.
pub fn content_height(root: &LayoutBox) -> f32 {
    fn deepest_edge(box_ref: &LayoutBox, acc: &mut f32) {
        let mb = box_ref.dimensions.margin_box();
        *acc = acc.max(mb.y + mb.height);
        for child in &box_ref.children {
            deepest_edge(child, acc);
        }
    }
    let mut acc = 0.0;
    deepest_edge(root, &mut acc);
    acc
}

/// Typography needed to measure a run of text. Font sizes are already resolved
/// to pixels by the cascade (see `style::compute_font_size`).
#[derive(Clone, Copy, Debug)]
struct FontSpec {
    size: f32,
    face: Face,
    monospace: bool,
}

impl FontSpec {
    fn width(&self, text: &str) -> f32 {
        text_metrics::measure_width(text, self.size, self.face, self.monospace)
    }

    fn space(&self) -> f32 {
        text_metrics::space_width(self.size, self.face, self.monospace)
    }
}

fn font_spec(styled: &StyledNode) -> FontSpec {
    let size = styled.font_size(None);
    let face = match (styled.is_bold(None), styled.is_italic(None)) {
        (true, true) => Face::BoldItalic,
        (true, false) => Face::Bold,
        (false, true) => Face::Italic,
        (false, false) => Face::Regular,
    };
    let family = match styled.value("font-family") {
        Some(Value::Keyword(k)) => Some(k.as_str()),
        Some(Value::StringLiteral(s)) => Some(s.as_str()),
        _ => None,
    };
    FontSpec {
        size,
        face,
        monospace: text_metrics::is_monospace_family(family),
    }
}

/// CSS line-height when specified, otherwise the font's own row height.
fn box_line_height(styled: &StyledNode) -> f32 {
    let spec = font_spec(styled);
    if styled.value("line-height").is_some() {
        styled.line_height(spec.size, None).max(spec.size)
    } else {
        text_metrics::line_height(spec.size, spec.face, spec.monospace)
    }
}

/// Intrinsic size of an image: CSS width/height, then HTML attributes, then a
/// placeholder box so the alt text still has somewhere to go.
fn image_size(styled: Option<&StyledNode>, container_width: f32, container_height: f32) -> (f32, f32) {
    const PLACEHOLDER_W: f32 = 240.0;
    const PLACEHOLDER_H: f32 = 160.0;
    let Some(styled) = styled else {
        return (PLACEHOLDER_W, PLACEHOLDER_H);
    };
    let attr = |name: &str| {
        styled
            .node
            .get_attribute(name)
            .and_then(|v| v.trim_end_matches("px").trim().parse::<f32>().ok())
    };
    let css_w = styled.value("width").map(|w| parse_len_value(w, container_width)).filter(|w| *w > 0.0);
    let css_h = styled
        .value("height")
        .map(|h| specified_height_px(h, container_height))
        .filter(|h| *h > 0.0);

    let w = css_w.or_else(|| attr("width")).unwrap_or(PLACEHOLDER_W);
    // Without a resolvable height the box keeps the placeholder's aspect ratio
    // rather than a fixed 160px, so a 122px thumbnail stays thumbnail-sized and
    // a full-width banner doesn't collapse.
    let h = css_h
        .or_else(|| attr("height"))
        .unwrap_or_else(|| (w * PLACEHOLDER_H / PLACEHOLDER_W).clamp(16.0, PLACEHOLDER_H * 3.5));
    (w, h)
}

fn keyword_value<'a>(styled: &'a StyledNode, property: &str) -> Option<&'a str> {
    match styled.value(property) {
        Some(Value::Keyword(k)) => Some(k.as_str()),
        Some(Value::StringLiteral(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// A length token carried by the `flex` shorthand. `auto`, `content` and the
/// keyword shorthands defer to content sizing, so they yield `None`.
fn token_length(token: &str, reference: f32) -> Option<f32> {
    let t = token.trim().to_ascii_lowercase();
    if matches!(t.as_str(), "auto" | "content" | "none" | "initial" | "") {
        return None;
    }
    crate::css::parse_value_str(&t)
        .filter(|v| matches!(v, Value::Length(..) | Value::Number(_)))
        .map(|v| parse_len_value(&v, reference))
}

fn keyword_value_ref<'a>(styled: Option<&'a StyledNode>, property: &str) -> Option<&'a str> {
    styled.and_then(|s| match s.value(property) {
        Some(Value::Keyword(k)) => Some(k.as_str()),
        _ => None,
    })
}

fn is_out_of_flow(styled: Option<&StyledNode>) -> bool {
    let position = match styled {
        Some(s) => match s.value("position") {
            Some(Value::Keyword(k)) => k.as_str(),
            _ => "",
        },
        None => "",
    };
    position == "absolute" || position == "fixed"
}

/// Containing block an out-of-flow box is sized and positioned against: the
/// viewport for `position: fixed`, the parent for `position: absolute`
/// (CSS 2.1 §10.1). Out-of-flow boxes don't stack, so the running
/// `content.height` offset the block loop uses as a cursor is dropped.
fn out_of_flow_containing_block(styled: Option<&StyledNode>, parent: Dimensions) -> Dimensions {
    let content = if keyword_value_ref(styled, "position") == Some("fixed") {
        let (vw, vh) = viewport_size();
        Rect::new(0.0, 0.0, vw, vh)
    } else {
        Rect {
            x: parent.content.x,
            y: parent.content.y,
            width: parent.content.width,
            height: 0.0,
        }
    };
    Dimensions {
        content,
        ..Dimensions::default()
    }
}

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

/// Main axis of a flex container, used to pick the sizing property.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
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
        Value::Length(vh, Unit::Vh) => viewport_size().1 * (*vh / 100.0),
        Value::Length(vw, Unit::Vw) => base * (*vw / 100.0),
        Value::Number(n) => *n,
        Value::Keyword(k) if k.starts_with("calc(") && k.ends_with(')') => {
            eval_calc(&k[5..k.len() - 1], base)
        }
        _ => 0.0,
    }
}

/// Used value of a `height`/`min-height`/`max-height` declaration.
///
/// CSS resolves a percentage height against the containing block's height and
/// falls back to `auto` when that height isn't definite. Resolving percentages
/// against the viewport instead turned every `height:100%` thumbnail into an
/// 800px box and made result pages many screens taller than Chrome shows them,
/// so a percentage with no definite parent yields `0.0`, which callers already
/// treat as "no declared height".
fn specified_height_px(value: &Value, containing_block_height: f32) -> f32 {
    match value {
        Value::Length(pct, Unit::Percent) if containing_block_height > 0.0 => {
            containing_block_height * pct / 100.0
        }
        Value::Length(_, Unit::Percent) => 0.0,
        other => parse_len_value(other, containing_block_height),
    }
}

#[derive(Clone, Debug)]
pub struct LayoutBox<'a> {
    pub dimensions: Dimensions,
    pub box_type: BoxType<'a>,
    pub children: Vec<LayoutBox<'a>>,
    /// The parent's definite content height, or 0 when the parent's height is
    /// `auto`. Descendants resolve percentage heights against this and nothing
    /// else; see `calculate_block_height`.
    pub percent_height_base: f32,
}

impl<'a> LayoutBox<'a> {
    pub fn new(box_type: BoxType<'a>) -> Self {
        LayoutBox {
            dimensions: Dimensions::default(),
            box_type,
            children: Vec::new(),
            percent_height_base: 0.0,
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
                self.layout_standalone_inline(containing_block);
            }
        }
    }

    /// An inline-level box reached directly — as a flex/grid item or an
    /// out-of-flow box — has to be sized on its own. Skipping it leaves the box
    /// at 0x0 and strands every descendant at the page origin.
    fn layout_standalone_inline(&mut self, containing_block: Dimensions) {
        match self.box_type {
            BoxType::InlineNode(_) => {
                self.calculate_inline_block_width(containing_block);
                self.calculate_block_position(containing_block);
                self.layout_block_children();
                self.calculate_block_height();
                self.apply_relative_and_absolute_positioning(containing_block);
            }
            BoxType::ImageNode { .. } => {
                self.calculate_block_width(containing_block);
                self.calculate_block_position(containing_block);
                let styled = self.get_styled_node();
                let (w, h) = image_size(styled, containing_block.content.width, containing_block.content.height);
                self.dimensions.content.width = w;
                self.dimensions.content.height = h;
            }
            BoxType::TextFragment { .. } => {
                self.calculate_block_width(containing_block);
                self.calculate_block_position(containing_block);
                if let BoxType::TextFragment { text, styled_node } = &self.box_type {
                    self.dimensions.content.width = font_spec(styled_node).width(text);
                    self.dimensions.content.height = box_line_height(styled_node);
                }
            }
            _ => {}
        }
    }

    /// Shrink-to-fit width for inline-block boxes and blockified flex items.
    fn calculate_inline_block_width(&mut self, containing_block: Dimensions) {
        let specified_width = self
            .get_styled_node()
            .and_then(|s| s.value("width"))
            .filter(|v| !matches!(v, Value::Keyword(k) if k == "auto"))
            .is_some();

        self.calculate_block_width(containing_block);
        if specified_width {
            return;
        }

        let chrome = self.dimensions.padding.left
            + self.dimensions.padding.right
            + self.dimensions.border.left
            + self.dimensions.border.right;
        let preferred = self.max_content_width() + chrome;
        let available = containing_block.content.width;
        let fitted = if available > 0.0 {
            (available - self.dimensions.margin.left - self.dimensions.margin.right).max(0.0).min(preferred)
        } else {
            preferred
        };
        self.dimensions.content.width = fitted.max(0.0);

        if let Some(style) = self.get_styled_node() {
            let parent_w = containing_block.content.width;
            if let Some(mw) = style.value("max-width") {
                let max_w = parse_len_value(mw, parent_w);
                if max_w > 0.0 {
                    self.dimensions.content.width = self.dimensions.content.width.min(max_w);
                }
            }
            if let Some(mw) = style.value("min-width") {
                let min_w = parse_len_value(mw, parent_w);
                if min_w > 0.0 {
                    self.dimensions.content.width = self.dimensions.content.width.max(min_w);
                }
            }
        }
    }

    /// Max-content width of this box's content area, i.e. how wide it would be
    /// with unlimited space (CSS 2.1 §16.1 step 1 of shrink-to-fit).
    pub fn max_content_width(&self) -> f32 {
        match &self.box_type {
            BoxType::TextFragment { text, styled_node } => font_spec(styled_node).width(text),
            BoxType::ImageNode { styled_node, .. } => image_size(Some(*styled_node), 0.0, 0.0).0,
            _ => {
                let in_flow: Vec<&LayoutBox> = self
                    .children
                    .iter()
                    .filter(|c| !is_out_of_flow(c.get_styled_node()))
                    .collect();
                if in_flow.is_empty() {
                    return 0.0;
                }
                let inline_flow = in_flow.iter().any(|c| {
                    matches!(
                        c.box_type,
                        BoxType::InlineNode(_) | BoxType::TextFragment { .. } | BoxType::ImageNode { .. }
                    )
                });

                if inline_flow {
                    let space = in_flow
                        .iter()
                        .find_map(|c| c.get_styled_node().map(font_spec))
                        .map(|s| s.space())
                        .unwrap_or(4.0);
                    let mut total: f32 = 0.0;
                    let mut first = true;
                    for child in &in_flow {
                        if matches!(child.box_type, BoxType::BlockNode(_)) {
                            // A block child breaks the line; the widest line wins.
                            total = total.max(child.margin_box_max_content_width());
                            first = true;
                            continue;
                        }
                        if !first {
                            total += space;
                        }
                        total += child.margin_box_max_content_width();
                        first = false;
                    }
                    total
                } else {
                    in_flow
                        .iter()
                        .map(|c| c.margin_box_max_content_width())
                        .fold(0.0, f32::max)
                }
            }
        }
    }

    fn margin_box_max_content_width(&self) -> f32 {
        let Some(style) = self.get_styled_node() else {
            return self.max_content_width();
        };
        let zero = Value::Length(0.0, Unit::Px);
        let side = |name: &str| style.value(name).unwrap_or(&zero);
        let auto_margin = |name: &str| matches!(side(name), Value::Keyword(k) if k == "auto");
        let chrome = parse_len_value(side("padding-left"), 0.0)
            + parse_len_value(side("padding-right"), 0.0)
            + parse_len_value(side("border-left-width"), 0.0)
            + parse_len_value(side("border-right-width"), 0.0)
            + if auto_margin("margin-left") { 0.0 } else { parse_len_value(side("margin-left"), 0.0) }
            + if auto_margin("margin-right") { 0.0 } else { parse_len_value(side("margin-right"), 0.0) };
        self.max_content_width() + chrome
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
        let grid_height_base = self.declared_height_px();

        let mut cell_idx = 0;
        for child in &mut self.children {
            let is_out_of_flow = child
                .get_styled_node()
                .and_then(|s| s.value("position"))
                .map_or(false, |v| matches!(v, Value::Keyword(k) if k == "absolute" || k == "fixed"));

            if is_out_of_flow {
                let cb = out_of_flow_containing_block(
                    child.get_styled_node(),
                    Dimensions {
                        content: Rect::new(
                            self.dimensions.content.x,
                            self.dimensions.content.y,
                            parent_w,
                            0.0,
                        ),
                        ..Dimensions::default()
                    },
                );
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

            child.percent_height_base = grid_height_base;
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
        let direction = keyword_value_ref(style, "flex-direction").unwrap_or("row").to_string();
        let wrap = matches!(keyword_value_ref(style, "flex-wrap"), Some("wrap") | Some("wrap-reverse"));
        let justify = keyword_value_ref(style, "justify-content").unwrap_or("flex-start").to_string();
        let align_default = keyword_value_ref(style, "align-items").unwrap_or("stretch").to_string();

        let gap = style
            .and_then(|s| s.value("gap").or_else(|| s.value(if direction == "column" { "row-gap" } else { "column-gap" })))
            .map(|v| parse_len_value(v, containing_block.content.width))
            .unwrap_or(0.0);

        let parent_w = self.dimensions.content.width.max(10.0);
        let parent_h = self.dimensions.content.height;
        let origin_x = self.dimensions.content.x;
        let origin_y = self.dimensions.content.y;

        let flow_children: Vec<usize> = (0..self.children.len())
            .filter(|i| !is_out_of_flow(self.children[*i].get_styled_node()))
            .collect();

        let flex_height_base = self.declared_height_px();
        for &child_idx in &flow_children {
            self.children[child_idx].percent_height_base = flex_height_base;
        }

        if direction == "column" {
            let mut cur_y = 0.0;
            for (i, &child_idx) in flow_children.iter().enumerate() {
                if i > 0 {
                    cur_y += gap;
                }
                let cb = Dimensions {
                    content: Rect::new(origin_x, origin_y + cur_y, parent_w, 0.0),
                    padding: EdgeSizes::default(),
                    border: EdgeSizes::default(),
                    margin: EdgeSizes::default(),
                };
                match self.flex_item_forced_height(child_idx, parent_h) {
                    Some(main_size) => self.children[child_idx]
                        .layout_main_axis_size(cb, main_size, Axis::Vertical),
                    None => self.children[child_idx].layout(cb),
                }
                let mb = self.children[child_idx].dimensions.margin_box();
                cur_y += mb.height;
            }
            if parent_h <= 0.0 {
                self.dimensions.content.height = cur_y;
            }
        } else {
            // Pass 1: base sizes from flex-basis / width / max-content.
            let mut base: Vec<f32> = Vec::with_capacity(flow_children.len());
            for &child_idx in &flow_children {
                base.push(self.flex_item_base_size(child_idx, parent_w, Axis::Horizontal));
            }

            let total_gap = if flow_children.is_empty() { 0.0 } else { (flow_children.len() as f32 - 1.0) * gap };
            let total_base: f32 = base.iter().sum();
            let free = parent_w - total_gap - total_base;

            // Pass 2: distribute free space with flex-grow / flex-shrink.
            if free > 0.01 {
                let total_grow: f32 = flow_children.iter().map(|i| self.flex_grow(*i)).sum();
                if total_grow > 0.0 {
                    for (n, &child_idx) in flow_children.iter().enumerate() {
                        let grow = self.flex_grow(child_idx);
                        if grow > 0.0 {
                            base[n] += free * (grow / total_grow);
                        }
                    }
                }
            } else if free < -0.01 {
                let weighted: Vec<f32> = base
                    .iter()
                    .enumerate()
                    .map(|(n, w)| self.flex_shrink(flow_children[n]) * w.max(1.0))
                    .collect();
                let total_shrink: f32 = weighted.iter().sum();
                if total_shrink > 0.0 {
                    for (n, &child_idx) in flow_children.iter().enumerate() {
                        let shrink = self.flex_shrink(child_idx);
                        if shrink > 0.0 {
                            let reduction = (-free) * (weighted[n] / total_shrink);
                            base[n] = (base[n] - reduction).max(1.0);
                        }
                    }
                }
            }

            // Pass 3: place lines.
            let mut lines: Vec<Vec<usize>> = Vec::new();
            let mut current: Vec<usize> = Vec::new();
            let mut used = 0.0f32;
            for (n, &_) in flow_children.iter().enumerate() {
                let w = base[n];
                if wrap && !current.is_empty() && used + gap + w > parent_w {
                    used = 0.0;
                    lines.push(std::mem::take(&mut current));
                }
                if !current.is_empty() {
                    used += gap;
                }
                used += w;
                current.push(n);
            }
            if !current.is_empty() {
                lines.push(current);
            }

            let mut line_y = 0.0f32;
            for (line_no, line) in lines.iter().enumerate() {
                let line_gap = if line.len() > 1 { (line.len() as f32 - 1.0) * gap } else { 0.0 };
                let line_total: f32 = line.iter().map(|n| base[*n]).sum::<f32>() + line_gap;
                let extra = (parent_w - line_total).max(0.0);
                let mut between = 0.0f32;
                let mut before = 0.0f32;
                match justify.as_str() {
                    "center" => before = extra / 2.0,
                    "flex-end" | "end" | "right" => before = extra,
                    "space-between" if line.len() > 1 => {
                        between = extra / (line.len() as f32 - 1.0);
                    }
                    "space-around" if !line.is_empty() => {
                        between = extra / line.len() as f32;
                        before = between / 2.0;
                    }
                    "space-evenly" if !line.is_empty() => {
                        between = extra / (line.len() as f32 + 1.0);
                        before = between;
                    }
                    _ => {}
                }

                let mut cur_x = before;
                let mut line_h = 0.0f32;
                for &n in line {
                    let child_idx = flow_children[n];
                    let item_w = base[n];
                    let cb = Dimensions {
                        content: Rect::new(origin_x + cur_x, origin_y, item_w.max(1.0), 0.0),
                        padding: EdgeSizes::default(),
                        border: EdgeSizes::default(),
                        margin: EdgeSizes::default(),
                    };
                    self.children[child_idx].layout_main_axis_size(cb, item_w, Axis::Horizontal);
                    let mb = self.children[child_idx].dimensions.margin_box();
                    line_h = line_h.max(mb.height);
                    cur_x += mb.width + gap + between;
                }

                for &n in line {
                    let child_idx = flow_children[n];
                    let align_self = keyword_value_ref(self.children[child_idx].get_styled_node(), "align-self")
                        .unwrap_or(align_default.as_str())
                        .to_string();
                    let mb = self.children[child_idx].dimensions.margin_box();
                    let dy = match align_self.as_str() {
                        "center" => (line_h - mb.height) / 2.0,
                        "flex-end" | "end" => line_h - mb.height,
                        _ => 0.0,
                    };
                    if dy.abs() > 0.01 {
                        self.children[child_idx].translate(0.0, dy);
                    }
                }

                line_y += line_h + if line_no + 1 < lines.len() { gap } else { 0.0 };
            }
            if parent_h <= 0.0 {
                self.dimensions.content.height = line_y;
            }
        }

        // Out-of-flow children are still descendants and must be laid out; the
        // flex line only ignores them.
        let parent_dims = self.dimensions;
        for child in &mut self.children {
            if is_out_of_flow(child.get_styled_node()) {
                let cb = out_of_flow_containing_block(
                    child.get_styled_node(),
                    Dimensions {
                        content: Rect {
                            height: 0.0,
                            ..parent_dims.content
                        },
                        ..Dimensions::default()
                    },
                );
                child.layout(cb);
            }
        }

        self.calculate_block_height();
        self.apply_relative_and_absolute_positioning(containing_block);
    }

    /// Lays the item out, then forces its main size to the resolved flex size so
    /// that `flex: 1` actually fills the line instead of sizing to content.
    fn layout_main_axis_size(&mut self, containing_block: Dimensions, main_size: f32, axis: Axis) {
        self.layout(containing_block);
        if main_size <= 0.0 {
            return;
        }
        match axis {
            Axis::Horizontal => {
                self.dimensions.content.width = (main_size
                    - self.dimensions.padding.left
                    - self.dimensions.padding.right
                    - self.dimensions.border.left
                    - self.dimensions.border.right)
                    .max(0.0);
            }
            Axis::Vertical => {
                self.dimensions.content.height = (main_size
                    - self.dimensions.padding.top
                    - self.dimensions.padding.bottom
                    - self.dimensions.border.top
                    - self.dimensions.border.bottom)
                    .max(0.0);
            }
        }
    }

    /// The basis hidden inside the `flex` shorthand, which the property parser
    /// keeps as one keyword string: `flex: 1` means `1 1 0`, while a length
    /// token (`flex: 300px`, `flex: 1 1 25%`) sets the basis directly.
    fn shorthand_flex_basis(&self, child_idx: usize, reference: f32) -> Option<f32> {
        let Some(Value::Keyword(shorthand)) =
            self.children[child_idx].get_styled_node()?.value("flex") else {
            return None;
        };
        let tokens: Vec<&str> = shorthand.split_whitespace().collect();
        let last = *tokens.last()?;
        // A numeric flex factor with no basis token resolves the basis to 0,
        // which is what `flex: 1` and `flex: 1 1` mean in a browser.
        if last.parse::<f32>().is_ok() {
            return Some(0.0);
        }
        token_length(last, reference)
    }

    /// The main size a column flex item must be resized to, or `None` when the
    /// item's laid-out height is already the used height.
    ///
    /// The intrinsic `margin_box_max_content_height` helper is deliberately not
    /// consulted here: it stacks the line box of every descendant, so a paragraph
    /// of wrapped text reports thousands of pixels as its "max-content" height.
    /// Forcing that value over a correctly laid-out box is what made search
    /// results six times taller than the same page in a real browser.
    fn flex_item_forced_height(&self, child_idx: usize, reference: f32) -> Option<f32> {
        let style = self.children[child_idx].get_styled_node()?;
        let declared = |prop: &str| {
            style
                .value(prop)
                .filter(|v| !matches!(v, Value::Keyword(k) if k == "auto" || k == "content"))
                .map(|v| parse_len_value(v, reference))
                .filter(|px| *px > 0.0)
        };
        declared("flex-basis")
            .or_else(|| declared("height"))
            .map(|h| h + self.children[child_idx].vertical_padding_and_border())
    }

    /// padding + border on the vertical axis, without laying out.
    ///
    /// `vertical_chrome` also counts margins, which is right for line packing but
    /// wrong here: `layout_main_axis_size` subtracts exactly this value, so using
    /// the margin-inclusive chrome would turn `height: 300px` into 312.
    fn vertical_padding_and_border(&self) -> f32 {
        let Some(style) = self.get_styled_node() else {
            return 0.0;
        };
        let zero = Value::Length(0.0, Unit::Px);
        let side = |name: &str| style.value(name).unwrap_or(&zero);
        parse_len_value(side("padding-top"), 0.0)
            + parse_len_value(side("padding-bottom"), 0.0)
            + parse_len_value(side("border-top-width"), 0.0)
            + parse_len_value(side("border-bottom-width"), 0.0)
    }

    /// The flex base size: the used main size before grow/shrink is applied.
    fn flex_item_base_size(&self, child_idx: usize, reference: f32, axis: Axis) -> f32 {
        let child = &self.children[child_idx];
        let style = child.get_styled_node();
        let (size_prop, min_prop, max_prop, chrome) = match axis {
            Axis::Horizontal => ("width", "min-width", "max-width", child.horizontal_chrome()),
            Axis::Vertical => ("height", "min-height", "max-height", child.vertical_chrome()),
        };

        let explicit_basis = style
            .and_then(|s| s.value(if axis == Axis::Horizontal { "flex-basis" } else { "height" }))
            .filter(|v| !matches!(v, Value::Keyword(k) if k == "auto" || k == "content"))
            .map(|v| parse_len_value(v, reference))
            .or_else(|| match axis {
                Axis::Horizontal => self.shorthand_flex_basis(child_idx, reference),
                Axis::Vertical => None,
            });

        let specified = explicit_basis.or_else(|| {
            style
                .and_then(|s| s.value(size_prop))
                .filter(|v| !matches!(v, Value::Keyword(k) if k == "auto"))
                .map(|v| parse_len_value(v, reference))
        });

        let mut base = match specified {
            Some(w) => w + chrome,
            None => match axis {
                Axis::Horizontal => child.margin_box_max_content_width(),
                Axis::Vertical => child.margin_box_max_content_height(),
            },
        };

        if let Some(mw) = style.and_then(|s| s.value(min_prop)) {
            let min_v = parse_len_value(mw, reference);
            if min_v > 0.0 {
                base = base.max(min_v + chrome);
            }
        }
        if let Some(mw) = style.and_then(|s| s.value(max_prop)) {
            let max_v = parse_len_value(mw, reference);
            if max_v > 0.0 {
                base = base.min(max_v + chrome);
            }
        }
        base.max(0.0)
    }

    /// `flex-grow` / `flex-shrink`, including the `flex` shorthand (`1 1 auto`).
    fn flex_factors(&self, child_idx: usize) -> (f32, f32) {
        let number = |v: &Value| match v {
            Value::Number(n) => *n,
            Value::Keyword(k) => k.split_whitespace().next().and_then(|t| t.parse().ok()).unwrap_or(0.0),
            _ => 0.0,
        };
        let style = self.children[child_idx].get_styled_node();
        let mut grow = style.and_then(|s| s.value("flex-grow")).map(|v| number(v)).unwrap_or(0.0);
        let mut shrink = style.and_then(|s| s.value("flex-shrink")).map(|v| number(v)).unwrap_or(1.0);

        if let Some(Value::Keyword(k)) = style.and_then(|s| s.value("flex")) {
            let tokens: Vec<&str> = k.split_whitespace().collect();
            match tokens.as_slice() {
                [g] => {
                    if *g == "auto" {
                        grow = 1.0;
                    } else if let Ok(parsed) = g.parse::<f32>() {
                        grow = parsed;
                        shrink = 1.0;
                    }
                }
                [g, s] => {
                    if let Ok(parsed) = g.parse::<f32>() {
                        grow = parsed;
                    }
                    if let Ok(parsed) = s.parse::<f32>() {
                        shrink = parsed;
                    }
                }
                [g, s, _basis] => {
                    if let Ok(parsed) = g.parse::<f32>() {
                        grow = parsed;
                    }
                    if let Ok(parsed) = s.parse::<f32>() {
                        shrink = parsed;
                    }
                }
                _ => {}
            }
        }
        (grow.max(0.0), shrink.max(0.0))
    }

    fn flex_grow(&self, child_idx: usize) -> f32 {
        self.flex_factors(child_idx).0
    }

    fn flex_shrink(&self, child_idx: usize) -> f32 {
        self.flex_factors(child_idx).1
    }

    /// padding + border + margin on the vertical axis, without laying out.
    fn vertical_chrome(&self) -> f32 {
        let Some(style) = self.get_styled_node() else {
            return 0.0;
        };
        let zero = Value::Length(0.0, Unit::Px);
        let side = |name: &str| style.value(name).unwrap_or(&zero);
        parse_len_value(side("padding-top"), 0.0)
            + parse_len_value(side("padding-bottom"), 0.0)
            + parse_len_value(side("border-top-width"), 0.0)
            + parse_len_value(side("border-bottom-width"), 0.0)
            + parse_len_value(side("margin-top"), 0.0)
            + parse_len_value(side("margin-bottom"), 0.0)
    }

    /// Intrinsic height of a column flex item: its own height, or the height it
    /// would take when given unlimited width.
    fn margin_box_max_content_height(&self) -> f32 {
        if let Some(style) = self.get_styled_node() {
            if let Some(h) = style.value("height") {
                let px = specified_height_px(h, 0.0);
                if px > 0.0 {
                    return px + self.vertical_chrome();
                }
            }
        }
        let line_unit = 16.0 * 1.35;
        match &self.box_type {
            BoxType::TextFragment { styled_node, .. } => box_line_height(styled_node),
            BoxType::ImageNode { styled_node, .. } => image_size(Some(*styled_node), 0.0, 0.0).1,
            _ => self
                .children
                .iter()
                .filter(|c| !is_out_of_flow(c.get_styled_node()))
                .map(|c| c.margin_box_max_content_height())
                .fold(0.0, |acc, h| acc + h)
                .max(line_unit),
        }
    }

    /// padding + border + margin on the horizontal axis, without laying out.
    fn horizontal_chrome(&self) -> f32 {
        let Some(style) = self.get_styled_node() else {
            return 0.0;
        };
        let zero = Value::Length(0.0, Unit::Px);
        let side = |name: &str| style.value(name).unwrap_or(&zero);
        parse_len_value(side("padding-left"), 0.0)
            + parse_len_value(side("padding-right"), 0.0)
            + parse_len_value(side("border-left-width"), 0.0)
            + parse_len_value(side("border-right-width"), 0.0)
            + parse_len_value(side("margin-left"), 0.0)
            + parse_len_value(side("margin-right"), 0.0)
    }

    fn apply_relative_and_absolute_positioning(&mut self, containing_block: Dimensions) {
        if let Some(style) = self.get_styled_node() {
            if let Some(Value::Keyword(pos)) = style.value("position") {
                let (viewport_w, viewport_h) = viewport_size();
                // `position: fixed` is anchored to the viewport, not to its parent.
                let (cb_rect, parent_w, parent_h) = if pos == "fixed" {
                    (Rect::new(0.0, 0.0, viewport_w, viewport_h), viewport_w, viewport_h)
                } else {
                    let w = containing_block.content.width.max(0.0);
                    let h = if containing_block.content.height > 0.0 {
                        containing_block.content.height
                    } else {
                        viewport_h
                    };
                    (containing_block.content, w, h)
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
                        let has_top = style.value("top").is_some();
                        let has_bottom = style.value("bottom").is_some();
                        let has_left = style.value("left").is_some();
                        let has_right = style.value("right").is_some();

                        // CSS 2.1 Section 10.3.7: with auto width and one
                        // horizontal offset, the box shrinks to its content.
                        let width_is_auto = style
                            .value("width")
                            .map_or(true, |v| matches!(v, Value::Keyword(k) if k == "auto"));
                        if width_is_auto && (has_left ^ has_right || has_top ^ has_bottom) {
                            let chrome = self.dimensions.padding.left
                                + self.dimensions.padding.right
                                + self.dimensions.border.left
                                + self.dimensions.border.right;
                            let preferred = self.max_content_width() + chrome;
                            self.dimensions.content.width =
                                self.dimensions.content.width.min(preferred.max(1.0));
                        }

                        if has_top {
                            self.dimensions.content.y = cb_rect.y + parse_len_value(style.value("top").unwrap(), parent_h);
                        } else if has_bottom {
                            let specified_b = parse_len_value(style.value("bottom").unwrap(), parent_h);
                            self.dimensions.content.y = cb_rect.y + parent_h - specified_b
                                - self.dimensions.content.height
                                - self.dimensions.padding.bottom
                                - self.dimensions.border.bottom
                                - self.dimensions.margin.bottom;
                        }
                        if has_left {
                            self.dimensions.content.x = cb_rect.x + parse_len_value(style.value("left").unwrap(), parent_w);
                        } else if has_right {
                            let specified_r = parse_len_value(style.value("right").unwrap(), parent_w);
                            self.dimensions.content.x = cb_rect.x + parent_w - specified_r
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

        if is_out_of_flow(style) {
            // §10.3.7: auto margins are zero and the leftover space belongs to
            // the offsets. Absorbing it into margin-right, as the in-flow
            // equation does, would slide `right`-anchored boxes across the page.
            self.dimensions.margin.left = specified_ml;
            self.dimensions.margin.right = specified_mr;
            self.dimensions.content.width = if w_is_auto {
                (parent_width - specified_ml - specified_mr - total_non_content).max(0.0)
            } else {
                specified_w
            };
        } else if !w_is_auto && sum > parent_width {
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
                (true, ml, mr) => {
                    // Auto width: the box first takes the whole line, then
                    // min/max-width constrains the used width, and only the
                    // space left over after that is shared with the auto
                    // margins. Clamping after positioning would leave the box
                    // pinned to the left edge instead of centered.
                    let mut used = underflow.max(0.0);
                    if let Some(mw) = style.and_then(|s| s.value("max-width")) {
                        let max_w = parse_len_value(mw, parent_width);
                        if max_w > 0.0 {
                            used = used.min(max_w);
                        }
                    }
                    if let Some(mw) = style.and_then(|s| s.value("min-width")) {
                        let min_w = parse_len_value(mw, parent_width);
                        used = used.max(min_w);
                    }
                    let leftover =
                        (parent_width - used - total_non_content - specified_ml - specified_mr).max(0.0);
                    self.dimensions.content.width = used;
                    match (ml, mr) {
                        (true, true) => {
                            self.dimensions.margin.left = leftover / 2.0;
                            self.dimensions.margin.right = leftover / 2.0;
                        }
                        (true, false) => {
                            self.dimensions.margin.left = leftover;
                            self.dimensions.margin.right = specified_mr;
                        }
                        (false, true) => {
                            self.dimensions.margin.left = specified_ml;
                            self.dimensions.margin.right = leftover;
                        }
                        (false, false) => {
                            self.dimensions.margin.left = specified_ml;
                            self.dimensions.margin.right = specified_mr + leftover;
                        }
                    }
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

        // CSS 2.1 Section 10.3: vertical percentages resolve against the
        // containing block's *width*, not its height.
        let percent_base = containing_block.content.width.max(0.0);
        self.dimensions.margin.top = parse_len_value(margin_top, percent_base);
        self.dimensions.margin.bottom = parse_len_value(margin_bottom, percent_base);
        self.dimensions.border.top = parse_len_value(border_top, percent_base);
        self.dimensions.border.bottom = parse_len_value(border_bottom, percent_base);
        self.dimensions.padding.top = parse_len_value(padding_top, percent_base);
        self.dimensions.padding.bottom = parse_len_value(padding_bottom, percent_base);

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
            let height_base = self.declared_height_px();
            let mut y_offset = 0.0;
            for child in &mut self.children {
                let mut cb = self.dimensions;
                cb.content.height = y_offset;
                child.percent_height_base = height_base;
                if is_out_of_flow(child.get_styled_node()) {
                    cb = out_of_flow_containing_block(child.get_styled_node(), cb);
                }
                child.layout(cb);
                if !is_out_of_flow(child.get_styled_node()) {
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
        if is_out_of_flow(item.get_styled_node()) {
            let cb = out_of_flow_containing_block(
                item.get_styled_node(),
                Dimensions {
                    content: Rect::new(container_content.x, container_content.y, max_width, 0.0),
                    ..Dimensions::default()
                },
            );
            item.layout(cb);
            return;
        }

        match item.box_type {
            BoxType::TextFragment { .. } => {
                let (text, styled) = match &item.box_type {
                    BoxType::TextFragment { text, styled_node } => (text.clone(), *styled_node),
                    _ => unreachable!(),
                };
                let spec = font_spec(styled);
                let item_line_height = box_line_height(styled);
                let nowrap = matches!(keyword_value(styled, "white-space"), Some("nowrap") | Some("pre") | Some("pre-line"));
                let text_w = spec.width(&text);

                // One collapsed space before a word that shares its line with
                // something already placed. Adding it after every word instead
                // widens each inline box by a space, so backgrounds, underlines
                // and shrink-to-fit widths all come out too long.
                let shares_line = *cur_x > 0.0;
                if shares_line {
                    *cur_x += spec.space();
                }
                if !nowrap && shares_line && *cur_x + text_w > max_width {
                    *cur_x = 0.0;
                    *cur_y += *line_height;
                    *line_height = item_line_height;
                }
                *line_height = line_height.max(item_line_height);

                item.dimensions.content.x = container_content.x + *cur_x;
                item.dimensions.content.y = container_content.y + *cur_y;
                item.dimensions.content.width = text_w;
                item.dimensions.content.height = item_line_height;
                *cur_x += text_w;
            }
            BoxType::ImageNode { .. } => {
                let styled = item.get_styled_node();
                let (img_w, img_h) = image_size(styled, max_width, container_content.height);
                let shares_line = *cur_x > 0.0;
                if shares_line {
                    *cur_x += styled.map(font_spec).unwrap_or(FontSpec { size: 16.0, face: Face::Regular, monospace: false }).space();
                }
                if shares_line && *cur_x + img_w > max_width {
                    *cur_x = 0.0;
                    *cur_y += *line_height;
                    *line_height = 0.0;
                }
                item.dimensions.content.x = container_content.x + *cur_x;
                item.dimensions.content.y = container_content.y + *cur_y;
                item.dimensions.content.width = img_w;
                item.dimensions.content.height = img_h;
                *line_height = line_height.max(img_h);
                *cur_x += img_w;
            }
            BoxType::InlineNode(_) => {
                let styled = item.get_styled_node();
                let font = styled.map(font_spec).unwrap_or(FontSpec { size: 16.0, face: Face::Regular, monospace: false });
                let item_line_height = styled.map(box_line_height).unwrap_or(font.size * 1.35);

                // Buttons and inputs are replaced-element-like: one box, no inline flow inside.
                let tag = styled.and_then(|s| s.node.tag_name()).unwrap_or("").to_string();
                if tag == "button" || tag == "input" {
                    let label = styled.map(|s| s.node.inner_text()).unwrap_or_default();
                    let is_input = tag == "input";
                    let content_w = if is_input {
                        styled
                            .and_then(|s| s.value("width"))
                            .map(|w| parse_len_value(w, max_width))
                            .filter(|w| *w > 0.0)
                            .unwrap_or_else(|| font.width(&label).max(120.0) + 16.0)
                    } else {
                        font.width(&label) + 16.0
                    };
                    let content_h = if is_input {
                        styled
                            .and_then(|s| s.value("height"))
                            .map(|h| specified_height_px(h, container_content.height))
                            .filter(|h| *h > 0.0)
                            .unwrap_or(item_line_height + 10.0)
                    } else {
                        item_line_height + 10.0
                    };

                    let shares_line = *cur_x > 0.0;
                    if shares_line {
                        *cur_x += font.space();
                    }
                    if shares_line && *cur_x + content_w > max_width {
                        *cur_x = 0.0;
                        *cur_y += *line_height;
                        *line_height = 0.0;
                    }
                    item.dimensions.content.x = container_content.x + *cur_x;
                    item.dimensions.content.y = container_content.y + *cur_y;
                    item.dimensions.content.width = content_w;
                    item.dimensions.content.height = content_h;
                    *line_height = line_height.max(content_h);
                    *cur_x += content_w;
                    return;
                }

                let has_block_descendants = item
                    .children
                    .iter()
                    .any(|c| matches!(c.box_type, BoxType::BlockNode(_) | BoxType::AnonymousBlock));
                let is_inline_block = styled.map(|s| s.display() == Display::InlineBlock).unwrap_or(false);

                if is_inline_block || has_block_descendants {
                    // Inline-block: size to content, wrapping only if the whole box
                    // would not fit on the remaining line.
                    let projected = item.margin_box_max_content_width();
                    let shares_line = *cur_x > 0.0;
                    if shares_line {
                        *cur_x += font.space();
                    }
                    if shares_line && *cur_x + projected > max_width {
                        *cur_x = 0.0;
                        *cur_y += *line_height;
                        *line_height = 0.0;
                    }
                    let remaining = (max_width - *cur_x).max(20.0);
                    let cb = Dimensions {
                        content: Rect::new(container_content.x + *cur_x, container_content.y + *cur_y, remaining, 0.0),
                        padding: EdgeSizes::default(),
                        border: EdgeSizes::default(),
                        margin: EdgeSizes::default(),
                    };
                    item.layout_standalone_inline(cb);
                    let mb = item.dimensions.margin_box();
                    *cur_x += mb.width;
                    *line_height = line_height.max(mb.height);
                    return;
                }

                // Pure inline: keep flowing on the current line.
                let start_x = *cur_x;
                let start_y = *cur_y;
                *line_height = line_height.max(item_line_height);
                let baseline_line = *line_height;

                for sub_child in &mut item.children {
                    Self::layout_inline_item(sub_child, container_content, max_width, cur_x, cur_y, line_height);
                }

                item.dimensions.content.x = container_content.x + start_x;
                item.dimensions.content.y = container_content.y + start_y;
                let measured_w = (*cur_x - start_x).max(0.0);
                let measured_h = (*cur_y - start_y + baseline_line).max(item_line_height);
                item.dimensions.content.width = measured_w;
                item.dimensions.content.height = measured_h;
            }
            BoxType::BlockNode(_) | BoxType::AnonymousBlock => {
                if *cur_x > 0.0 {
                    *cur_x = 0.0;
                    *cur_y += *line_height;
                    *line_height = 0.0;
                }
                let cb = Dimensions {
                    content: Rect::new(container_content.x, container_content.y + *cur_y, max_width, 0.0),
                    padding: EdgeSizes::default(),
                    border: EdgeSizes::default(),
                    margin: EdgeSizes::default(),
                };
                item.layout(cb);
                let mb = item.dimensions.margin_box();
                *cur_y += mb.height;
                *cur_x = 0.0;
                *line_height = 0.0;
            }
        }
    }

    /// Lays out the direct children of a block box that contains inline-level
    /// children: text runs, inline boxes, images and inline-blocks.
    fn layout_inline_children(&mut self) {
        let max_width = self.dimensions.content.width.max(10.0);
        let mut cur_x = 0.0;
        let mut cur_y = 0.0;
        let mut line_height: f32 = self
            .get_styled_node()
            .map(box_line_height)
            .unwrap_or(16.0 * 1.35);

        for child in &mut self.children {
            Self::layout_inline_item(child, self.dimensions.content, max_width, &mut cur_x, &mut cur_y, &mut line_height);
        }

        let align = self
            .get_styled_node()
            .and_then(|s| keyword_value(s, "text-align"))
            .unwrap_or("left")
            .to_string();
        if align != "left" && align != "start" {
            self.align_inline_lines(max_width, &align);
        }

        self.dimensions.content.height = cur_y + line_height;
    }

    /// Shifts the items of each finished line to honour `text-align`.
    fn align_inline_lines(&mut self, max_width: f32, align: &str) {
        let mut lines: Vec<(i32, Vec<usize>)> = Vec::new();
        for (idx, child) in self.children.iter().enumerate() {
            if is_out_of_flow(child.get_styled_node()) {
                continue;
            }
            let key = child.dimensions.margin_box().y.round() as i32;
            match lines.iter_mut().find(|(k, _)| *k == key) {
                Some((_, items)) => items.push(idx),
                None => lines.push((key, vec![idx])),
            }
        }

        let content_x = self.dimensions.content.x;
        for (_, items) in lines {
            let mut line_left = f32::MAX;
            let mut line_right = 0.0f32;
            for &idx in &items {
                let mb = self.children[idx].dimensions.margin_box();
                if mb.width <= 0.0 {
                    continue;
                }
                line_left = line_left.min(mb.x);
                line_right = line_right.max(mb.x + mb.width);
            }
            if line_left == f32::MAX || line_right <= line_left {
                continue;
            }
            let offset_from_content = line_left - content_x;
            let dx = match align {
                "center" => (max_width - (line_right - line_left)) / 2.0 - offset_from_content,
                "right" | "end" => max_width - (line_right - line_left) - offset_from_content,
                _ => 0.0,
            };
            if dx.abs() > 0.5 {
                for &idx in &items {
                    self.children[idx].translate(dx, 0.0);
                }
            }
        }
    }

    /// The height descendants may resolve a percentage against.
    ///
    /// CSS 2.1 §10.5 lets a percentage height resolve only when the parent's
    /// height is definite — a declared length. An `auto` height yields 0 here,
    /// which callers read as "no definite base" and fall back to `auto`.
    fn declared_height_px(&self) -> f32 {
        self.get_styled_node()
            .and_then(|s| s.value("height"))
            .map(|v| specified_height_px(v, self.percent_height_base))
            .unwrap_or(0.0)
    }

    fn calculate_block_height(&mut self) {
        let cb_h = self.percent_height_base;
        if let Some(style) = self.get_styled_node() {
            if let Some(h) = style.value("height") {
                let px = specified_height_px(h, cb_h);
                if px > 0.0 {
                    self.dimensions.content.height = px;
                }
            }
            if let Some(min_h) = style.value("min-height") {
                let px = specified_height_px(min_h, cb_h);
                if px > 0.0 {
                    self.dimensions.content.height = self.dimensions.content.height.max(px);
                }
            }
            if let Some(max_h) = style.value("max-height") {
                let px = specified_height_px(max_h, cb_h);
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
