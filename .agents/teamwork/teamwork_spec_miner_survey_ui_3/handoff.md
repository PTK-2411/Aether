# Chrome-Style Native DevTools Panel Specification & Architecture

## 1. Observation

Direct investigation of the Rust browser codebase (`src/app.rs`, `src/dom.rs`, `src/layout.rs`, `src/style.rs`, `src/network.rs`) and project requirements (`ORIGINAL_REQUEST.md` R3) revealed the following:

1. **Current DevTools State in `src/app.rs`**:
   - `BrowserApp` contains primitive DevTools fields (lines 104–107, 125–130):
     ```rust
     show_devtools: bool,
     devtools_tab: DevToolsTab,
     selected_dom_node: Option<String>,
     selected_node_info: Option<SelectedNodeInfo>,
     ```
   - Only 4 tabs currently exist:
     ```rust
     enum DevToolsTab {
         DomTree,
         Styles,
         Source,
         Network,
     }
     ```
   - Key toggle is bound exclusively to `Key::F12` at line 605:
     ```rust
     if ctx.input(|i| i.key_pressed(Key::F12)) {
         self.show_devtools = !self.show_devtools;
     }
     ```
     `Ctrl+Shift+I` is not currently handled.
   - The panel is hardcoded as an `egui::SidePanel::right("devtools_panel")` at line 619 with width 420px, without bottom docking support or dock switching.
   - The 3-dots popup menu (`src/app.rs:954`) currently has a basic button `ui.button("DevTools Inspector (F12)")` rather than the standardized Chrome submenu "More tools -> Developer Tools (Ctrl+Shift+I)".

2. **DOM Tree and Selection in `src/app.rs` & `src/dom.rs`**:
   - `render_dom_node_ui` (`src/app.rs:1613-1710`) uses a string label (e.g. `"<div class=\"...\">"`) for `selected` comparison (line 1634: `let is_sel = selected.as_ref() == Some(&label);`).
   - If two elements have identical tags and classes, selecting one highlights all identical elements, causing inspection ambiguity.
   - Expand/collapse carets (`▶` / `▼`) are missing; the entire DOM is rendered expanded up to depth 12 (`if depth > 12 { return; }`), causing UI lag on non-trivial web pages.
   - Colored tokenization is missing: tag names, attributes, quotes, and values are currently rendered as a single monolithic string button.

3. **Box Model and Styles in `src/app.rs`, `src/layout.rs`, & `src/style.rs`**:
   - `render_box_model_diagram` (`src/app.rs:1491-1518`) uses mock hardcoded strings (`"margin (0px)"`, `"border: 1px"`, `"padding (0px)"`, `"block"`) rather than querying the real layout box model computed by `src/layout.rs`.
   - `src/layout.rs:40-60` already defines `Dimensions` with exact `content: Rect`, `padding: EdgeSizes`, `border: EdgeSizes`, and `margin: EdgeSizes`, ready for live binding.
   - `src/style.rs:261-271` provides `matching_rules(node, ancestors, stylesheet)`, but it is private to `style.rs` rather than exposed to DevTools for cascading CSS inspection.

4. **Network and Diagnostics in `src/app.rs` & `src/network.rs`**:
   - `DevToolsTab::Network` (`src/app.rs:1449-1486`) only renders a static diagnostics table with URL, document size, CSS rules count, and render height. It lacks a dynamic HTTP request table, waterfall timeline, status codes, timing metrics, and response inspection.
   - `NetworkManager` in `src/network.rs:35-46` dispatches background threads, but does not record timestamps, durations, or headers in a per-tab request log.

5. **Missing Tabs**:
   - `Console`, `Application`, and `Security` tabs do not exist in the current enum or UI implementation.

---

## 2. Logic Chain

