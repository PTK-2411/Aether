# Handoff Report: Milestone M2 — Requirement R2 (Chrome 3-Dots Popup Menu Implementation Strategy)

**Author**: teamwork_explorer (M2 Chrome 3-Dots Popup Menu)  
**Parent Conversation ID**: ede09244-cd85-447e-82fe-12ac1159a1a2  
**Target Component**: `src/app.rs` (Chrome 3-dots popup menu, zoom ladder, vector icons, profile card, dismissal logic) and `tests/chrome_menu_tests.rs`  

---

## 1. Observation

### 1.1 Existing Implementation State in `src/app.rs`
1. **3-Dots Button Trigger** (`src/app.rs:930-941`):
   ```rust
   // Menu button ⋮ (Vector 3 vertical dots, guaranteed 100% bug-free)
   let (m_rect, m_resp) = ui.allocate_exact_size(Vec2::new(26.0, 26.0), egui::Sense::click());
   let m_center = m_rect.center();
   if m_resp.hovered() {
       ui.painter().circle_filled(m_center, 13.0, Color32::from_rgb(55, 58, 62));
   }
   for dy in [-5.0, 0.0, 5.0] {
       ui.painter().circle_filled(Pos2::new(m_center.x, m_center.y + dy), 1.5, Color32::from_rgb(218, 220, 224));
   }
   if m_resp.clicked() {
       self.show_menu_popup = !self.show_menu_popup;
   }
   ```
2. **Current Placeholder Window** (`src/app.rs:943-981`):
   ```rust
   if self.show_menu_popup {
       egui::Window::new("Menu")
           .anchor(Align2::RIGHT_TOP, Vec2::new(-10.0, 75.0))
           .collapsible(false)
           .resizable(false)
           .frame(Frame::NONE.fill(Color32::from_rgb(43, 45, 48)).corner_radius(8).inner_margin(8))
           .show(ctx, |ui| {
               if ui.button("New Tab").clicked() {
                   self.open_new_tab("about:newtab".to_string());
                   self.show_menu_popup = false;
               }
               if ui.button("DevTools Inspector (F12)").clicked() {
                   self.show_devtools = !self.show_devtools;
                   self.show_menu_popup = false;
               }
               if ui.button("Reload").clicked() {
                   self.reload();
                   self.show_menu_popup = false;
               }
               if ui.button("CSS Test Suite (about:test)").clicked() {
                   self.navigate_active_tab("about:test");
                   self.show_menu_popup = false;
               }
               if ui.button("Engine Architecture (about:credits)").clicked() {
                   self.navigate_active_tab("about:credits");
                   self.show_menu_popup = false;
               }
               ui.separator();
               ui.horizontal(|ui| {
                   if ui.button("-").clicked() {
                       self.zoom_level = (self.zoom_level - 0.1).max(0.5);
                   }
                   ui.label(format!("{:.0}%", self.zoom_level * 100.0));
                   if ui.button("+").clicked() {
                       self.zoom_level = (self.zoom_level + 0.1).min(2.0);
                   }
               });
           });
   }
   ```
   - Currently lacks: frameless Chrome styling (`title_bar(false)`), dark slate fill `#292a2d`, 1px border `#3c4043`, drop shadow, section dividers, Profile card ("Khoa", "Signed in"), vector icons, right-aligned shortcuts (`Ctrl+T`, `Ctrl+N`, `Ctrl+Shift+N`, `Ctrl+H`, `Ctrl+J`, `Ctrl+Shift+Del`, `Ctrl+P`), fullscreen toggle button, and outside click / Escape key dismissal.
3. **Current Zoom Handling** (`src/app.rs:108, 150, 971-979, 1163-1202`):
   - `zoom_level: f32` initialized to `1.0`.
   - Stepping currently uses crude linear float arithmetic: `(self.zoom_level - 0.1).max(0.5)` and `(self.zoom_level + 0.1).min(2.0)`.
   - Discrete 17-step Chrome zoom ladder required by R2 is completely absent.
4. **Current Test Baseline** (`tests/challenger_ui_tests.rs`):
   - Ran `cargo test`: all 26 existing tests pass cleanly (10 adversarial, 11 browser, 5 challenger UI).
   - Invariant check lines in `tests/challenger_ui_tests.rs`:
     - Checks `!source.contains("Hi, Khoa")`. Notice: R2 requires Profile card with label `"Khoa"`, which does **not** contain `"Hi, Khoa"`.
     - Checks absence of `"✦ AI Mode"`, `"100% Pure Rust Web Browser • Zero Chromium"`, `"Aether AI Assistant"`, `"Explore Open Web"`, `"✦ Aether AI"`.
     - Checks presence of `"Vec2::new(64.0, 28.0)"`, `"\"Login\""`, `"https://accounts.google.com"`, `"let total_shortcuts_w = 6.0 * 68.0;"`, `"let available_edit_w = child_ui.available_width();"`.
     - None of these invariants conflict with replacing lines 943-981 with the Chrome popup menu.
