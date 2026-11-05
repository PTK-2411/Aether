# Implementation Strategy Report: Requirement R1 (Tab Title Selection Prevention & 4-Corner Rounding)

**Milestone**: M2 (Browser UI Polish - Tab & Window Geometry)  
**Author**: teamwork_explorer_m2_1  
**Target Requirement**: Requirement R1 (Tab Title Selection Prevention, Tab 4-Corner Rounding, Window Transparency & 4-Corner Window Frame Rounding)  
**Files Affected**: `src/app.rs`, `src/main.rs`, `tests/challenger_ui_tests.rs`

---

## 1. Observation

### 1.1 Tab Strip Current Implementation (`src/app.rs:664-726`)
1. **Corner Rounding in `src/app.rs:664-672`**:
   ```rust
   let frame = Frame::NONE
       .fill(tab_bg)
       .corner_radius(CornerRadius {
           nw: 8,
           ne: 8,
           sw: 0,
           se: 0,
       })
       .inner_margin(egui::Margin::symmetric(10, 6));
   ```
   Only top-left (`nw`) and top-right (`ne`) corners are rounded (8px). The bottom corners (`sw: 0, se: 0`) are explicitly square, conflicting with Requirement R1 ("All four corners of each tab pill must be rounded").

2. **Tab Title Label Selection in `src/app.rs:694-706`**:
   ```rust
   let display_title = if tab.title.is_empty() { "New Tab" } else { &tab.title };
   let label = ui.add(
       egui::Label::new(
           RichText::new(display_title)
               .size(12.0)
               .color(tab_text_color),
       )
       .truncate(),
   );
   if label.clicked() {
       self.active_tab_idx = idx;
       self.address_input = if tab.is_new_tab() { String::new() } else { tab.url.clone() };
   }
   ```
   In `egui-0.31.1/src/widgets/label.rs:124-152`, `Label` defaults `selectable` to `ui.style().interaction.selectable_labels` (which defaults to `true`). When `selectable` is true, egui sets `sense = sense.union(Sense::click_and_drag())`. Dragging or clicking over the tab label initiates text galley selection, painting an unwanted highlight rectangle over the tab title (as captured in Reference Image 1).

3. **egui 0.31 Sense Interaction Nuance in `label.rs:128-135`**:
   If `.selectable(false)` is set alone without specifying `.sense(...)`, `label.rs` defaults `sense` to `Sense::hover()`. When `sense` is only `Sense::hover()`, `label.clicked()` evaluates to `false`! Therefore, to keep the label clickable as a tab switcher, it MUST be chained with `.sense(egui::Sense::click())`.

### 1.2 Main Window Configuration (`src/main.rs:46-53`)
```rust
let native_options = eframe::NativeOptions {
    viewport: eframe::egui::ViewportBuilder::default()
        .with_inner_size(Vec2::new(1280.0, 800.0))
        .with_min_inner_size(Vec2::new(800.0, 500.0))
        .with_title("Aether")
        .with_decorations(false),
    ..Default::default()
};
```
While `.with_decorations(false)` is set, `.with_transparent(true)` is missing. Without `.with_transparent(true)`, the native windowing system (X11/Wayland/DWM/Quartz) allocates an opaque framebuffer, resulting in black or system-default rectangular corners outside any rounded UI boundary.

### 1.3 Viewport Frame & Background in `src/app.rs:544-639`
1. `BrowserApp` implements `eframe::App` without overriding `fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4]`. It defaults to an opaque grey/black clear color (`epi.rs:215-222`), which blocks transparent corner passthrough.
2. `egui::TopBottomPanel::top("chrome_top_panel")` and `egui::CentralPanel::default()` use `Frame::NONE` without any corner radius, producing 90-degree square window corners.
3. No outer boundary border stroke is painted around the window perimeter.

### 1.4 Test Suite String Invariants (`tests/challenger_ui_tests.rs:79-146`)
Existing tests enforce strict string checks on `src/app.rs`:
- Absence of: `"✦ AI Mode"`, `"Hi, Khoa"`, `"100% Pure Rust Web Browser • Zero Chromium"`, `"Aether AI Assistant"`, `"Explore Open Web"`, `"✦ Aether AI"`.
- Presence of: `"Vec2::new(64.0, 28.0)"`, `"\"Login\""`, `"https://accounts.google.com"`, `"let total_shortcuts_w = 6.0 * 68.0;"`, `"let available_edit_w = child_ui.available_width();"`.
All current tests pass (`cargo test`: 26 tests ok).

---

## 2. Logic Chain