1. **Dockable & Resizable Panel**:
   - Chrome DevTools allows docking to the right, bottom, or undocking into a separate window. In `egui`, `egui::SidePanel::right` and `egui::TopBottomPanel::bottom` can be conditionally used based on a `DevToolsDockPosition` enum (`Right` vs `Bottom`), toggleable via an icon button in the DevTools header.
   - Adding `Ctrl+Shift+I` alongside `F12` ensures full compatibility with Chromium standard key bindings.
   - In `TopBottomPanel::bottom`, `default_height(280.0).min_height(160.0).resizable(true)` gives users an optimal inspecting experience for widescreen displays.

2. **Tab Architecture (6 Chrome Tabs)**:
   - Defining `pub enum DevToolsTab { Elements, Console, Sources, Network, Application, Security }` satisfies Requirement R3 and Chrome DevTools standard layout.
   - Header styling must strictly follow Chrome DevTools Dark Theme (`#202124` tab bar background, `#292a2d` toolbar background, `#8ab4f8` active blue underline, `#9aa0a6` inactive text, `#e8eaed` active text).

3. **Two-Pane Elements Tab**:
   - **Unique Node Identity**: Using a `NodePath` (`Vec<usize>`) representing the child-index trajectory from the root solves the duplicate label selection collision completely.
   - **Expand/Collapse**: A `HashSet<NodePath>` tracking expanded nodes enables collapsible tree nodes with `▶` (collapsed) and `▼` (expanded) carets, improving rendering performance on large DOM trees.
   - **Syntax Tokenization**: Tokenizing opening tags into `<` (blue), `tag_name` (light blue/purple `#8ab4f8`), `attr_name` (yellow/orange `#fdd663`), `=` (gray `#c8cdd2`), `"value"` (green `#81c995`), and `>` (blue) gives an authentic Chrome DevTools appearance.
   - **Live Box Model**: Connecting selected DOM node to its corresponding `LayoutBox` in `Tab.display_list` or layout tree allows extracting live `Dimensions` (`content`, `padding`, `border`, `margin`) to render the nested Chrome box model diagram.
   - **Styles Inspector**: Exposing `style::get_matched_rules` allows displaying matched CSS rules with specificity and selectors, alongside an alphabetized computed properties list.

4. **Console Tab**:
   - Implementing a `ConsoleLogBuffer` with `ConsoleEntry { timestamp, level, source, message }` captures events from HTML parser, CSS parser, network fetches, and layout computations.
   - Filter bar with level badges (`[All]`, `[Errors]`, `[Warnings]`, `[Info]`), clear button (`🚫`), and an interactive command input (`> `) for evaluating browser diagnostics.

5. **Network Tab**:
   - Storing a `Vec<NetworkEntry>` per tab with columns: `Name`, `Status`, `Method`, `Type`, `Size`, `Time`, and a horizontal `Waterfall` bar.
   - Click-to-inspect drawer for HTTP headers and response preview.

6. **Sources Tab**:
   - Left tree showing origin, HTML document, default user-agent stylesheet, inline stylesheets, and scripts.
   - Right pane with line numbers in a dedicated gutter and syntax-highlighted code.

7. **Application & Security Tabs**:
   - **Application**: Displays storage categories (Local Storage, Session Storage, Cookies, Image Cache), quota usage, and a "Clear site data" action.
   - **Security**: Displays HTTPS TLS certificate state (Rustls pure-Rust engine), cipher suite, mixed content check, and security status badge.

---