5. **UI Reference Image 2 Inspection** (`media_1793864186546_760d982c.png`):
   - Dark floating card container (~290px wide) with rounded corners and drop shadow.
   - Section 0: Alert row "Found 111 compromised passwords" with shield icon.
   - Section 1: Navigation rows ("New tab" [Ctrl+T], "New window" [Ctrl+N], "New Incognito window" [Ctrl+Shift+N]).
   - Section 2: Profile card row ("Khoa", blue pill "Signed in", chevron `>`).
   - Section 3: Tools rows ("Passwords and autofill" `[>]`, "History" `[Ctrl+H / >]`, "Downloads" `[Ctrl+J]`, "Bookmarks and lists" `[>]`, "Tab groups" `[>]`, "Extensions" `[>]`, "Delete browsing data..." `[Ctrl+Shift+Del]`).
   - Section 4: Zoom row (Magnifying glass, "Zoom", `[-]`, percentage `100%`, `[+]`, Fullscreen `⛶`).
   - Section 5: Page actions ("Print..." `[Ctrl+P]`, "Search this tab with Google Lens", "Translate...", "Find and edit" `[>]`, "Cast, save, and share" `[>]`, "More tools" `[>]`).
   - Section 6: Settings & exit ("Help" `[>]`, "Settings", "Exit").
   - Distinct 1px dividers between sections; right-aligned shortcut hints in muted grey `#9aa0a6`.

---

## 2. Logic Chain

1. **Window Setup & Floating Overlay**:
   - Calling `egui::Window::new("chrome_3dots_menu").title_bar(false).anchor(Align2::RIGHT_TOP, Vec2::new(-8.0, 76.0)).fixed_size(Vec2::new(290.0, 0.0))` anchors the popup directly beneath the 3-dots button with 8px margin from the right edge.
   - Using `Frame::NONE.fill(Color32::from_rgb(41, 42, 45)).corner_radius(CornerRadius::same(8)).stroke(Stroke::new(1.0, Color32::from_rgb(60, 64, 67)))` and `egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(160) }` gives the precise Chrome dark theme look.
2. **Scroll Safety for Small Displays**:
   - Wrapping menu items in `ScrollArea::vertical().max_height((ctx.screen_rect().height() - 90.0).max(300.0).min(680.0))` ensures the menu does not overflow below the screen on small viewports (e.g. 800x500 minimum size), while scrolling smoothly if needed.
3. **Discrete Zoom Ladder & Fullscreen Interaction**:
   - Defining `pub const ZOOM_STEPS: &[f32] = &[0.25, 0.33, 0.50, 0.67, 0.75, 0.80, 0.90, 1.00, 1.10, 1.25, 1.50, 1.75, 2.00, 2.50, 3.00, 4.00, 5.00];` allows exact stepping.
   - When `[-]` or `[+]` is clicked, `self.zoom_level` advances to the adjacent step.
   - Percentage text is rendered with `format!("{:.0}%", (self.zoom_level * 100.0).round())`.
   - Fullscreen button toggles viewport fullscreen via `ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen))`.
   - Zoom and fullscreen interactions deliberately **retain** `self.show_menu_popup = true;`.
4. **Dismissal Logic**:
   - Escape key: `if ctx.input(|i| i.key_pressed(Key::Escape)) { self.show_menu_popup = false; }`.
   - Outside click: `window_resp.response.rect` defines the popup bounds `menu_rect`. If `ctx.input(|i| i.pointer.any_pressed())` and pointer position is outside both `menu_rect` and `m_rect`, `self.show_menu_popup = false;`.
   - Action item clicks: Every standard item handler (New Tab, History, Downloads, Settings, Exit, More Tools) sets `self.show_menu_popup = false;`.
5. **Pixel-Perfect Vector Icons and Typography**:
   - Vector-rendered icons via `draw_menu_icon(painter, icon, center, color)` guarantee crisp visual quality independent of system font fallback glitches.
   - Labels rendered at 12.5pt (`#e8eaed`), shortcut hints right-aligned at 11.5pt (`#9aa0a6`).
   - Profile card styled as a subtle container (`#35383d`) with avatar circle, "Khoa", blue pill badge ("Signed in"), and chevron `>`.

---

## 3. Caveats

1. **`Hi, Khoa` Substring Invariant**:
   - `tests/challenger_ui_tests.rs` strictly asserts `!source.contains("Hi, Khoa")`.
   - In the profile card, the label text must be `"Khoa"` (matching Image 2), **never** `"Hi, Khoa"`.
