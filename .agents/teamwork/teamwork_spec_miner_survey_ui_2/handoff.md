# Specification Report: Browser UI Geometry Polish & Chrome 3-Dots Popup Menu

**Document Version**: 1.0.0  
**Author**: teamwork_spec_miner  
**Date**: 2026-11-05  
**Target Component**: Tab strip, frameless window frame, and Chrome 3-dots popup menu (`src/app.rs`, `src/main.rs`)  
**Reference Material**:
- UI Reference Image 1: `/home/ptk/.gemini/antigravity/brain/76bf5cd9-6ca4-421a-851e-9034dbe7d734/.user_uploaded/media_1793864095995_7f70a4e3.png`
- UI Reference Image 2: `/home/ptk/.gemini/antigravity/brain/76bf5cd9-6ca4-421a-851e-9034dbe7d734/.user_uploaded/media_1793864186546_760d982c.png`
- Base Requirements: `/home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/ORIGINAL_REQUEST.md`

---

## 1. Executive Summary

This specification establishes the authoritative design, layout, color palette, component catalog, state machine, and interaction logic for:
1. **Tab & Window Geometry Polish**: Preventing accidental tab title text selection, rendering 4-corner rounded tab pills, and shaping the outer frameless browser window into an anti-aliased 4-corner rounded frame.
2. **Chrome-Style 3-Dots Popup Menu**: A pixel-faithful implementation of Google Chrome's native desktop menu (Image 2) featuring dark theme styling, subtle border and drop shadow, grouped sections with dividers, profile banner, full command catalog with right-aligned keyboard shortcut hints, and an interactive zoom control row with fullscreen toggle.

---

## 2. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Tab Geometry | Non-Selectable Tab Text | Tab title text labels cannot be highlighted or selected via click/drag, ensuring clean tab switching and window dragging. | Mouse primary click / drag on tab title | Tab selection without text highlighting rectangle | If selectable, draws blue text highlight rectangle over title | Image 1 & egui `Label::selectable(false)` |
| 2 | Tab Geometry | 4-Corner Rounded Tab Pills | Every tab pill in the tab strip has all four corners rounded (top-left, top-right, bottom-left, bottom-right). | Tab dimensions & render pass | Rounded rectangle pill with radius 8px | If radius only applied to top corners (`sw:0, se:0`), creates jarring flat bottom edge | Image 1 & `src/app.rs:666-671` |
| 3 | Tab Geometry | Active vs Inactive Tab Styling | Active tab features lighter dark fill (`#323639`), full contrast title (`#f1f3f4`), vector close button, while inactive tab is transparent/subtle with hover highlight. | Tab activation index state | Visual differentiation between active and background tabs | Inactive tabs missing dividers or hover feedback look broken | Image 1 & Chrome UI design |
| 4 | Window Geometry | 4-Corner Rounded Window Frame | Outer boundary of frameless browser window is clipped/rendered with 4 rounded corners and a 1px subtle stroke. | Window viewport dimensions | Rounded outer window with anti-aliased corners | If viewport background is opaque, black corner artifacts appear | ORIGINAL_REQUEST.md & `eframe::NativeOptions` |
| 5 | Popup Menu | Chrome 3-Dots Trigger Button | Vertical 3-dots button (⋮) on the top-right toolbar toggles menu popup visibility with hover highlight circle. | Mouse click or `Alt+F`/`Alt+E` | Toggles `show_menu_popup: bool` | None; idempotent state toggle | Image 2 & `src/app.rs:930-942` |
| 6 | Popup Menu | Dark Theme Container & Frame | Floating popup container styled with dark slate background (`#292a2d`), 8px rounded corners, 1px border stroke (`#3c4043`), and drop shadow. | Context render loop | Styled floating popup panel | If title bar is present, breaks native menu aesthetic | Image 2 & egui `Window::title_bar(false)` |
| 7 | Popup Menu | Dismiss on Outside Click or Escape | Menu automatically closes when the user clicks anywhere outside the menu rect or presses the Escape key. | Mouse click outside or `Key::Escape` | `show_menu_popup = false` | Stale open menu blocks interaction with underlying web page | Standard desktop UI behavior |
| 8 | Popup Menu | Profile / Account Status Row | Account banner displaying user avatar, profile name ("Khoa"), blue pill badge ("Signed in" / "Sync is on"), and submenu chevron (`>`). | User profile state | Visual account status row | If omitted, diverges from Chrome reference layout | Image 2 Section 2 |
| 9 | Popup Menu | Window/Tab Navigation Rows | Menu items for "New tab" (`Ctrl+T`), "New window" (`Ctrl+N`), and "New Incognito window" (`Ctrl+Shift+N`). | Click on navigation item | Executes action (opens tab, window, or incognito mode) and closes menu | Unsupported actions should gracefully no-op or inform user | Image 2 Section 1 |
| 10 | Popup Menu | Data & History Tools Rows | Items for "Passwords and autofill", "History" (`Ctrl+H`), "Downloads" (`Ctrl+J`), "Bookmarks and lists", "Tab groups", "Extensions", "Delete browsing data..." (`Ctrl+Shift+Del`). | Click on item | Routes to corresponding page or feature dialog | Disabled/unimplemented features route to placeholder or about page | Image 2 Section 3 |
| 11 | Popup Menu | Interactive Zoom Control Row | Composite control row: "Zoom" label, interactive `[-]` button, bold percentage display (e.g. `100%`), `[+]` button, and Fullscreen toggle button. | Click on `[-]`, `[+]`, or Fullscreen | Updates `zoom_level` or toggles fullscreen; keeps menu open | Zoom clamping outside [25%, 500%] prevented | Image 2 Section 4 |
| 12 | Popup Menu | Page Actions & More Tools | Items for "Print..." (`Ctrl+P`), "Search this tab with Google Lens", "Translate...", "Find and edit", "Cast, save, and share", and "More tools" (`>`). | Click on action item | Triggers page action or opens developer/devtools submenu | None | Image 2 Section 5 |
| 13 | Popup Menu | Settings, Help & Exit Rows | Final section containing "Help" (`>`), "Settings", and "Exit". | Click on item | "Settings" opens settings; "Exit" terminates application via `ViewportCommand::Close` | Exit must cleanly close window | Image 2 Section 6 |
| 14 | Popup Menu | Right-Aligned Shortcut Hints | Keyboard shortcuts (`Ctrl+T`, `Ctrl+N`, `Ctrl+J`, etc.) rendered in muted grey (`#9aa0a6`), precisely right-aligned in each row. | Row layout pass | Clean alignment matching Chrome desktop layout | Shortcuts overlapping item label prevented via layout margins | Image 2 visual styling |