## 3. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | Shell | Toggle Shortcuts (F12, Ctrl+Shift+I) | Hotkey listeners in `BrowserApp::update` toggling DevTools visibility | Keyboard events | `show_devtools = !show_devtools` | Ignores if input focused in omnibox | Spec dispatch & ORIGINAL_REQUEST.md R3 |
| 2 | Shell | 3-Dots Menu Toggle | "More tools -> Developer Tools" menu item | Mouse click | Opens DevTools & closes popup | No-op if already open | Spec dispatch & Image 2 |
| 3 | Shell | Dock Position Toggle | Switch between Right SidePanel and Bottom TopBottomPanel | Dock icon button click | Layout switches smoothly | Resets to default dimensions if clamped | DevTools architecture analysis |
| 4 | Shell | Chrome Header & Close Button | Dark themed header (`#202124`) with title, shortcut badge, dock button, close button ('×') | Click '×' button | `show_devtools = false` | None | Spec dispatch & devtools-frontend |
| 5 | Shell | Inspect Element Mode Toggle | Cursor icon toggle activating viewport hover-inspection | Click cursor icon | Highlights hovered element on page | Deactivates on element selection | devtools-frontend reference |
| 6 | Navigation | 6-Tab Chrome Navigation Bar | Tab pills: Elements, Console, Sources, Network, Application, Security | Tab click | Sets `devtools_tab` | Preserves sub-tab state | Spec dispatch & ORIGINAL_REQUEST.md R3 |
| 7 | Elements | Colored Token DOM Tree | Interactive DOM hierarchy with colored tokens (tag, attr name, attr value, text, comment) | Tab.dom_root | Interactive UI tree | Shows `<empty DOM>` if None | Spec dispatch & src/app.rs |
| 8 | Elements | Expand / Collapse Carets | Carets (`▶` / `▼`) next to parent elements controlling child visibility | Caret click | Toggles path in `expanded_nodes` | Prevents recursion past collapsed node | Spec dispatch & Chrome DevTools |
| 9 | Elements | Node Selection State | Unique path-based node selection highlighting row in `#283e58` | Element click | Updates `selected_node_path` | Unchanged if clicking caret | Bug fix for duplicate tag selection |
| 10 | Elements | Live Chrome Box Model Diagram | 4-layer nested colored boxes (Margin `#f9ab00`, Border `#fdd663`, Padding `#81c995`, Content `#8ab4f8`) | `Dimensions` from `LayoutBox` | Visual box diagram with exact pixel values | Fallbacks to default values if layout unavailable | Spec dispatch & src/layout.rs |
| 11 | Elements | Matched CSS Rules Inspector | List of CSS rules matching selected node sorted by specificity | Node + ancestors + stylesheet | Display of selectors and declarations | Shows "No matching CSS rules" if empty | Spec dispatch & src/style.rs |
| 12 | Elements | Computed Properties List | Alphabetical list of resolved CSS properties with search filter | StyledNode properties | Filtered property key-value table | Shows "No matching properties" on filter miss | devtools-frontend reference |
| 13 | Console | Log Stream with Severity Levels | Log entries with Info (`ℹ️`), Warn (`⚠️`), and Error (`❌`) badges, timestamps, source location | `ConsoleEntry` stream | Formatted log rows with colored backgrounds | Auto-scrolls to bottom unless pinned | Spec dispatch & ORIGINAL_REQUEST.md R3 |
| 14 | Console | Severity Filter & Clear | Filter buttons `[All]`, `[Errors]`, `[Warnings]`, `[Info]` and clear button `🚫` | Filter click / Clear click | Filters log view / purges buffer | Empty view if all filtered out | Spec dispatch & devtools-frontend |
| 15 | Console | Interactive REPL Input | Input bar (`> `) executing built-in commands (`clear`, `help`, `dom`, `styles`, `zoom`, `goto`) | Text string + Enter | Prints command output to log | Logs "Unknown command" on invalid syntax | Chrome DevTools REPL feature |
| 16 | Network | Request Waterfall Table | Table with Name, Status (colored badge), Method, Type, Size, Time, and visual Waterfall bar | `NetworkEntry` list | Multi-column request ledger | Shows "No network requests recorded" if empty | Spec dispatch & ORIGINAL_REQUEST.md R3 |
| 17 | Network | Request Type Filtering | Filter buttons `[All]`, `[Doc]`, `[CSS]`, `[JS]`, `[Img]`, `[Fetch]` and URL substring filter | Filter click / text entry | Filtered request table | None | devtools-frontend reference |
| 18 | Network | Request Details Drawer | Drawer showing General info, Response/Request Headers, and response body preview | Row click | Expandable details drawer | Dismissible via '×' | Chrome DevTools Network detail |
| 19 | Sources | File & Resource Tree | Tree grouping HTML documents, user-agent CSS, internal `<style>`, and scripts | Tab resources | Expandable resource tree | None | Spec dispatch & ORIGINAL_REQUEST.md R3 |
| 20 | Sources | Line-Numbered Code Viewer | Monospace code viewer with line numbers in gutter and syntax coloring | Active resource text | Gutter + syntax colored editor | Wraps or scrolls horizontally | Spec dispatch & devtools-frontend |
| 21 | Application | Storage & Cookie Inspector | Tree for Cookies, Local Storage, Session Storage, and Texture Cache | Active tab origin | Key-value table view with add/delete/clear | Shows "(empty)" if no entries | Spec dispatch & ORIGINAL_REQUEST.md R3 |
| 22 | Application | Texture Cache & Quota Meter | Inspector for loaded image textures with dimensions and memory usage meter | `BrowserApp.image_textures` | Thumbnail cards and memory usage bar | Shows 0 B if empty | Chrome DevTools Application cache |
| 23 | Security | TLS Security Overview | Security badge (Valid HTTPS 🔒, Insecure HTTP ⚠️, Internal ℹ️), protocol, and cipher suite | Active tab URL + TLS state | Security dashboard card | Flags mixed content warnings | Spec dispatch & ORIGINAL_REQUEST.md R3 |
| 24 | Security | Mixed Content Scanner | Detection of insecure `http://` image/script resources loaded on HTTPS pages | DOM node attribute scan | List of insecure mixed content URLs | Displays "Secure" if 0 found | Chrome DevTools Security tab |