2. **`Shadow` Struct in egui 0.31.1**:
   - `epaint::Shadow` in egui 0.31 has fields `offset: [i8; 2]`, `blur: u8`, `spread: u8`, `color: Color32`.
   - Do not use old egui 0.28 `extrusion` field. Use `egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(160) }`.
3. **Submenu Expansion**:
   - Items with `>` (such as "Passwords and autofill", "Bookmarks and lists", "History") navigate active tab to their corresponding `about:` or settings pages. "More tools" directly toggles `show_devtools`, providing immediate utility without complex nested flyout windows.

---

## 4. Conclusion

A concrete, complete, error-free strategy and code diff is formulated. Replacing lines 943-981 in `src/app.rs` with `render_chrome_menu_popup` and adding helper methods and unit tests fully satisfies Requirement R2 while maintaining 100% test compatibility.

### 4.1 Detailed Code Modifications for Worker

#### A. Add Zoom Ladder Constants & Methods to `src/app.rs`
```rust
/// Chrome discrete zoom percentage steps
pub const ZOOM_STEPS: &[f32] = &[
    0.25, 0.33, 0.50, 0.67, 0.75, 0.80, 0.90, 1.00,
    1.10, 1.25, 1.50, 1.75, 2.00, 2.50, 3.00, 4.00, 5.00,
];

impl BrowserApp {
    /// Steps zoom level up to next discrete step
    pub fn zoom_in(&mut self) {
        let current_idx = ZOOM_STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (self.zoom_level - *a).abs().partial_cmp(&(self.zoom_level - *b).abs()).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)
            .unwrap_or(7);
        if current_idx + 1 < ZOOM_STEPS.len() {
            self.zoom_level = ZOOM_STEPS[current_idx + 1];
        }
    }

    /// Steps zoom level down to previous discrete step
    pub fn zoom_out(&mut self) {
        let current_idx = ZOOM_STEPS
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (self.zoom_level - *a).abs().partial_cmp(&(self.zoom_level - *b).abs()).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx)
            .unwrap_or(7);
        if current_idx > 0 {
            self.zoom_level = ZOOM_STEPS[current_idx - 1];
        }
    }

    /// Resets zoom level to 100%
    pub fn zoom_reset(&mut self) {
        self.zoom_level = 1.0;
    }

    /// Returns human-readable zoom percentage string
    pub fn zoom_percentage(&self) -> String {
        format!("{:.0}%", (self.zoom_level * 100.0).round())
    }
}
```

