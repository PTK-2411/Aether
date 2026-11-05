use eframe::egui::{self, Color32, FontFamily, FontId};
use std::cell::{Cell, RefCell};

// Layout runs on the UI thread, so a thread-local egui handle is enough to
// measure text with the real faces.
thread_local! {
    static FONT_CTX: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
    static FACE_AVAILABILITY: RefCell<[bool; 8]> = const { RefCell::new([true; 8]) };
    static PROBE_DONE: Cell<bool> = const { Cell::new(false) };
}

pub fn set_font_context(ctx: egui::Context) {
    FONT_CTX.with(|c| *c.borrow_mut() = Some(ctx));
}

pub fn clear_font_context() {
    FONT_CTX.with(|c| *c.borrow_mut() = None);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Face {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl Face {
    fn index(self) -> usize {
        match self {
            Face::Regular => 0,
            Face::Bold => 1,
            Face::Italic => 2,
            Face::BoldItalic => 3,
        }
    }
}

pub fn font_id(size: f32, face: Face, monospace: bool) -> FontId {
    let family = match (monospace, face) {
        (false, Face::Regular) => FontFamily::Proportional,
        (true, Face::Regular) => FontFamily::Monospace,
        (false, Face::Bold) => FontFamily::Name("bold".into()),
        (false, Face::Italic) => FontFamily::Name("italic".into()),
        (false, Face::BoldItalic) => FontFamily::Name("bold-italic".into()),
        (true, Face::Bold) => FontFamily::Name("mono-bold".into()),
        (true, Face::Italic) => FontFamily::Name("mono-italic".into()),
        (true, Face::BoldItalic) => FontFamily::Name("mono-bold-italic".into()),
    };
    if matches!(family, FontFamily::Name(_)) && !face_available(monospace, face) {
        // No bold/italic file on this system: fall back to the regular face so
        // text stays visible, and let measurement and painting agree.
        return FontId {
            size,
            family: if monospace { FontFamily::Monospace } else { FontFamily::Proportional },
        };
    }
    FontId { size, family }
}

fn face_available(monospace: bool, face: Face) -> bool {
    let idx = face.index() + if monospace { 4 } else { 0 };
    PROBE_DONE.with(|done| {
        if !done.get() {
            probe_faces();
            done.set(true);
        }
    });
    FACE_AVAILABILITY.with(|a| a.borrow()[idx])
}

/// Measures each named family once. An unregistered family yields an empty
/// galley, which we detect and remember instead of painting invisible text.
fn probe_faces() {
    let mut availability = [true; 8];
    let ctx = FONT_CTX.with(|c| c.borrow().clone());
    if let Some(ctx) = ctx {
        for (mono, base) in [(false, 0usize), (true, 4usize)] {
            for (offset, face) in [(1usize, Face::Bold), (2, Face::Italic), (3, Face::BoldItalic)] {
                let id = match font_id_raw(mono, face) {
                    Some(id) => id,
                    None => continue,
                };
                let width = ctx.fonts(|fonts| {
                    fonts
                        .layout_no_wrap("MWmm".to_string(), id, Color32::WHITE)
                        .size()
                        .x
                });
                availability[base + offset] = width > 0.0;
            }
        }
    }
    FACE_AVAILABILITY.with(|a| *a.borrow_mut() = availability);
}

fn font_id_raw(monospace: bool, face: Face) -> Option<FontId> {
    let name = match (monospace, face) {
        (false, Face::Bold) => "bold",
        (false, Face::Italic) => "italic",
        (false, Face::BoldItalic) => "bold-italic",
        (true, Face::Bold) => "mono-bold",
        (true, Face::Italic) => "mono-italic",
        (true, Face::BoldItalic) => "mono-bold-italic",
        _ => return None,
    };
    Some(FontId {
        size: 16.0,
        family: FontFamily::Name(name.into()),
    })
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
}

/// Width and height of a single text run, measured with the same `FontId` that
/// the painter uses. Falls back to a metrics estimate when no egui context is
/// installed (unit tests, offline tooling).
pub fn measure(text: &str, size: f32, face: Face, monospace: bool) -> TextMetrics {
    if text.is_empty() {
        return TextMetrics { width: 0.0, height: size * 1.35 };
    }
    let ctx = FONT_CTX.with(|c| c.borrow().clone());
    match ctx {
        Some(ctx) => {
            let id = font_id(size, face, monospace);
            let galley = ctx.fonts(|fonts| fonts.layout_no_wrap(text.to_string(), id, Color32::WHITE));
            let size_vec = galley.size();
            TextMetrics {
                width: size_vec.x,
                height: size_vec.y,
            }
        }
        None => TextMetrics {
            width: estimate_text_width(text, size),
            height: size * 1.35,
        },
    }
}

pub fn measure_width(text: &str, size: f32, face: Face, monospace: bool) -> f32 {
    measure(text, size, face, monospace).width
}

/// Advance inserted between words; taken from the real space glyph.
pub fn space_width(size: f32, face: Face, monospace: bool) -> f32 {
    let ctx = FONT_CTX.with(|c| c.borrow().clone());
    match ctx {
        Some(ctx) => {
            let id = font_id(size, face, monospace);
            ctx.fonts(|fonts| fonts.layout_no_wrap(" ".to_string(), id, Color32::WHITE).size().x)
                .max(size * 0.2)
        }
        None => (size * 0.35).max(2.0),
    }
}

/// Height of a line for the given face, used when CSS does not specify one.
pub fn line_height(size: f32, face: Face, monospace: bool) -> f32 {
    let ctx = FONT_CTX.with(|c| c.borrow().clone());
    match ctx {
        Some(ctx) => ctx.fonts(|fonts| fonts.row_height(&font_id(size, face, monospace))).max(size),
        None => size * 1.35,
    }
}

pub fn is_monospace_family(family: Option<&str>) -> bool {
    let Some(family) = family else {
        return false;
    };
    let lower = family.to_ascii_lowercase();
    for token in lower.split(',') {
        let token = token.trim();
        if token.contains("mono")
            || token.contains("consolas")
            || token.contains("courier")
            || token.contains("menlo")
            || token.contains("code")
        {
            return true;
        }
    }
    false
}

/// Rough per-character fallback used before the font atlas is available.
pub fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let mut width = 0.0;
    for c in text.chars() {
        let factor = match c {
            ' ' => 0.35,
            'i' | 'l' | 'j' | '!' | '.' | ',' | ':' | ';' | '\'' | '|' => 0.30,
            'f' | 'r' | 't' | '(' | ')' | '[' | ']' | '-' => 0.40,
            'm' | 'w' | 'M' | 'W' => 0.95,
            'A'..='Z' => 0.75,
            '0'..='9' => 0.62,
            _ => 0.60,
        };
        width += factor * font_size;
    }
    width.max(2.0)
}