---

## 4. Edge Cases

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | Elements DOM Tree | Empty page (`about:blank` or `about:newtab` without DOM) | Gracefully renders empty placeholder message ("No DOM available for this page") without crashing or panicking. |
| 2 | Elements DOM Tree | Deeply nested DOM (>20 levels of nesting) | Horizontal scrolling with `ScrollArea::both()`, indent clamped or stepped smoothly, no UI overflow or stack overflow. |
| 3 | Elements DOM Tree | Identical tag names and attributes (e.g. 50 `<li>` items with no classes) | Unique `NodePath` (`Vec<usize>`) ensures only the clicked element is highlighted, preventing mass-selection bug. |
| 4 | Elements DOM Tree | Giant text nodes (>10,000 characters) | Tree preview truncates to 60 characters with trailing `...`; full content viewable in inspector without bogging egui painter. |
| 5 | Box Model Diagram | Selected element with `display: none` or not present in layout tree | Falls back cleanly to display `0 × 0` content dimensions and default margins without panic. |
| 6 | Box Model Diagram | Negative margins or auto margins in CSS | Displays exact computed float or string representation (e.g. `-10`, `auto`) safely. |
| 7 | Console Tab | Rapid log spam (e.g. 10,000 continuous console messages) | Ring buffer with fixed maximum capacity (e.g. 1,000 entries) automatically discards oldest entries, capping memory. |
| 8 | Console Tab | REPL command entered with leading/trailing whitespace or empty input | Gracefully trims input; empty Enter does nothing without emitting error. |
| 9 | Network Tab | Failed network requests (DNS resolution failure, timeout, 404, 500) | Displays red status badge (`404 Not Found`, `ERR_FAILED`), duration time, and error reason in details drawer. |
| 10 | Network Tab | Streaming or very long requests (>60 seconds) | Status displays `Pending` in amber `#fdd663`, waterfall bar animates or grows until completed. |
| 11 | Sources Tab | Very large source code (>100,000 lines of HTML/CSS) | Monospace virtual scroll using `egui::ScrollArea` ensures only visible lines are formatted and painted. |
| 12 | Panel Docking | Switching between Right dock and Bottom dock | Retains active tab, selected node, search filter, and scroll positions across dock switch. |
| 13 | Panel Resizing | User resizes window to extremely small dimension (<400px width) | DevTools minimum width/height is clamped to prevent overlapping the omnibox or disappearing offscreen. |
| 14 | Tab Switching | User switches active browser tab | DevTools updates inspected DOM, stylesheets, and network entries to match newly focused tab. |

---