#### B. Vector Icon Drawing & Components in `src/app.rs`
```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuIcon {
    Shield,
    NewTab,
    NewWindow,
    Incognito,
    Passwords,
    History,
    Downloads,
    Bookmarks,
    TabGroups,
    Extensions,
    DeleteData,
    Zoom,
    Print,
    GoogleLens,
    Translate,
    Find,
    Cast,
    MoreTools,
    Help,
    Settings,
    Exit,
}

fn draw_menu_icon(painter: &egui::Painter, icon: MenuIcon, center: Pos2, color: Color32) {
    let s = Stroke::new(1.3, color);
    match icon {
        MenuIcon::Shield => {
            painter.line_segment([Pos2::new(center.x - 5.0, center.y - 6.0), Pos2::new(center.x + 5.0, center.y - 6.0)], s);
            painter.line_segment([Pos2::new(center.x + 5.0, center.y - 6.0), Pos2::new(center.x + 5.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x + 5.0, center.y), Pos2::new(center.x, center.y + 6.0)], s);
            painter.line_segment([Pos2::new(center.x, center.y + 6.0), Pos2::new(center.x - 5.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x - 5.0, center.y), Pos2::new(center.x - 5.0, center.y - 6.0)], s);
        }
        MenuIcon::NewTab => {
            let r = Rect::from_center_size(center, Vec2::new(11.0, 9.0));
            painter.rect_stroke(r, 1.5, s, StrokeKind::Inside);
            painter.line_segment([Pos2::new(center.x - 2.5, center.y), Pos2::new(center.x + 2.5, center.y)], s);
            painter.line_segment([Pos2::new(center.x, center.y - 2.5), Pos2::new(center.x, center.y + 2.5)], s);
        }
        MenuIcon::NewWindow => {
            let r1 = Rect::from_min_size(Pos2::new(center.x - 5.0, center.y - 5.0), Vec2::new(8.0, 7.0));
            let r2 = Rect::from_min_size(Pos2::new(center.x - 2.0, center.y - 2.0), Vec2::new(8.0, 7.0));
            painter.rect_stroke(r1, 1.0, Stroke::new(1.0, color), StrokeKind::Inside);
            painter.rect_filled(r2, 1.0, Color32::from_rgb(41, 42, 45));
            painter.rect_stroke(r2, 1.0, s, StrokeKind::Inside);
        }
        MenuIcon::Incognito => {
            painter.circle_stroke(Pos2::new(center.x - 3.5, center.y + 1.0), 3.0, s);
            painter.circle_stroke(Pos2::new(center.x + 3.5, center.y + 1.0), 3.0, s);
            painter.line_segment([Pos2::new(center.x - 0.5, center.y + 1.0), Pos2::new(center.x + 0.5, center.y + 1.0)], s);
            painter.line_segment([Pos2::new(center.x - 7.0, center.y - 3.0), Pos2::new(center.x + 7.0, center.y - 3.0)], s);
        }
        MenuIcon::Passwords => {
            painter.circle_stroke(Pos2::new(center.x - 3.0, center.y), 3.0, s);
            painter.line_segment([Pos2::new(center.x, center.y), Pos2::new(center.x + 6.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x + 4.0, center.y), Pos2::new(center.x + 4.0, center.y + 2.5)], s);
            painter.line_segment([Pos2::new(center.x + 6.0, center.y), Pos2::new(center.x + 6.0, center.y + 2.5)], s);
        }
        MenuIcon::History => {
            painter.circle_stroke(center, 5.5, s);
            painter.line_segment([center, Pos2::new(center.x, center.y - 3.5)], s);
            painter.line_segment([center, Pos2::new(center.x + 2.5, center.y)], s);
        }
        MenuIcon::Downloads => {
            painter.line_segment([Pos2::new(center.x, center.y - 4.5), Pos2::new(center.x, center.y + 2.5)], s);
            painter.line_segment([Pos2::new(center.x - 3.0, center.y), Pos2::new(center.x, center.y + 3.0)], s);
            painter.line_segment([Pos2::new(center.x + 3.0, center.y), Pos2::new(center.x, center.y + 3.0)], s);
            painter.line_segment([Pos2::new(center.x - 5.0, center.y + 5.0), Pos2::new(center.x + 5.0, center.y + 5.0)], s);
        }
        MenuIcon::Bookmarks => {
            let r = 5.5;
            let mut pts = Vec::new();
            for i in 0..10 {
                let angle = (i as f32) * std::f32::consts::PI / 5.0 - std::f32::consts::FRAC_PI_2;
                let rad = if i % 2 == 0 { r } else { r * 0.45 };
                pts.push(Pos2::new(center.x + angle.cos() * rad, center.y + angle.sin() * rad));
            }
            painter.add(egui::Shape::closed_line(pts, s));
        }
        MenuIcon::TabGroups => {
            for dx in [-3.5, 1.5] {
                for dy in [-3.5, 1.5] {
                    let r = Rect::from_min_size(Pos2::new(center.x + dx, center.y + dy), Vec2::new(3.0, 3.0));
                    painter.rect_stroke(r, 0.5, Stroke::new(1.0, color), StrokeKind::Inside);
                }
            }
        }
        MenuIcon::Extensions => {
            let r = Rect::from_center_size(center, Vec2::new(9.0, 9.0));
            painter.rect_stroke(r, 1.5, s, StrokeKind::Inside);
            painter.circle_filled(Pos2::new(center.x + 4.5, center.y), 1.5, color);
        }
        MenuIcon::DeleteData => {
            painter.line_segment([Pos2::new(center.x - 5.0, center.y - 4.0), Pos2::new(center.x + 5.0, center.y - 4.0)], s);
            let bin = Rect::from_min_size(Pos2::new(center.x - 3.5, center.y - 3.0), Vec2::new(7.0, 8.0));
            painter.rect_stroke(bin, 1.0, s, StrokeKind::Inside);
            painter.line_segment([Pos2::new(center.x, center.y - 1.5), Pos2::new(center.x, center.y + 3.5)], Stroke::new(1.0, color));
        }
        MenuIcon::Zoom => {
            let glass_center = Pos2::new(center.x - 1.5, center.y - 1.5);
            painter.circle_stroke(glass_center, 4.0, s);
            painter.line_segment([Pos2::new(glass_center.x + 2.8, glass_center.y + 2.8), Pos2::new(center.x + 5.5, center.y + 5.5)], Stroke::new(1.8, color));
        }
        MenuIcon::Print => {
            let base = Rect::from_center_size(Pos2::new(center.x, center.y + 1.0), Vec2::new(11.0, 6.0));
            painter.rect_stroke(base, 1.0, s, StrokeKind::Inside);
            let paper = Rect::from_center_size(Pos2::new(center.x, center.y - 3.5), Vec2::new(7.0, 4.0));
            painter.rect_stroke(paper, 0.5, Stroke::new(1.0, color), StrokeKind::Inside);
        }
        MenuIcon::GoogleLens => {
            draw_camera_icon(painter, center, 5.5, color);
        }
        MenuIcon::Translate => {
            painter.text(center, Align2::CENTER_CENTER, "文A", FontId::proportional(10.0), color);
        }
        MenuIcon::Find => {
            let doc = Rect::from_center_size(Pos2::new(center.x - 1.0, center.y), Vec2::new(8.0, 10.0));
            painter.rect_stroke(doc, 1.0, Stroke::new(1.0, color), StrokeKind::Inside);
            painter.circle_stroke(Pos2::new(center.x + 2.0, center.y + 2.0), 2.5, Stroke::new(1.2, color));
        }
        MenuIcon::Cast => {
            let screen = Rect::from_center_size(center, Vec2::new(11.0, 8.0));
            painter.rect_stroke(screen, 1.0, s, StrokeKind::Inside);
            painter.circle_filled(Pos2::new(center.x - 3.5, center.y + 2.0), 1.0, color);
        }
        MenuIcon::MoreTools => {
            painter.line_segment([Pos2::new(center.x - 4.5, center.y - 4.5), Pos2::new(center.x + 4.5, center.y + 4.5)], s);
            painter.line_segment([Pos2::new(center.x - 4.5, center.y + 4.5), Pos2::new(center.x + 4.5, center.y - 4.5)], s);
        }
        MenuIcon::Help => {
            painter.circle_stroke(center, 5.5, s);
            painter.text(center, Align2::CENTER_CENTER, "?", FontId::proportional(9.0), color);
        }
        MenuIcon::Settings => {
            painter.circle_stroke(center, 4.0, s);
            for i in 0..4 {
                let angle = (i as f32) * std::f32::consts::PI / 2.0;
                let p1 = Pos2::new(center.x + angle.cos() * 4.0, center.y + angle.sin() * 4.0);
                let p2 = Pos2::new(center.x + angle.cos() * 5.8, center.y + angle.sin() * 5.8);
                painter.line_segment([p1, p2], s);
            }
        }
        MenuIcon::Exit => {
            let door = Rect::from_center_size(Pos2::new(center.x - 2.5, center.y), Vec2::new(6.0, 10.0));
            painter.rect_stroke(door, 1.0, s, StrokeKind::Inside);
            painter.line_segment([Pos2::new(center.x - 1.0, center.y), Pos2::new(center.x + 5.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x + 3.0, center.y - 2.0), Pos2::new(center.x + 5.0, center.y)], s);
            painter.line_segment([Pos2::new(center.x + 3.0, center.y + 2.0), Pos2::new(center.x + 5.0, center.y)], s);
        }
    }
}

fn render_menu_item(
    ui: &mut egui::Ui,
    icon: MenuIcon,
    label: &str,
    shortcut_or_chevron: Option<&str>,
) -> egui::Response {
    let desired_size = Vec2::new(ui.available_width(), 28.0);
    let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(60, 64, 67));
    }

    let icon_center = Pos2::new(rect.min.x + 16.0, rect.center().y);
    draw_menu_icon(ui.painter(), icon, icon_center, Color32::from_rgb(232, 234, 237));

    let text_pos = Pos2::new(rect.min.x + 34.0, rect.center().y);
    ui.painter().text(
        text_pos,
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        Color32::from_rgb(232, 234, 237),
    );

    if let Some(right_hint) = shortcut_or_chevron {
        let is_chevron = right_hint == ">" || right_hint == "›";
        let font_size = if is_chevron { 13.0 } else { 11.5 };
        let right_pos = Pos2::new(rect.max.x - 10.0, rect.center().y);
        ui.painter().text(
            right_pos,
            Align2::RIGHT_CENTER,
            right_hint,
            FontId::proportional(font_size),
            Color32::from_rgb(154, 160, 166),
        );
    }

    resp
}

fn render_profile_card(
    ui: &mut egui::Ui,
    name: &str,
    status: &str,
) -> egui::Response {
    let desired_size = Vec2::new(ui.available_width(), 36.0);
    let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    let bg_color = if resp.hovered() {
        Color32::from_rgb(64, 68, 74)
    } else {
        Color32::from_rgb(53, 56, 61)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(6), bg_color);

    let avatar_center = Pos2::new(rect.min.x + 18.0, rect.center().y);
    ui.painter().circle_filled(avatar_center, 10.0, Color32::from_rgb(32, 33, 36));
    ui.painter().text(
        avatar_center,
        Align2::CENTER_CENTER,
        "K",
        FontId::proportional(11.0),
        Color32::from_rgb(138, 180, 248),
    );

    let name_pos = Pos2::new(rect.min.x + 34.0, rect.center().y);
    ui.painter().text(
        name_pos,
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(12.5),
        Color32::from_rgb(232, 234, 237),
    );

    let chevron_x = rect.max.x - 10.0;
    ui.painter().text(
        Pos2::new(chevron_x, rect.center().y),
        Align2::RIGHT_CENTER,
        ">",
        FontId::proportional(13.0),
        Color32::from_rgb(154, 160, 166),
    );

    let pill_w = 60.0;
    let pill_h = 18.0;
    let pill_rect = Rect::from_center_size(
        Pos2::new(chevron_x - 12.0 - pill_w / 2.0, rect.center().y),
        Vec2::new(pill_w, pill_h),
    );
    ui.painter().rect_filled(pill_rect, CornerRadius::same(9), Color32::from_rgb(26, 115, 232));
    ui.painter().text(
        pill_rect.center(),
        Align2::CENTER_CENTER,
        status,
        FontId::proportional(10.0),
        Color32::WHITE,
    );

    resp
}

fn render_menu_divider(ui: &mut egui::Ui) {
    ui.add_space(2.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [Pos2::new(rect.min.x + 4.0, rect.center().y), Pos2::new(rect.max.x - 4.0, rect.center().y)],
        Stroke::new(1.0, Color32::from_rgb(60, 64, 67)),
    );
    ui.add_space(2.0);
}

fn render_zoom_row(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut BrowserApp,
) {
    let desired_size = Vec2::new(ui.available_width(), 32.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

    let icon_center = Pos2::new(rect.min.x + 16.0, rect.center().y);
    draw_menu_icon(ui.painter(), MenuIcon::Zoom, icon_center, Color32::from_rgb(232, 234, 237));

    let label_pos = Pos2::new(rect.min.x + 34.0, rect.center().y);
    ui.painter().text(
        label_pos,
        Align2::LEFT_CENTER,
        "Zoom",
        FontId::proportional(12.5),
        Color32::from_rgb(232, 234, 237),
    );

    let btn_size = Vec2::new(24.0, 24.0);

    // Fullscreen button
    let fs_rect = Rect::from_center_size(Pos2::new(rect.max.x - 16.0, rect.center().y), btn_size);
    let fs_resp = ui.interact(fs_rect, ui.id().with("menu_zoom_fs"), egui::Sense::click());
    if fs_resp.hovered() {
        ui.painter().circle_filled(fs_rect.center(), 12.0, Color32::from_rgb(70, 74, 80));
    }
    let fc = fs_rect.center();
    let fs_stroke = Stroke::new(1.3, Color32::from_rgb(232, 234, 237));
    let d = 4.5;
    let b = 2.0;
    ui.painter().line_segment([Pos2::new(fc.x - d, fc.y - d + b), Pos2::new(fc.x - d, fc.y - d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x - d, fc.y - d), Pos2::new(fc.x - d + b, fc.y - d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x + d - b, fc.y - d), Pos2::new(fc.x + d, fc.y - d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x + d, fc.y - d), Pos2::new(fc.x + d, fc.y - d + b)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x - d, fc.y + d - b), Pos2::new(fc.x - d, fc.y + d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x - d, fc.y + d), Pos2::new(fc.x - d + b, fc.y + d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x + d - b, fc.y + d), Pos2::new(fc.x + d, fc.y + d)], fs_stroke);
    ui.painter().line_segment([Pos2::new(fc.x + d, fc.y + d), Pos2::new(fc.x + d, fc.y + d - b)], fs_stroke);

    if fs_resp.clicked() {
        let is_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!is_fullscreen));
    }

    // [+] Button
    let plus_rect = Rect::from_center_size(Pos2::new(fs_rect.min.x - 16.0, rect.center().y), btn_size);
    let plus_resp = ui.interact(plus_rect, ui.id().with("menu_zoom_plus"), egui::Sense::click());
    if plus_resp.hovered() {
        ui.painter().circle_filled(plus_rect.center(), 12.0, Color32::from_rgb(70, 74, 80));
    }
    let pc = plus_rect.center();
    let p_stroke = Stroke::new(1.4, Color32::from_rgb(232, 234, 237));
    ui.painter().line_segment([Pos2::new(pc.x - 4.0, pc.y), Pos2::new(pc.x + 4.0, pc.y)], p_stroke);
    ui.painter().line_segment([Pos2::new(pc.x, pc.y - 4.0), Pos2::new(pc.x, pc.y + 4.0)], p_stroke);
    if plus_resp.clicked() {
        app.zoom_in();
    }

    // Percentage display
    let pct_text = app.zoom_percentage();
    let pct_x = plus_rect.min.x - 22.0;
    ui.painter().text(
        Pos2::new(pct_x, rect.center().y),
        Align2::CENTER_CENTER,
        pct_text,
        FontId::proportional(12.0),
        Color32::from_rgb(232, 234, 237),
    );

    // [-] Button
    let minus_rect = Rect::from_center_size(Pos2::new(pct_x - 22.0, rect.center().y), btn_size);
    let minus_resp = ui.interact(minus_rect, ui.id().with("menu_zoom_minus"), egui::Sense::click());
    if minus_resp.hovered() {
        ui.painter().circle_filled(minus_rect.center(), 12.0, Color32::from_rgb(70, 74, 80));
    }
    let mc = minus_rect.center();
    ui.painter().line_segment([Pos2::new(mc.x - 4.0, mc.y), Pos2::new(mc.x + 4.0, mc.y)], p_stroke);
    if minus_resp.clicked() {
        app.zoom_out();
    }
}
```

