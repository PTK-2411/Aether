use crate::css::{CssColor, Value};
use crate::layout::{BoxType, LayoutBox, Rect};
use eframe::egui::{self, Color32, FontId, Pos2, Stroke, StrokeKind, Vec2};

#[derive(Clone, Debug)]
pub enum DisplayItem {
    SolidColor {
        rect: Rect,
        color: Color32,
        corner_radius: f32,
    },
    Border {
        rect: Rect,
        color: Color32,
        width: f32,
        corner_radius: f32,
    },
    Text {
        pos: Pos2,
        text: String,
        color: Color32,
        size: f32,
        #[allow(dead_code)]
        bold: bool,
        #[allow(dead_code)]
        is_link: bool,
        #[allow(dead_code)]
        href: Option<String>,
        underline: bool,
    },
    ButtonBox {
        rect: Rect,
        label: String,
        bg_color: Color32,
        text_color: Color32,
    },
    InputBox {
        rect: Rect,
        text: String,
        placeholder: String,
    },
}

#[derive(Clone, Debug)]
pub struct InteractiveLink {
    pub rect: Rect,
    pub href: String,
}

pub struct DisplayList {
    pub items: Vec<DisplayItem>,
    pub links: Vec<InteractiveLink>,
}

impl DisplayList {
    pub fn new() -> Self {
        DisplayList {
            items: Vec::new(),
            links: Vec::new(),
        }
    }
}

pub fn build_display_list(layout_root: &LayoutBox) -> DisplayList {
    let mut list = DisplayList::new();
    render_layout_box(&mut list, layout_root);
    list
}

fn render_layout_box(list: &mut DisplayList, layout_box: &LayoutBox) {
    render_background(list, layout_box);
    render_borders(list, layout_box);

    match &layout_box.box_type {
        BoxType::TextFragment { text, styled_node } => {
            let font_size = styled_node.font_size(None);
            let color = css_to_egui_color(styled_node.color(None));
            let bold = styled_node.is_bold(None);

            let (is_link, href) = get_link_info(styled_node);
            let underline = is_link || styled_node.value("text-decoration").map_or(false, |v| match v {
                Value::Keyword(k) => k == "underline",
                _ => false,
            });

            let final_color = if is_link {
                Color32::from_rgb(26, 115, 232)
            } else {
                color
            };

            let rect = layout_box.dimensions.content;
            let pos = Pos2::new(rect.x, rect.y);

            list.items.push(DisplayItem::Text {
                pos,
                text: text.clone(),
                color: final_color,
                size: font_size,
                bold,
                is_link,
                href: href.clone(),
                underline,
            });

            if let Some(target) = href {
                list.links.push(InteractiveLink {
                    rect,
                    href: target,
                });
            }
        }
        BoxType::InlineNode(styled_node) => {
            let tag = styled_node.node.tag_name().unwrap_or("");
            if tag == "button" {
                let rect = layout_box.dimensions.border_box();
                let label = styled_node.node.inner_text();
                let bg_color = css_to_egui_color(styled_node.background_color());
                let text_color = css_to_egui_color(styled_node.color(None));
                list.items.push(DisplayItem::ButtonBox {
                    rect,
                    label,
                    bg_color,
                    text_color,
                });
            } else if tag == "input" {
                let rect = layout_box.dimensions.border_box();
                let placeholder = styled_node
                    .node
                    .get_attribute("placeholder")
                    .unwrap_or("")
                    .to_string();
                let value = styled_node
                    .node
                    .get_attribute("value")
                    .unwrap_or("")
                    .to_string();
                list.items.push(DisplayItem::InputBox {
                    rect,
                    text: value,
                    placeholder,
                });
            }
        }
        _ => {}
    }

    for child in &layout_box.children {
        render_layout_box(list, child);
    }
}

fn render_background(list: &mut DisplayList, layout_box: &LayoutBox) {
    if let Some(style) = layout_box.get_styled_node() {
        let bg_color = style.background_color();
        if bg_color.a > 0 {
            let corner_radius = parse_corner_radius(style);
            list.items.push(DisplayItem::SolidColor {
                rect: layout_box.dimensions.border_box(),
                color: css_to_egui_color(bg_color),
                corner_radius,
            });
        }
    }
}

fn render_borders(list: &mut DisplayList, layout_box: &LayoutBox) {
    if let Some(style) = layout_box.get_styled_node() {
        let border_w = layout_box.dimensions.border.left;
        if border_w > 0.0 {
            let border_color = match style.value("border-left-color") {
                Some(Value::ColorValue(c)) => *c,
                _ => CssColor::new(218, 220, 224, 255),
            };
            let corner_radius = parse_corner_radius(style);
            list.items.push(DisplayItem::Border {
                rect: layout_box.dimensions.border_box(),
                color: css_to_egui_color(border_color),
                width: border_w,
                corner_radius,
            });
        }
    }
}