1. **Tab Rounding**:
   Replacing `CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }` with `CornerRadius::same(8)` rounds all four corners of each tab pill uniformly to 8px.
2. **Text Selection Prevention & Click Preservation**:
   Calling `.selectable(false)` disables galley text highlighting. Calling `.sense(egui::Sense::click())` ensures the widget receives primary pointer clicks so `label.clicked()` triggers tab switching. Tab dragging on the empty tab strip region (`ctx.send_viewport_cmd(ViewportCommand::StartDrag)`) remains unaffected.
3. **Window Transparency**:
   Enabling `.with_transparent(true)` on `ViewportBuilder` coupled with `BrowserApp::clear_color` returning `[0.0, 0.0, 0.0, 0.0]` ensures pixels outside the rounded UI frame remain 100% transparent.
4. **Window Outer Rounding & 1px Border Stroke**:
   - When the window is maximized (`is_maximized == true`), corners should be square (`window_radius = 0`) to seamlessly fill the monitor.
   - When unmaximized (`!is_maximized`), `window_radius = 10`.
   - `TopBottomPanel` receives `CornerRadius { nw: window_radius, ne: window_radius, sw: 0, se: 0 }`.
   - `SidePanel::right` (DevTools, when open) receives `CornerRadius { nw: 0, ne: 0, sw: 0, se: window_radius }`.
   - `CentralPanel` receives `CornerRadius { nw: 0, ne: 0, sw: window_radius, se: if self.show_devtools { 0 } else { window_radius } }`.
   - A 1px border stroke (`Stroke::new(1.0, Color32::from_rgb(60, 64, 67))`) is painted on `egui::Order::Foreground` with `CornerRadius::same(10)` and `StrokeKind::Inside`.
5. **Canvas Edge Clipping**:
   In `render_page_viewport`, passing the bottom corner radius to `painter.rect_filled(response.rect, CornerRadius { nw: 0, ne: 0, sw: bottom_radius, se: ... }, canvas_bg)` prevents web pages with white backgrounds from covering the window's bottom rounded corners.

---

## 3. Caveats

1. **Window Maximized State**:
   When maximized, window frame rounding MUST collapse to `0` and the 1px border stroke must not render, avoiding unsightly transparent corner cutouts on full screens.
2. **DevTools Side Panel Intersection**:
   If DevTools is open on the right, the bottom-right corner of the window is owned by `SidePanel::right`, while the bottom-left corner is owned by `CentralPanel`. The panel frame configurations account for this dynamically.
3. **Tab Close Button Precedence**:
   If the user clicks the close button (`×`), tab activation must not fire on the same click. Guarding with `if close_tab_idx.is_none()` ensures clean closing semantics.
4. **Preservation of `challenger_ui_tests.rs` Invariants**:
   No modifications touch the login pill (`Vec2::new(64.0, 28.0)`), omnibox width calculation (`let available_edit_w = child_ui.available_width();`), or shortcuts row (`let total_shortcuts_w = 6.0 * 68.0;`).

---

## 4. Conclusion

Requirement R1 is cleanly solvable with targeted, surgical modifications across `src/main.rs` and `src/app.rs`. Below are the exact code diffs and test cases to be implemented by the worker.

### 4.1 Exact Code Diffs

#### Diff 1: `src/main.rs` — Viewport Transparency
```rust
--- a/src/main.rs
+++ b/src/main.rs
@@ -49,6 +49,7 @@ fn main() -> eframe::Result<()> {
             .with_inner_size(Vec2::new(1280.0, 800.0))
             .with_min_inner_size(Vec2::new(800.0, 500.0))
             .with_title("Aether")
-            .with_decorations(false),
+            .with_decorations(false)
+            .with_transparent(true),
         ..Default::default()
     };
```

#### Diff 2: `src/app.rs` — Imports
```rust
--- a/src/app.rs
+++ b/src/app.rs
@@ -9,7 +9,7 @@ use crate::render::{build_display_list, paint_display_list, DisplayList};
 use crate::style::style_tree;
 use eframe::egui::{
-    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Key, Pos2, RichText,
-    ScrollArea, Stroke, StrokeKind, Vec2,
+    self, Align2, Color32, CornerRadius, CursorIcon, FontId, Frame, Id, Key, LayerId, Order,
+    Pos2, RichText, ScrollArea, Stroke, StrokeKind, Vec2,
 };
```