---

## 3. Edge Cases

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | Tab Selection Prevention | Mouse drag across tab label text | With `.selectable(false)`, drag does not create text highlight; drag either initiates tab reordering or window movement. |
| 2 | Tab Title Truncation | Extremely long page title (e.g. 100+ characters) | Label must be truncated with ellipsis (`...`) within max tab width (~180px) to prevent pushing other tabs or window controls offscreen. |
| 3 | Close Button on Active Tab | Clicking close `×` button on active tab | Closes active tab without triggering tab switch to itself; if last tab is closed, resets to `about:newtab`. |
| 4 | Rapid Zoom Stepping | User clicks `[+]` or `[-]` multiple times in quick succession | Menu remains visible and responsive; `zoom_level` advances along discrete ladder steps without closing menu. |
| 5 | Zoom Limits | Clicking `[-]` at minimum zoom (25%) or `[+]` at maximum zoom (500%) | Value is clamped to min/max; button visually remains disabled or safely no-ops. |
| 6 | Menu Click Outside | Clicking Omnibox, web page, or blank tab bar while menu is open | Menu detects pointer pressed outside its bounding rect and immediately dismisses (`show_menu_popup = false`). |
| 7 | Menu Toggle Click | Clicking the 3-dots button when menu is already open | Toggles menu to closed state without immediately re-opening. |
| 8 | Escape Key Press | Pressing Escape key while menu is open | Menu dismisses immediately; focus returns to active page or omnibox. |
| 9 | Fullscreen Toggle | Clicking fullscreen button in Zoom row | Viewport enters or leaves borderless fullscreen mode via `ViewportCommand::Fullscreen(!is_fullscreen)`. |
| 10 | Window Resize / Small Screens | Window resized below menu width (e.g. 400px width) | Menu clamps to right edge of viewport and scrolls vertically if height exceeds available space. |

---

## 4. Part 1: Tab and Window Geometry Specification