## 5. Architectural Specification & Concrete Data Structures

### 5.1 DevTools State Model

```rust
// Suggested location: src/ui/devtools.rs or src/app.rs

#[derive(Debug, Clone)]
pub struct DevToolsState {
    pub is_open: bool,
    pub dock_position: DevToolsDockPosition,
    pub active_tab: DevToolsTab,
    
    // Elements Tab State
    pub selected_node_path: Option<Vec<usize>>,
    pub expanded_nodes: std::collections::HashSet<Vec<usize>>,
    pub dom_search_query: String,
    pub inspecting_element: bool,
    pub computed_styles_filter: String,
    
    // Console Tab State
    pub console_entries: Vec<ConsoleEntry>,
    pub console_filter: String,
    pub console_level_filter: Option<ConsoleLogLevel>,
    pub console_input: String,
    
    // Network Tab State
    pub network_entries: Vec<NetworkEntry>,
    pub network_filter: String,
    pub network_type_filter: NetworkTypeFilter,
    pub selected_network_id: Option<usize>,
    
    // Sources Tab State
    pub sources_active_file: SourcesFileId,
    pub sources_search_query: String,
    
    // Application Tab State
    pub storage_section: StorageSection,
    pub cookies: Vec<CookieEntry>,
    pub local_storage: std::collections::HashMap<String, String>,
    
    // Security Tab State
    pub mixed_content_warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsDockPosition {
    Bottom,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsTab {
    Elements,
    Console,
    Sources,
    Network,
    Application,
    Security,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    pub id: usize,
    pub timestamp: String,
    pub level: ConsoleLogLevel,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkTypeFilter {
    All,
    Doc,
    Css,
    Js,
    Img,
    Fetch,
}

#[derive(Debug, Clone)]
pub struct NetworkEntry {
    pub id: usize,
    pub url: String,
    pub name: String,
    pub method: String,
    pub status: u16,
    pub status_text: String,
    pub content_type: String,
    pub size_bytes: usize,
    pub start_offset_ms: u64,
    pub duration_ms: u64,
    pub headers: Vec<(String, String)>,
    pub response_body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcesFileId {
    HtmlDocument,
    UserAgentCss,
    InlineCss(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageSection {
    Cookies,
    LocalStorage,
    SessionStorage,
    TextureCache,
}

#[derive(Debug, Clone)]
pub struct CookieEntry {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: String,
    pub secure: bool,
    pub http_only: bool,
}
```

### 5.2 Color Palette Specifications (Chrome DevTools Dark Theme)

