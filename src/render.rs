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
        bold: bool,
        italic: bool,
        monospace: bool,
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
    Image {
        rect: Rect,
        src: String,
        alt: String,
    },
}

#[derive(Clone, Debug)]
pub struct InteractiveLink {
    pub rect: Rect,
    pub href: String,
    /// True when the link lives in a `position: fixed` subtree, so hit-testing
    /// has to use the viewport-shifted origin instead of the document one.
    pub fixed: bool,
}

pub struct DisplayList {
    pub items: Vec<DisplayItem>,
    /// Painted in viewport coordinates: the document scroll doesn't move them.
    pub fixed: Vec<DisplayItem>,
    pub links: Vec<InteractiveLink>,
}

impl DisplayList {
    pub fn new() -> Self {
        DisplayList {
            items: Vec::new(),
            fixed: Vec::new(),
            links: Vec::new(),
        }
    }
}

pub fn build_display_list(layout_root: &LayoutBox) -> DisplayList {
    let mut list = DisplayList::new();
    render_box(&mut list, layout_root, false);
    list
}

fn is_fixed_box(layout_box: &LayoutBox) -> bool {
    matches!(
        layout_box.get_styled_node().and_then(|s| s.value("position")),
        Some(Value::Keyword(k)) if k == "fixed"
    )
}

/// Appends `layout_box` to `list`. A `position: fixed` subtree is diverted into
/// `DisplayList::fixed` so the paint pass can pin it to the viewport while the
/// rest of the document scrolls underneath it.
fn render_box(list: &mut DisplayList, layout_box: &LayoutBox, in_fixed: bool) {
    let mine = in_fixed || is_fixed_box(layout_box);
    if mine && !in_fixed {
        let mut pinned = DisplayList::new();
        render_box_inner(&mut pinned, layout_box, true);
        list.fixed.append(&mut pinned.items);
        list.links.append(&mut pinned.links);
        return;
    }
    render_box_inner(list, layout_box, mine);
}

/// Sites such as search engines hide the page with `opacity:0` plus a reveal
/// animation that only becomes visible once the animation runs. Aether paints a
/// single static frame, so the pre-animation state is the wrong one: a box
/// carrying a named animation is drawn at the state its animation ends in.
/// `visibility` keeps its strict rule because hidden-then-revealed-on-hover
/// menus rely on it and have no timed end state to reach.
fn reveal_animation_pending(styled_node: &crate::style::StyledNode) -> bool {
    let named = |v: Option<&Value>| match v {
        Some(Value::Keyword(k)) => k != "none",
        Some(Value::StringLiteral(s)) => !s.trim().is_empty() && s.trim() != "none",
        _ => false,
    };
    named(styled_node.value("animation-name")) || named(styled_node.value("animation"))
}