### 4.1 Tab Title Non-Selectability
- **Root Cause Observed in Image 1**: In egui, the `Label` widget inherits the context setting `ui.style().interaction.selectable_labels`, which defaults to `true`. When a user clicks, presses, or drags on a tab title, egui captures pointer events for text selection and draws an electric-blue highlight box (`#005a9e` as seen over `"free ency"` in Image 1).
- **Required Specification**:
  ```rust
  // Explicitly disable text selection on the tab title label
  let tab_label = egui::Label::new(
      RichText::new(display_title)
          .size(12.0)
          .color(tab_text_color),
  )
  .truncate()
  .selectable(false);
  
  let label_resp = ui.add(tab_label);
  ```
  Alternatively, set `ui.style_mut().interaction.selectable_labels = false;` within the tab rendering scope.
- **Interaction Contract**:
  - Clicking on the label activates the tab (`self.active_tab_idx = idx;`).
  - Dragging starting on the label does NOT select text; it delegates to the tab bar drag logic.
  - Cursor icon remains default arrow (not text I-beam `CursorIcon::Text`).

### 4.2 4-Corner Rounded Tab Pills
- **Geometry Specifications**:
  - Corner radius: `CornerRadius::same(8)` (or `CornerRadius { nw: 8, ne: 8, sw: 8, se: 8 }`).
  - Height: `30.0` px to `32.0` px (floating centered inside the 40.0 px tab strip).
  - Max Width: `200.0` px; Min Width: `80.0` px.
  - Tab Inner Margin: `Margin::symmetric(10, 5)`.
  - Outer Spacing: `2.0` px horizontal spacing between tabs.
- **Color & Style Palette**:
  - **Active Tab**:
    - Fill: Dark slate `#323639` (`Color32::from_rgb(50, 54, 57)`).
    - Text: Pure white `#f1f3f4` (`Color32::from_rgb(241, 243, 244)`).
    - Close Icon: Vector cross drawn with stroke `1.2px`, color `#aaafb4`, circular hover background `#464a50` with color `#ffffff`.
    - Favicon: Loading indicator dot `#fbbc04`, or Aether circle `#8ab4f8` with black "A", or site favicon.
  - **Inactive Tab**:
    - Fill: `Color32::TRANSPARENT`.
    - Hover Fill: `#2a2b2e` (`Color32::from_rgb(42, 43, 46)`) with `CornerRadius::same(8)`.
    - Text: Muted grey `#9aa0a6` (`Color32::from_rgb(154, 160, 166)`).
    - Close Icon: `#9aa0a6`, hidden or subtle until tab is hovered.
    - Vertical Separator: 1px vertical line `#3c4043` between adjacent inactive tabs, height `16.0` px.

### 4.3 4-Corner Rounded Outer Window Frame
- **Architecture**:
  - In `src/main.rs`:
    ```rust
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(Vec2::new(1280.0, 800.0))
            .with_min_inner_size(Vec2::new(800.0, 500.0))
            .with_title("Aether")
            .with_decorations(false)
            .with_transparent(true),
        ..Default::default()
    };
    ```
  - In `BrowserApp` (`eframe::App` implementation):
    ```rust
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // Fully transparent clear color
    }
    ```
  - Outer Container Window Styling:
    - Root Frame: Corner radius `CornerRadius::same(10)` (or `12`).
    - Outer Stroke: `Stroke::new(1.0, Color32::from_rgb(60, 64, 67))` (`#3c4043`).
    - Panel corner rounding:
      - Top Chrome Panel (`TopBottomPanel::top`): `CornerRadius { nw: 10, ne: 10, sw: 0, se: 0 }`.
      - Central Web Viewport Panel (`CentralPanel::default`): `CornerRadius { nw: 0, ne: 0, sw: 10, se: 10 }`.

---

## 5. Part 2: Chrome 3-Dots Popup Menu Catalog & Visual Specification

### 5.1 Container & Layout Properties (Reference Image 2)
- **Container Type**: Frameless floating window (`egui::Window::new("chrome_menu_popup").title_bar(false).resizable(false).collapsible(false)`).
- **Positioning**: Anchored top-right directly below the 3-dots trigger button:
  `anchor(Align2::RIGHT_TOP, Vec2::new(-8.0, 42.0))`.