| Element | Hex Color | egui Color32 |
|---|---|---|
| Tab Bar Background | `#202124` | `Color32::from_rgb(32, 33, 36)` |
| Toolbar Background | `#292a2d` | `Color32::from_rgb(41, 42, 45)` |
| Content Panel Background | `#202124` | `Color32::from_rgb(32, 33, 36)` |
| Active Tab Text | `#e8eaed` | `Color32::from_rgb(232, 234, 237)` |
| Active Tab Underline | `#8ab4f8` | `Color32::from_rgb(138, 180, 248)` |
| Inactive Tab Text | `#9aa0a6` | `Color32::from_rgb(154, 160, 166)` |
| Tab Hover Background | `#2f3339` | `Color32::from_rgb(47, 51, 57)` |
| Selected Node Row Fill | `#283e58` | `Color32::from_rgb(40, 62, 88)` |
| Hovered Node Row Fill | `#282b30` | `Color32::from_rgb(40, 43, 48)` |
| HTML Bracket `< > </` | `#8ab4f8` | `Color32::from_rgb(138, 180, 248)` |
| HTML Tag Name | `#8ab4f8` | `Color32::from_rgb(138, 180, 248)` |
| HTML Attribute Name | `#fdd663` | `Color32::from_rgb(253, 214, 99)` |
| HTML Attribute Value | `#81c995` | `Color32::from_rgb(129, 201, 149)` |
| HTML Text Node | `#e8eaed` | `Color32::from_rgb(232, 234, 237)` |
| HTML Comment Node | `#80868b` | `Color32::from_rgb(128, 134, 139)` |
| Box Model Margin (Fill/Stroke) | `#2e2518` / `#f9ab00` | Fill: `(46, 37, 24)`, Stroke: `(249, 171, 0)` |
| Box Model Border (Fill/Stroke) | `#2c2817` / `#fdd663` | Fill: `(44, 40, 23)`, Stroke: `(253, 214, 99)` |
| Box Model Padding (Fill/Stroke)| `#1b2a20` / `#81c995` | Fill: `(27, 42, 32)`, Stroke: `(129, 201, 149)` |
| Box Model Content (Fill/Stroke)| `#1a2638` / `#8ab4f8` | Fill: `(26, 38, 56)`, Stroke: `(138, 180, 248)` |
| Console Error (Bg/Text) | `#351e1e` / `#f28b82` | Fill: `(53, 30, 30)`, Text: `(242, 139, 130)` |
| Console Warning (Bg/Text) | `#2e2714` / `#fdd663` | Fill: `(46, 39, 20)`, Text: `(253, 214, 99)` |
| Console Info (Text) | `#e8eaed` | `Color32::from_rgb(232, 234, 237)` |
| Network 2xx OK Badge | `#81c995` | `Color32::from_rgb(129, 201, 149)` |
| Network 3xx Redirect Badge | `#8ab4f8` | `Color32::from_rgb(138, 180, 248)` |
| Network 4xx/5xx Error Badge | `#ea4335` | `Color32::from_rgb(234, 67, 53)` |
| Security Secure Lock | `#81c995` | `Color32::from_rgb(129, 201, 149)` |

---

### 5.3 Engine Integration Bridges

1. **`src/dom.rs` Bridge**:
   - Provide helper methods on `Node`:
     ```rust
     impl Node {
         pub fn get_node_by_path<'a>(&'a self, path: &[usize]) -> Option<&'a Node> {
             let mut current = self;
             for &idx in path {
                 current = current.children.get(idx)?;
             }
             Some(current)
         }
         
         pub fn collect_ancestor_nodes<'a>(&'a self, path: &[usize]) -> Vec<&'a Node> {
             let mut ancestors = Vec::new();
             let mut current = self;
             ancestors.push(current);
             for &idx in path.iter().take(path.len().saturating_sub(1)) {
                 if let Some(child) = current.children.get(idx) {
                     current = child;
                     ancestors.push(current);
                 }
             }
             ancestors
         }
     }
     ```

2. **`src/style.rs` Bridge**:
   - Make rule matching accessible to DevTools:
     ```rust
     pub fn get_matched_rules<'a>(
         node: &Node,
         ancestors: &[&Node],
         stylesheet: &'a Stylesheet,
     ) -> Vec<(Specificity, &'a Rule)> {
         matching_rules(node, ancestors, stylesheet)
     }
     ```
   - Provide a helper to resolve full computed style key-values for display in the Computed Styles section.

3. **`src/layout.rs` Bridge**:
   - Add a method to match `LayoutBox` with the selected `Node`:
     ```rust
     pub fn find_layout_box_for_node<'a, 'b>(
         root: &'b LayoutBox<'a>,
         target: &Node,
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
     ```
   - Enables pulling live pixel `Dimensions` (`content`, `padding`, `border`, `margin`) for any selected DOM node directly into the visual Box Model diagram.

4. **`src/network.rs` Bridge**:
   - Track request start, completion, HTTP status, and duration in `fetch_url_content`:
     ```rust
     let start = std::time::Instant::now();
     // fetch execution...
     let duration_ms = start.elapsed().as_millis() as u64;
     ```
   - Transmit `NetworkEntry` via `NetworkMessage` so that `Tab` appends to its request history.