fn render_box_inner(list: &mut DisplayList, layout_box: &LayoutBox, in_fixed: bool) {
    if let Some(styled_node) = layout_box.get_styled_node() {
        if let Some(Value::Keyword(v)) = styled_node.value("visibility") {
            if v == "hidden" || v == "collapse" {
                return;
            }
        }
        if let Some(Value::Number(op)) = styled_node.value("opacity") {
            if op <= &0.0 && !reveal_animation_pending(styled_node) {
                return;
            }
        }
    }

    let mb = layout_box.dimensions.margin_box();
    if mb.y + mb.height < -50.0 || mb.x + mb.width < -100.0 {
        return;
    }

    render_background(list, layout_box);
    render_borders(list, layout_box);

    match &layout_box.box_type {
        BoxType::TextFragment { text, styled_node } => {
            let font_size = styled_node.font_size(None);
            let color = css_to_egui_color(styled_node.color(None));
            let bold = styled_node.is_bold(None);
            let italic = styled_node.is_italic(None);
            let family = match styled_node.value("font-family") {
                Some(Value::Keyword(k)) => Some(k.as_str()),
                Some(Value::StringLiteral(s)) => Some(s.as_str()),
                _ => None,
            };
            let monospace = crate::text_metrics::is_monospace_family(family);

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
                italic,
                monospace,
                is_link,
                href: href.clone(),
                underline,
            });

            if let Some(target) = href {
                list.links.push(InteractiveLink {
                    rect,
                    href: target,
                    fixed: in_fixed,
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
                return;
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
                return;
            }
        }
        BoxType::ImageNode { src, alt, .. } => {
            list.items.push(DisplayItem::Image {
                rect: layout_box.dimensions.content,
                src: src.clone(),
                alt: alt.clone(),
            });
        }
        _ => {}
    }

    let mut normal = Vec::new();
    let mut below = Vec::new();
    let mut above = Vec::new();
    for (idx, child) in layout_box.children.iter().enumerate() {
        match stacking_layer(child) {
            Some((true, z)) => below.push((z, idx)),
            Some((false, z)) => above.push((z, idx)),
            None => normal.push((0, idx)),
        }
    }

    if below.is_empty() && above.is_empty() {
        for (_, idx) in normal {
            render_box(list, &layout_box.children[idx], in_fixed);
        }
        return;
    }

    above.sort_by_key(|(z, idx)| (*z, *idx));
    below.sort_by_key(|(z, idx)| (*z, *idx));

    for (_, idx) in below {
        render_box(list, &layout_box.children[idx], in_fixed);
    }
    for (_, idx) in normal {
        render_box(list, &layout_box.children[idx], in_fixed);
    }
    for (_, idx) in above {
        render_box(list, &layout_box.children[idx], in_fixed);
    }
}