- **Dimensions**: Fixed width `290.0` px; auto vertical height with scrolling if necessary (`ScrollArea::vertical().max_height(700.0)`).
- **Background Fill**: Dark slate `#292a2d` (`Color32::from_rgb(41, 42, 45)`).
- **Window Corner Radius**: `CornerRadius::same(8)` (or `10`).
- **Border Stroke**: `Stroke::new(1.0, Color32::from_rgb(60, 64, 67))` (`#3c4043`).
- **Drop Shadow**: `Shadow { extrusion: 8.0, color: Color32::from_black_alpha(140) }`.
- **Inner Padding**: `Margin::symmetric(4, 6)`.
- **Item Row Dimensions**:
  - Height: `30.0` px.
  - Hover highlight: `CornerRadius::same(6)`, fill `#3c4043` (`Color32::from_rgb(60, 64, 67)`).
  - Left icon slot width: `24.0` px (centered vector or symbol).
  - Label font: Proportional `12.5` pt, color `#e8eaed` (`Color32::from_rgb(232, 234, 237)`).
  - Right shortcut font: Proportional `11.5` pt, color `#9aa0a6` (`Color32::from_rgb(154, 160, 166)`).
  - Submenu indicator: Right chevron `›` or `>`, color `#9aa0a6`.

### 5.2 Exhaustive Catalog of Sections and Menu Items

#### Section 0: Alert / Notification Banner (Top Row)
| Icon | Label | Right Item | Action |
|------|-------|------------|--------|
| Shield / Alert | "Found 111 compromised passwords" | None | Displays security notice or opens password safety check |

#### Divider Line (`Stroke::new(1.0, Color32::from_rgb(60, 64, 67))`)

#### Section 1: Window & Tab Navigation
| Icon | Label | Right Item | Action |
|------|-------|------------|--------|
| Tab with `+` | "New tab" | `Ctrl+T` | `self.open_new_tab("about:newtab".to_string())` |
| Window rect | "New window" | `Ctrl+N` | Opens new browser instance/window |
| Incognito mask/glasses | "New Incognito window" | `Ctrl+Shift+N` | Opens private/incognito browsing session |

#### Divider Line

#### Section 2: Profile / Account Status Row
- Framed card with fill `#35383d` (`Color32::from_rgb(53, 56, 61)`), corner radius `6px`.
- Left: Profile Avatar (circle diameter 20px with user silhouette or colored avatar).
- Label: `"Khoa"` (user account profile name).
- Right Badge: Blue pill container (fill `#1a73e8`, text `"Signed in"`, font size 11px bold, corner radius 10px, padding 8x3).
- Right Chevron: `>` (`Color32::from_rgb(154, 160, 166)`).
- Action: Opens Google Account management (`https://myaccount.google.com`).

#### Divider Line

#### Section 3: Browser Features & Tools
| Icon | Label | Right Item | Action |
|------|-------|------------|--------|
| Key | "Passwords and autofill" | `>` | Submenu / Password manager view |
| Clock counter-clockwise | "History" | `>` (or `Ctrl+H`) | Opens browser history panel |
| Down arrow to tray | "Downloads" | `Ctrl+J` | Opens downloads panel / directory |
| Star | "Bookmarks and lists" | `>` | Submenu / Bookmarks manager |
| 4-square grid | "Tab groups" | `>` | Tab grouping controls |
| Puzzle piece | "Extensions" | `>` | Extensions manager |
| Trash can | "Delete browsing data..." | `Ctrl+Shift+Del` | Clears cache, cookies, and local data |

#### Divider Line

#### Section 4: Zoom Controls Row
- Composite interactive row spanning full menu width:
  - Left: Magnifying glass icon `⌕` + Label `"Zoom"` (font size 12.5pt, `#e8eaed`).
  - Right Controls Group:
    - **`[-]` Button**: Pill/circular button (24x24 px), hover fill `#464a50`, minus symbol `−` (stroke 1.5px, `#e8eaed`). Decrements zoom level.
    - **Percentage Display**: Label centered in ~40px width (e.g. `"100%"`), font size 12.0pt bold, color `#e8eaed`.
    - **`[+]` Button**: Pill/circular button (24x24 px), hover fill `#464a50`, plus symbol `+` (stroke 1.5px, `#e8eaed`). Increments zoom level.
    - **Fullscreen Button**: Pill/circular button (24x24 px), hover fill `#464a50`, vector 4-corner square `⛶`. Toggles fullscreen.