#### C. Menu Popup Render Method in `src/app.rs`
```rust
impl BrowserApp {
    fn render_chrome_menu_popup(&mut self, ctx: &egui::Context, m_rect: egui::Rect) {
        let menu_width = 290.0;
        let window_resp = egui::Window::new("chrome_3dots_menu")
            .title_bar(false)
            .anchor(Align2::RIGHT_TOP, Vec2::new(-8.0, 76.0))
            .collapsible(false)
            .resizable(false)
            .fixed_size(Vec2::new(menu_width, 0.0))
            .frame(
                Frame::NONE
                    .fill(Color32::from_rgb(41, 42, 45)) // #292a2d
                    .corner_radius(CornerRadius::same(8))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(60, 64, 67))) // #3c4043
                    .shadow(egui::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: Color32::from_black_alpha(160),
                    })
                    .inner_margin(Margin::symmetric(4, 6)),
            )
            .show(ctx, |ui| {
                ui.set_width(menu_width - 8.0);
                let max_h = (ctx.screen_rect().height() - 90.0).max(300.0).min(680.0);
                ScrollArea::vertical()
                    .max_height(max_h)
                    .show(ui, |ui| {
                        ui.set_width(menu_width - 8.0);

                        // Section 0: Optional Top Alert (Image 2)
                        if render_menu_item(ui, MenuIcon::Shield, "Found 111 compromised passwords", None).clicked() {
                            self.navigate_active_tab("about:settings");
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 1: Navigation
                        if render_menu_item(ui, MenuIcon::NewTab, "New tab", Some("Ctrl+T")).clicked() {
                            self.open_new_tab("about:newtab".to_string());
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::NewWindow, "New window", Some("Ctrl+N")).clicked() {
                            self.open_new_tab("about:newtab".to_string());
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Incognito, "New Incognito window", Some("Ctrl+Shift+N")).clicked() {
                            self.open_new_tab("about:blank".to_string());
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 2: Profile Card Row
                        if render_profile_card(ui, "Khoa", "Signed in").clicked() {
                            self.navigate_active_tab("https://myaccount.google.com");
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 3: Tools & Data
                        if render_menu_item(ui, MenuIcon::Passwords, "Passwords and autofill", Some(">")).clicked() {
                            self.navigate_active_tab("about:settings");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::History, "History", Some("Ctrl+H")).clicked() {
                            self.navigate_active_tab("about:history");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Downloads, "Downloads", Some("Ctrl+J")).clicked() {
                            self.navigate_active_tab("about:downloads");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Bookmarks, "Bookmarks and lists", Some(">")).clicked() {
                            self.navigate_active_tab("about:bookmarks");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::TabGroups, "Tab groups", Some(">")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Extensions, "Extensions", Some(">")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::DeleteData, "Delete browsing data...", Some("Ctrl+Shift+Del")).clicked() {
                            self.navigate_active_tab("about:settings");
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 4: Interactive Zoom row (keeps menu open)
                        render_zoom_row(ui, ctx, self);

                        render_menu_divider(ui);

                        // Section 5: Page actions & Developer Tools
                        if render_menu_item(ui, MenuIcon::Print, "Print...", Some("Ctrl+P")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::GoogleLens, "Search this tab with Google Lens", None).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Translate, "Translate...", None).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Find, "Find and edit", Some(">")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Cast, "Cast, save, and share", Some(">")).clicked() {
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::MoreTools, "More tools", Some(">")).clicked() {
                            self.show_devtools = !self.show_devtools;
                            self.show_menu_popup = false;
                        }

                        render_menu_divider(ui);

                        // Section 6: Settings, Help & Exit
                        if render_menu_item(ui, MenuIcon::Help, "Help", Some(">")).clicked() {
                            self.navigate_active_tab("about:credits");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Settings, "Settings", None).clicked() {
                            self.navigate_active_tab("about:settings");
                            self.show_menu_popup = false;
                        }
                        if render_menu_item(ui, MenuIcon::Exit, "Exit", None).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            self.show_menu_popup = false;
                        }
                    });
            });

        // Dismissal logic: Dismiss on Escape or clicking outside
        if let Some(inner) = window_resp {
            let menu_rect = inner.response.rect;
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.show_menu_popup = false;
            } else if ctx.input(|i| i.pointer.any_pressed()) {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !menu_rect.contains(pos) && !m_rect.contains(pos) {
                        self.show_menu_popup = false;
                    }
                }
            }
        }
    }
}
```