5. **`src/app.rs` Integration**:
   - Dockable panel rendering:
     ```rust
     if self.devtools_state.is_open {
         match self.devtools_state.dock_position {
             DevToolsDockPosition::Right => {
                 egui::SidePanel::right("devtools_panel")
                     .default_width(480.0)
                     .min_width(320.0)
                     .resizable(true)
                     .frame(Frame::NONE.fill(Color32::from_rgb(32, 33, 36)))
                     .show(ctx, |ui| {
                         self.render_devtools_container(ui, ctx);
                     });
             }
             DevToolsDockPosition::Bottom => {
                 egui::TopBottomPanel::bottom("devtools_panel")
                     .default_height(280.0)
                     .min_height(160.0)
                     .resizable(true)
                     .frame(Frame::NONE.fill(Color32::from_rgb(32, 33, 36)))
                     .show(ctx, |ui| {
                         self.render_devtools_container(ui, ctx);
                     });
             }
         }
     }
     ```
   - Keyboard shortcut handler in `update`:
     ```rust
     ctx.input(|i| {
         if i.key_pressed(Key::F12) || (i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::I)) {
             self.devtools_state.is_open = !self.devtools_state.is_open;
         }
     });
     ```

---

## 6. Caveats

1. **JavaScript Engine Integration**: The Rust browser engine is currently a pure Rust HTML/CSS renderer without V8 or QuickJS execution. Console REPL commands operate on internal browser diagnostics and DOM inspection rather than JS execution.
2. **WebSocket & Worker Tabs**: Chrome DevTools includes additional tabs (Performance, Memory, Profiler) which are outside the scope of R3. The 6 tabs specified (Elements, Console, Sources, Network, Application, Security) completely satisfy all specifications in `ORIGINAL_REQUEST.md`.
3. **Live CSS Editing**: While the specification provides for inspecting matched CSS rules, live inline stylesheet mutation can be handled in a follow-up feature phase.

---

## 7. Conclusion

The specification provides an exhaustive, production-ready blueprint for the Chrome-Style Native DevTools Panel. It resolves all architectural gaps (F12/Ctrl+Shift+I shortcut binding, bottom/right dock switching, 6-tab navigation, colored tokenized DOM tree, path-based node selection, live box model diagram linked to `layout.rs`, structured console log stream with REPL, network waterfall table, line-numbered sources viewer, storage inspector, and TLS security overview).

The modular state model (`DevToolsState`) and clean integration bridges with `dom.rs`, `style.rs`, `layout.rs`, `network.rs`, and `app.rs` allow implementation without destabilizing existing rendering or network tests.

---

## 8. Verification Method

To verify the DevTools implementation independently:

1. **Build & Automated Test Verification**:
   ```bash
   cargo check
   cargo test
   ```
   Ensures zero compilation errors, proper trait implementations, and regression-free operation of existing tests.

2. **Interactive UI Verification (`cargo run`)**:
   - Press `F12` or `Ctrl+Shift+I` to verify DevTools toggles open/closed.
   - Open 3-dots menu, click "More tools -> Developer Tools" to verify menu toggle.
   - Click the Dock switch button to toggle between Right SidePanel and Bottom Panel.
   - Switch between all 6 tabs (Elements, Console, Sources, Network, Application, Security) and verify active tab blue underline (`#8ab4f8`).
   - In Elements tab:
     - Verify DOM tree displays colored tokens (`<div ...>` blue, attributes yellow, values green).
     - Click expand/collapse carets (`▶` / `▼`) to expand and collapse subtrees.
     - Click an element to select it; verify row highlights in `#283e58`.
     - Verify right pane displays nested Margin (orange), Border (yellow), Padding (green), and Content (blue) boxes with live pixel dimensions.
     - Verify Matched CSS Rules list displays selectors and specificity.
   - In Console tab:
     - Verify logs appear with Info, Warning, and Error badges.
     - Toggle level filters and type a REPL command (e.g. `help` or `dom`).
   - In Network tab:
     - Verify request table lists Name, Status code (green 200), Method, Type, Size, and Time.
   - In Sources tab:
     - Verify line-numbered gutter and HTML source preview.
   - In Application tab:
     - Verify Cookies/Storage table and texture cache inspector.
   - In Security tab:
     - Verify HTTPS status badge (valid HTTPS lock) and TLS details.