fn parse_corner_radius(style: &crate::style::StyledNode) -> f32 {
    match style.value("border-radius") {
        Some(Value::Length(px, _)) => *px,
        Some(Value::Number(n)) => *n,
        _ => 0.0,
    }
}

fn get_link_info(styled_node: &crate::style::StyledNode) -> (bool, Option<String>) {
    if styled_node.node.tag_name() == Some("a") {
        if let Some(href) = styled_node.node.get_attribute("href") {
            return (true, Some(href.to_string()));
        }
        return (true, None);
    }
    (false, None)
}

pub fn css_to_egui_color(c: CssColor) -> Color32 {
    Color32::from_rgba_premultiplied(c.r, c.g, c.b, c.a)
}

/// Paints the entire display list onto egui::Painter with offset and zoom.
pub fn paint_display_list(
    painter: &egui::Painter,
    list: &DisplayList,
    offset: Vec2,
    zoom: f32,
) {
    for item in &list.items {
        match item {
            DisplayItem::SolidColor { rect, color, corner_radius } => {
                let screen_rect = egui::Rect::from_min_size(
                    Pos2::new(rect.x * zoom + offset.x, rect.y * zoom + offset.y),
                    Vec2::new(rect.width * zoom, rect.height * zoom),
                );
                painter.rect_filled(screen_rect, *corner_radius * zoom, *color);
            }
            DisplayItem::Border { rect, color, width, corner_radius } => {
                let screen_rect = egui::Rect::from_min_size(
                    Pos2::new(rect.x * zoom + offset.x, rect.y * zoom + offset.y),
                    Vec2::new(rect.width * zoom, rect.height * zoom),
                );
                painter.rect_stroke(
                    screen_rect,
                    *corner_radius * zoom,
                    Stroke::new(*width * zoom, *color),
                    StrokeKind::Inside,
                );
            }
            DisplayItem::Text { pos, text, color, size, bold: _, is_link: _, href: _, underline } => {
                let screen_pos = Pos2::new(pos.x * zoom + offset.x, pos.y * zoom + offset.y);
                let font_id = FontId::proportional(*size * zoom);

                let text_pos = Pos2::new(screen_pos.x, screen_pos.y);
                painter.text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    text,
                    font_id.clone(),
                    *color,
                );

                if *underline {
                    let galley = painter.layout_no_wrap(text.clone(), font_id, *color);
                    let line_y = screen_pos.y + galley.size().y + 1.0;
                    let p1 = Pos2::new(screen_pos.x, line_y);
                    let p2 = Pos2::new(screen_pos.x + galley.size().x, line_y);
                    painter.line_segment([p1, p2], Stroke::new(1.0 * zoom, *color));
                }
            }
            DisplayItem::ButtonBox { rect, label, bg_color, text_color } => {
                let screen_rect = egui::Rect::from_min_size(
                    Pos2::new(rect.x * zoom + offset.x, rect.y * zoom + offset.y),
                    Vec2::new(rect.width * zoom, rect.height * zoom),
                );
                painter.rect_filled(screen_rect, 4.0 * zoom, *bg_color);
                painter.rect_stroke(screen_rect, 4.0 * zoom, Stroke::new(1.0_f32, Color32::from_rgb(200, 200, 200)), StrokeKind::Inside);
                painter.text(
                    screen_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(14.0 * zoom),
                    *text_color,
                );
            }
            DisplayItem::InputBox { rect, text, placeholder } => {
                let screen_rect = egui::Rect::from_min_size(
                    Pos2::new(rect.x * zoom + offset.x, rect.y * zoom + offset.y),
                    Vec2::new(rect.width * zoom, rect.height * zoom),
                );
                painter.rect_filled(screen_rect, 4.0 * zoom, Color32::WHITE);
                painter.rect_stroke(screen_rect, 4.0 * zoom, Stroke::new(1.0_f32, Color32::from_rgb(200, 200, 200)), StrokeKind::Inside);
                let display_str = if text.is_empty() { placeholder } else { text };
                let color = if text.is_empty() {
                    Color32::from_rgb(150, 150, 150)
                } else {
                    Color32::from_rgb(32, 33, 36)
                };
                painter.text(
                    Pos2::new(screen_rect.min.x + 8.0, screen_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    display_str,
                    FontId::proportional(13.0 * zoom),
                    color,
                );
            }
        }
    }
}