- **Zoom Ladder Steps**:
  ```rust
  pub const ZOOM_STEPS: &[f32] = &[
      0.25, 0.33, 0.50, 0.67, 0.75, 0.80, 0.90, 1.00,
      1.10, 1.25, 1.50, 1.75, 2.00, 2.50, 3.00, 4.00, 5.00,
  ];
  ```
  - When `[-]` is clicked: find closest step index, decrement by 1 (minimum step 0.25).
  - When `[+]` is clicked: find closest step index, increment by 1 (maximum step 5.00).
  - Menu remains open upon zoom click so user can repeatedly adjust zoom.

#### Divider Line

#### Section 5: Page Actions & Media
| Icon | Label | Right Item | Action |
|------|-------|------------|--------|
| Printer | "Print..." | `Ctrl+P` | Opens print dialog / print preview |
| Camera lens | "Search this tab with Google Lens" | None | Visual page search |
| Translate doc | "Translate..." | None | Google Translate integration |
| Document with lens | "Find and edit" | `>` | In-page text search / editing tools |
| Screen with waves | "Cast, save, and share" | `>` | Casting and sharing options |
| Toolbox / Wrench | "More tools" | `>` | Submenu: DevTools Inspector (`F12`), CSS Test Suite (`about:test`), Engine Credits (`about:credits`) |

#### Divider Line

#### Section 6: Settings, Help & Exit
| Icon | Label | Right Item | Action |
|------|-------|------------|--------|
| Question in circle | "Help" | `>` | Submenu: About Aether, Help Center |
| Gear / Cog | "Settings" | None | Opens settings panel (`about:settings`) |
| Door with exit arrow | "Exit" | None | Closes browser application via `ctx.send_viewport_cmd(egui::ViewportCommand::Close)` |

---

## 6. Part 3: State Machine & Interaction Logic

### 6.1 State Lifecycle
```
                [3-Dots Click or Alt+F]
  [CLOSED] ─────────────────────────────────► [OPEN]
     ▲                                          │
     │      [Escape / Click Outside / Action]    │
     └──────────────────────────────────────────┘
```

1. **Opening the Menu**:
   - Primary Trigger: Clicking the 3-dots button (⋮) on the navigation bar.
   - Hotkey Trigger: Pressing `Alt+F` or `Alt+E`.
   - Action: `self.show_menu_popup = !self.show_menu_popup;`

2. **Closing the Menu**:
   - **Click-Outside Detection**:
     ```rust
     if self.show_menu_popup {
         if ctx.input(|i| i.pointer.any_pressed()) {
             if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                 if !menu_rect.contains(pos) && !three_dots_rect.contains(pos) {
                     self.show_menu_popup = false;
                 }
             }
         }
         if ctx.input(|i| i.key_pressed(Key::Escape)) {
             self.show_menu_popup = false;
         }
     }
     ```
   - **Action Item Click**:
     When any standard action item is clicked (e.g. New tab, Print, Exit, Settings):
     1. Execute the corresponding handler function.
     2. Set `self.show_menu_popup = false;`.
   - **Exception**: Zoom controls (`[-]`, `[+]`, Fullscreen) deliberately keep `self.show_menu_popup = true`.

3. **Menu Item Component Pattern**:
   ```rust
   fn render_menu_row(
       ui: &mut egui::Ui,
       icon: MenuIcon,
       label: &str,
       shortcut_or_chevron: Option<&str>,
   ) -> egui::Response {
       let desired_size = Vec2::new(ui.available_width(), 30.0);
       let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());
       
       if resp.hovered() {
           ui.painter().rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(60, 64, 67));
       }
       
       // Draw Left Icon
       let icon_center = Pos2::new(rect.min.x + 14.0, rect.center().y);
       draw_menu_icon(ui.painter(), icon, icon_center, Color32::from_rgb(232, 234, 237));
       
       // Draw Left Label
       let text_pos = Pos2::new(rect.min.x + 32.0, rect.center().y);
       ui.painter().text(
           text_pos,
           Align2::LEFT_CENTER,
           label,
           FontId::proportional(12.5),
           Color32::from_rgb(232, 234, 237),
       );
       
       // Draw Right Shortcut or Chevron
       if let Some(right_text) = shortcut_or_chevron {
           let right_pos = Pos2::new(rect.max.x - 8.0, rect.center().y);
           ui.painter().text(
               right_pos,
               Align2::RIGHT_CENTER,
               right_text,
               FontId::proportional(11.5),
               Color32::from_rgb(154, 160, 166),
           );
       }
       
       resp
   }
   ```

---

## 7. 5-Component Handoff Report