#### Diff 3: `src/app.rs` — `clear_color` and Rounded Window Panels
```rust
--- a/src/app.rs
+++ b/src/app.rs
@@ -544,6 +544,10 @@ fn collect_style_tags(node: &Node, out: &mut String) {
 }
 
 impl eframe::App for BrowserApp {
+    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
+        [0.0, 0.0, 0.0, 0.0]
+    }
+
     fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
         self.check_network_responses();
@@ -608,29 +612,56 @@ impl eframe::App for BrowserApp {
         if ctx.input(|i| i.key_pressed(Key::F12)) {
             self.show_devtools = !self.show_devtools;
         }
 
+        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
+        let window_radius = if is_maximized { 0 } else { 10 };
+
         // Top Chrome Panel (Tabs + Navigation Bar)
+        let top_frame = Frame::NONE
+            .fill(Color32::from_rgb(32, 33, 36))
+            .corner_radius(CornerRadius {
+                nw: window_radius,
+                ne: window_radius,
+                sw: 0,
+                se: 0,
+            });
         egui::TopBottomPanel::top("chrome_top_panel")
-            .frame(Frame::NONE.fill(Color32::from_rgb(32, 33, 36)))
+            .frame(top_frame)
             .show(ctx, |ui| {
                 self.render_tab_strip(ui, ctx);
                 self.render_navigation_bar(ui, ctx);
             });
 
         // DevTools side panel if open
         if self.show_devtools {
+            let devtools_frame = Frame::NONE
+                .fill(Color32::from_rgb(28, 30, 33))
+                .corner_radius(CornerRadius {
+                    nw: 0,
+                    ne: 0,
+                    sw: 0,
+                    se: window_radius,
+                });
             egui::SidePanel::right("devtools_panel")
                 .default_width(420.0)
                 .min_width(320.0)
-                .frame(Frame::NONE.fill(Color32::from_rgb(28, 30, 33)))
+                .frame(devtools_frame)
                 .show(ctx, |ui| {
                     self.render_devtools(ui);
                 });
         }
 
         // Main Web Viewport or Aether New Tab Start Page
+        let central_frame = Frame::NONE
+            .fill(Color32::from_rgb(53, 54, 58)) // #35363a
+            .corner_radius(CornerRadius {
+                nw: 0,
+                ne: 0,
+                sw: window_radius,
+                se: if self.show_devtools { 0 } else { window_radius },
+            });
         egui::CentralPanel::default()
-            .frame(Frame::NONE.fill(Color32::from_rgb(53, 54, 58))) // #35363a
+            .frame(central_frame)
             .show(ctx, |ui| {
                 let is_new_tab = self.tabs.get(self.active_tab_idx).map_or(true, |t| t.is_new_tab());
                 if is_new_tab {
                     self.render_aether_new_tab_page(ui, ctx);
                 } else {
                     self.render_page_viewport(ui, ctx);
                 }
             });
+
+        // Outer window border stroke for 4-corner rounded frameless window
+        if !is_maximized {
+            let screen_rect = ctx.screen_rect();
+            let border_stroke = Stroke::new(1.0, Color32::from_rgb(60, 64, 67));
+            let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("window_border")));
+            painter.rect_stroke(screen_rect, CornerRadius::same(10), border_stroke, StrokeKind::Inside);
+        }
     }
 }
```

#### Diff 4: `src/app.rs` — Tab Strip 4-Corner Rounding & Non-Selectable Label
```rust
--- a/src/app.rs
+++ b/src/app.rs
@@ -664,12 +664,7 @@ impl BrowserApp {
                 let frame = Frame::NONE
                     .fill(tab_bg)
-                    .corner_radius(CornerRadius {
-                        nw: 8,
-                        ne: 8,
-                        sw: 0,
-                        se: 0,
-                    })
+                    .corner_radius(CornerRadius::same(8))
                     .inner_margin(egui::Margin::symmetric(10, 6));
 
                 let resp = frame.show(ui, |ui| {
@@ -694,7 +689,9 @@ impl BrowserApp {
                         let display_title = if tab.title.is_empty() { "New Tab" } else { &tab.title };
                         let label = ui.add(
                             egui::Label::new(
                                 RichText::new(display_title)
                                     .size(12.0)
                                     .color(tab_text_color),
                             )
-                            .truncate(),
+                            .truncate()
+                            .selectable(false)
+                            .sense(egui::Sense::click()),
                         );
                         if label.clicked() {
                             self.active_tab_idx = idx;
@@ -721,7 +718,7 @@ impl BrowserApp {
                     });
                 });
 
-                if resp.response.clicked() {
+                if close_tab_idx.is_none() && resp.response.interact(egui::Sense::click()).clicked() {
                     self.active_tab_idx = idx;
                     self.address_input = if tab.is_new_tab() { String::new() } else { tab.url.clone() };
                 }
```