/// Paint layer of a positioned box: `None` stays in normal flow order,
/// otherwise `(below_normal_flow, z_index)`.
fn stacking_layer(layout_box: &LayoutBox) -> Option<(bool, i32)> {
    let style = layout_box.get_styled_node()?;
    let positioned = match style.value("position") {
        Some(Value::Keyword(k)) => matches!(k.as_str(), "relative" | "absolute" | "fixed" | "sticky"),
        _ => false,
    };
    if !positioned {
        return None;
    }
    let z = match style.value("z-index") {
        Some(Value::Number(n)) => *n as i32,
        _ => 0,
    };
    if z < 0 {
        Some((true, z))
    } else {
        Some((false, z))
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
    if let Some(ref href) = styled_node.link_url {
        return (true, Some(href.clone()));
    }
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

/// Paints the entire display list onto egui::Painter with offset, zoom, and optional image textures.
/// Paints the document at `offset` and the pinned `position: fixed` layer at
/// `fixed_offset`, which is the viewport origin rather than the scrolled one.
pub fn paint_display_list(
    painter: &egui::Painter,
    list: &DisplayList,
    offset: Vec2,
    fixed_offset: Vec2,
    zoom: f32,
    textures: Option<&std::collections::HashMap<String, egui::TextureHandle>>,
) {
    paint_items(painter, &list.items, offset, zoom, textures);
    paint_items(painter, &list.fixed, fixed_offset, zoom, textures);
}

fn paint_items(
    painter: &egui::Painter,
    items: &[DisplayItem],
    offset: Vec2,
    zoom: f32,
    textures: Option<&std::collections::HashMap<String, egui::TextureHandle>>,
) {
    // A long article can hold tens of thousands of boxes; skipping what the
    // viewport can't show keeps scrolling smooth. The margin keeps partially
    // visible runs from flickering at the edges.
    let clip = painter.clip_rect().expand(32.0);
    let screen = |rect: Rect| {
        egui::Rect::from_min_size(
            Pos2::new(rect.x * zoom + offset.x, rect.y * zoom + offset.y),
            Vec2::new(rect.width * zoom, rect.height * zoom),
        )
    };

    for item in items {
        match item {
            DisplayItem::SolidColor { rect, color, corner_radius } => {
                let screen_rect = screen(*rect);
                if !screen_rect.is_positive() || !screen_rect.intersects(clip) {
                    continue;
                }
                painter.rect_filled(screen_rect, *corner_radius * zoom, *color);
            }
            DisplayItem::Border { rect, color, width, corner_radius } => {
                let screen_rect = screen(*rect);
                if !screen_rect.is_positive() || !screen_rect.intersects(clip) {
                    continue;
                }
                painter.rect_stroke(
                    screen_rect,
                    *corner_radius * zoom,
                    Stroke::new(*width * zoom, *color),
                    StrokeKind::Inside,
                );
            }
            DisplayItem::Text { pos, text, color, size, bold, italic, monospace, is_link: _, href: _, underline } => {
                let screen_pos = Pos2::new(pos.x * zoom + offset.x, pos.y * zoom + offset.y);
                let run_box = egui::Rect::from_min_size(
                    screen_pos,
                    Vec2::new(
                        text.chars().count() as f32 * size * 0.7 * zoom,
                        size * 1.8 * zoom,
                    ),
                );
                if !run_box.intersects(clip) {
                    continue;
                }
                let face = match (*bold, *italic) {
                    (true, true) => crate::text_metrics::Face::BoldItalic,
                    (true, false) => crate::text_metrics::Face::Bold,
                    (false, true) => crate::text_metrics::Face::Italic,
                    (false, false) => crate::text_metrics::Face::Regular,
                };
                let font_id = crate::text_metrics::font_id(*size * zoom, face, *monospace);

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
            DisplayItem::Image { rect, src, alt } => {
                let screen_rect = screen(*rect);
                if !screen_rect.is_positive() || !screen_rect.intersects(clip) {
                    continue;
                }

                let mut drawn = false;
                if let Some(tex_map) = textures {
                    if let Some(texture) = tex_map.get(src) {
                        painter.image(
                            texture.id(),
                            screen_rect,
                            egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        drawn = true;
                    }
                }

                if !drawn {
                    painter.rect_filled(screen_rect, 6.0 * zoom, Color32::from_rgb(240, 242, 245));
                    painter.rect_stroke(
                        screen_rect,
                        6.0 * zoom,
                        Stroke::new(1.0 * zoom, Color32::from_rgb(215, 218, 224)),
                        StrokeKind::Inside,
                    );

                    let center = screen_rect.center();
                    let icon_r = (screen_rect.height() * 0.18).clamp(8.0, 22.0) * zoom;
                    let icon_center = if alt.is_empty() {
                        center
                    } else {
                        Pos2::new(center.x, center.y - 8.0 * zoom)
                    };
                    draw_image_placeholder_icon(painter, icon_center, icon_r, Color32::from_rgb(140, 146, 156));

                    if !alt.is_empty() && screen_rect.height() > 36.0 {
                        let preview_alt: String = if alt.chars().count() > 30 {
                            alt.chars().take(28).collect::<String>() + "..."
                        } else {
                            alt.clone()
                        };
                        painter.text(
                            Pos2::new(center.x, center.y + 10.0 * zoom),
                            egui::Align2::CENTER_CENTER,
                            preview_alt,
                            FontId::proportional(11.0 * zoom),
                            Color32::from_rgb(115, 120, 130),
                        );
                    }
                }
            }
            DisplayItem::ButtonBox { rect, label, bg_color, text_color } => {
                let screen_rect = screen(*rect);
                if !screen_rect.is_positive() || !screen_rect.intersects(clip) {
                    continue;
                }
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
                let screen_rect = screen(*rect);
                if !screen_rect.is_positive() || !screen_rect.intersects(clip) {
                    continue;
                }
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

fn draw_image_placeholder_icon(painter: &egui::Painter, center: Pos2, r: f32, color: Color32) {
    let frame = egui::Rect::from_center_size(center, Vec2::new(r * 2.2, r * 1.6));
    painter.rect_stroke(frame, 2.0_f32, Stroke::new(1.2_f32, color), StrokeKind::Inside);
    let sun_pos = Pos2::new(frame.right() - r * 0.55, frame.top() + r * 0.45);
    painter.circle_filled(sun_pos, r * 0.2, color);
    let m1 = Pos2::new(frame.left() + r * 0.25, frame.bottom() - 1.0);
    let m2 = Pos2::new(frame.left() + r * 0.95, frame.top() + r * 0.7);
    let m3 = Pos2::new(frame.left() + r * 1.65, frame.bottom() - 1.0);
    painter.add(egui::Shape::convex_polygon(vec![m1, m2, m3], color, Stroke::NONE));
}