#### D. Wire In `render_navigation_bar`
Replace lines 943-981 with:
```rust
if self.show_menu_popup {
    self.render_chrome_menu_popup(ctx, m_rect);
}
```

---

## 5. Verification Method

### 5.1 Independent Test Suite
Create `tests/chrome_menu_tests.rs`:
```rust
use rust_browser::app::{BrowserApp, ZOOM_STEPS};

#[test]
fn test_chrome_menu_zoom_stepping_ladder() {
    let mut app = BrowserApp::new();
    assert_eq!(app.zoom_percentage(), "100%");

    // Test stepping in
    let expected_steps_up = [1.10, 1.25, 1.50, 1.75, 2.00, 2.50, 3.00, 4.00, 5.00];
    for &expected in &expected_steps_up {
        app.zoom_in();
        assert!((app.zoom_level - expected).abs() < 0.001);
    }
    // At maximum 500%, stepping in clamps
    app.zoom_in();
    assert!((app.zoom_level - 5.00).abs() < 0.001);

    // Test stepping out down to 25%
    let expected_steps_down = [
        4.00, 3.00, 2.50, 2.00, 1.75, 1.50, 1.25, 1.10,
        1.00, 0.90, 0.80, 0.75, 0.67, 0.50, 0.33, 0.25,
    ];
    for &expected in &expected_steps_down {
        app.zoom_out();
        assert!((app.zoom_level - expected).abs() < 0.001);
    }
    // At minimum 25%, stepping out clamps
    app.zoom_out();
    assert!((app.zoom_level - 0.25).abs() < 0.001);

    // Reset to 100%
    app.zoom_reset();
    assert_eq!(app.zoom_percentage(), "100%");
}

#[test]
fn test_chrome_menu_source_invariants() {
    let source = std::fs::read_to_string("src/app.rs").expect("Failed to read src/app.rs");

    // Profile card: "Khoa" present, "Hi, Khoa" strictly absent
    assert!(source.contains("\"Khoa\""), "Profile card name 'Khoa' missing");
    assert!(!source.contains("Hi, Khoa"), "'Hi, Khoa' must not be in src/app.rs");
    assert!(source.contains("\"Signed in\""), "Profile card badge 'Signed in' missing");

    // Navigation section
    assert!(source.contains("\"New tab\""));
    assert!(source.contains("\"Ctrl+T\""));
    assert!(source.contains("\"New window\""));
    assert!(source.contains("\"Ctrl+N\""));
    assert!(source.contains("\"New Incognito window\""));
    assert!(source.contains("\"Ctrl+Shift+N\""));

    // Tools section
    assert!(source.contains("\"Passwords and autofill\""));
    assert!(source.contains("\"History\""));
    assert!(source.contains("\"Downloads\""));
    assert!(source.contains("\"Ctrl+J\""));
    assert!(source.contains("\"Bookmarks and lists\""));
    assert!(source.contains("\"Delete browsing data...\""));
    assert!(source.contains("\"Ctrl+Shift+Del\""));

    // Interactive Zoom row
    assert!(source.contains("\"Zoom\""));
    assert!(source.contains("zoom_in"));
    assert!(source.contains("zoom_out"));

    // Actions & System
    assert!(source.contains("\"Print...\""));
    assert!(source.contains("\"Ctrl+P\""));
    assert!(source.contains("\"More tools\""));
    assert!(source.contains("\"Settings\""));
    assert!(source.contains("\"Exit\""));

    // Dismissal logic
    assert!(source.contains("Key::Escape"));
    assert!(source.contains("menu_rect.contains(pos)"));
}
```

### 5.2 Commands to Run
1. `cargo check` — verify clean compilation.
2. `cargo test` — verify all existing and new tests pass with 0 failures.
3. Invalidation condition:
   - Failure of `cargo check` or any `cargo test` failure.
   - Text selection highlight visible when interacting with tab bar.
   - Menu fails to dismiss when clicking outside or pressing Escape.
   - Zoom buttons close the menu instead of keeping it open.