#### Diff 5: `src/app.rs` — Viewport Webpage Canvas Bottom Corner Rounding
```rust
--- a/src/app.rs
+++ b/src/app.rs
@@ -1189,7 +1189,17 @@ impl BrowserApp {
                 } else {
                     Color32::WHITE
                 };
-                painter.rect_filled(response.rect, 0.0, canvas_bg);
+                let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
+                let bottom_radius = if is_maximized { 0 } else { 10 };
+                painter.rect_filled(
+                    response.rect,
+                    CornerRadius {
+                        nw: 0,
+                        ne: 0,
+                        sw: bottom_radius,
+                        se: if self.show_devtools { 0 } else { bottom_radius },
+                    },
+                    canvas_bg,
+                );
 
                 let origin = response.rect.min;
```

---

### 4.2 Concrete Test Cases to Implement (`tests/challenger_ui_tests.rs`)

The worker should append the following automated tests to `tests/challenger_ui_tests.rs`:

```rust
#[test]
fn test_tab_and_window_geometry_polish() {
    let app_source = std::fs::read_to_string("src/app.rs")
        .expect("Failed to read src/app.rs");
    let main_source = std::fs::read_to_string("src/main.rs")
        .expect("Failed to read src/main.rs");

    // 1. Tab title selection prevention: selectable(false) must be called on tab label
    assert!(
        app_source.contains(".selectable(false)"),
        "Tab title label in src/app.rs must have .selectable(false) to prevent accidental text selection"
    );

    // 2. Tab 4-corner rounding: CornerRadius::same(8) must be used, old flat bottom (sw: 0, se: 0) removed
    assert!(
        app_source.contains("CornerRadius::same(8)"),
        "Tab strip must use 4-corner rounded tab pills with CornerRadius::same(8)"
    );
    assert!(
        !app_source.contains("sw: 0,\n                        se: 0,"),
        "Old square bottom corners (sw: 0, se: 0) must be removed from tab strip in src/app.rs"
    );

    // 3. Main window transparency: ViewportBuilder with_transparent(true) and with_decorations(false)
    assert!(
        main_source.contains(".with_transparent(true)"),
        "ViewportBuilder in src/main.rs must specify .with_transparent(true)"
    );
    assert!(
        main_source.contains(".with_decorations(false)"),
        "ViewportBuilder in src/main.rs must specify .with_decorations(false)"
    );

    // 4. Window frame clear color: transparent clear_color on BrowserApp
    assert!(
        app_source.contains("fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4]")
            && app_source.contains("[0.0, 0.0, 0.0, 0.0]"),
        "BrowserApp in src/app.rs must implement clear_color returning [0.0, 0.0, 0.0, 0.0]"
    );

    // 5. Outer window frame border stroke: 1px border stroke with Color32::from_rgb(60, 64, 67)
    assert!(
        app_source.contains("Stroke::new(1.0, Color32::from_rgb(60, 64, 67))"),
        "Window frame must render 1px outer stroke with Color32::from_rgb(60, 64, 67)"
    );
}

#[test]
fn test_tab_strip_tab_switching_and_management() {
    let mut app = BrowserApp::new();
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab_idx, 0);

    // Open second tab
    app.open_new_tab("about:newtab".to_string());
    assert_eq!(app.tabs.len(), 2);
    assert_eq!(app.active_tab_idx, 1);

    // Switch back to tab 0
    app.active_tab_idx = 0;
    assert_eq!(app.active_tab_idx, 0);

    // Close tab 1
    app.close_tab(1);
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab_idx, 0);
}
```

---

## 5. Verification Method

1. **Compilation Check**:
   ```bash
   cargo check
   ```
2. **Automated Test Suite**:
   ```bash
   cargo test --test challenger_ui_tests
   cargo test
   ```
   Both the new invariant/state tests and the 26 existing tests must pass with 0 errors.
3. **Interactive Visual Verification**:
   ```bash
   cargo run
   ```
   - Click and drag across the tab title text: Verify that no blue selection highlight box appears.
   - Inspect the tab pills: Verify all four corners of each tab pill are smoothly rounded (`8px`).
   - Inspect the window perimeter: Verify all four outer corners of the browser window are rounded (`10px`) with a subtle 1px border stroke `#3c4043`.
   - Maximize the window: Verify that corners become square to fill the monitor.
   - Click tab titles and the `+` button: Verify tab switching and new tab creation work seamlessly.