### 1. Observation
- **Reference Image 1 Inspection**:
  - Location: `/home/ptk/.gemini/antigravity/brain/76bf5cd9-6ca4-421a-851e-9034dbe7d734/.user_uploaded/media_1793864095995_7f70a4e3.png`.
  - Content: Demonstrates a tab title with accidental text highlighting (`"free ency"` rendered with an electric blue rectangular background `#005a9e`). Shows active tab styled as a lighter dark pill alongside an inactive tab and the top of the Omnibox.
- **Reference Image 2 Inspection**:
  - Location: `/home/ptk/.gemini/antigravity/brain/76bf5cd9-6ca4-421a-851e-9034dbe7d734/.user_uploaded/media_1793864186546_760d982c.png`.
  - Content: Complete Chrome 3-dots popup menu in dark mode with 6 distinct sections separated by divider lines, account profile card ("Khoa", "Signed in"), navigation rows with `Ctrl+T`, `Ctrl+N`, `Ctrl+Shift+N`, tool rows with `Ctrl+J`, `Ctrl+Shift+Del`, composite zoom row with `[-]`, `100%`, `[+]`, and fullscreen toggle, page action rows with `Ctrl+P`, and bottom settings/exit rows.
- **Existing Codebase Inspection**:
  - `src/main.rs:46-53`: Window is configured with `with_decorations(false)`, but lacks `with_transparent(true)`.
  - `src/app.rs:666-671`: Tab frame corner radius is configured as `CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }`, leaving bottom corners completely square.
  - `src/app.rs:694-702`: Tab label is created via `egui::Label::new(...)` without `.selectable(false)`.
  - `src/app.rs:943-980`: Popup menu is a minimal 5-item placeholder window with default window title bar, missing the Chrome visual style, profile banner, shortcuts, icons, and sections.

### 2. Logic Chain
1. Accidental text selection occurs because egui's default `interaction.selectable_labels` is true and `Label::selectable(false)` was not called. Adding `.selectable(false)` guarantees tabs behave strictly as clickable buttons without highlighting text.
2. In Image 1 and modern Chrome styling, tab pills sit floating inside the tab strip. Setting `CornerRadius::same(8)` makes all 4 corners smooth.
3. Frameless desktop windows have sharp square outer corners unless transparency is enabled (`with_transparent(true)`) and clear color is set to transparent (`clear_color` returning `[0.0, 0.0, 0.0, 0.0]`), allowing egui to paint an outer frame with `CornerRadius::same(10)` and a 1px boundary stroke.
4. Image 2 establishes the exact layout and hierarchy for Chrome's desktop menu. Translating each row, divider, shortcut hint, and zoom step into an egui component architecture ensures 100% compliance with user requirements.

### 3. Caveats
- Submenu drill-downs (such as clicking `>` on "Bookmarks and lists" or "More tools") can be implemented as either child popup windows or in-place navigation transitions; standard items (New Tab, Zoom, Settings, Exit, DevTools) are immediately actionable.
- On some Linux Wayland compositors, window transparency may depend on compositor support; fallback to dark window background fill ensures no visual breakage.

### 4. Conclusion
The tab selection bug, tab pill corner rounding, frameless window rounding, and 3-dots popup menu have been exhaustively surveyed, cataloged, and specified. The specification provides exact pixel measurements, color constants, interaction contracts, and component implementations ready for the worker to implement.

### 5. Verification Method
- **Tab Selection**: Run `cargo run`. Click and drag across any tab title. Confirm no blue selection highlight appears.
- **Tab Corners**: Inspect running browser tab bar. Confirm all four corners of active and hovered tabs are rounded (`8px`).
- **Window Frame Corners**: Confirm the outer 4 corners of the window are rounded with a subtle 1px border.
- **3-Dots Menu**:
  1. Click 3-dots button (⋮). Verify dark theme menu opens anchored to top right with no default window header.
  2. Verify all sections, profile card, shortcuts (`Ctrl+T`, `Ctrl+N`, `Ctrl+J`, `Ctrl+P`), and dividers match Image 2.
  3. Click `[-]` and `[+]` on the zoom row. Verify page zoom adjusts, percentage updates, and menu remains open.
  4. Click outside the menu. Verify menu dismisses.
  5. Open menu and press Escape. Verify menu dismisses.
  6. Click "Exit". Verify application terminates cleanly.
- **Automated Verification**: `cargo check` and `cargo test`.
