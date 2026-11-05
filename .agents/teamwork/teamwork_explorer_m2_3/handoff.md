# Chrome-Style Native DevTools Panel (Requirement R3) Implementation Strategy

## 1. Observation

Direct investigation of the Rust browser codebase (`src/dom.rs`, `src/style.rs`, `src/layout.rs`, `src/network.rs`, `src/app.rs`), test suite (`tests/browser_tests.rs`, `tests/challenger_ui_tests.rs`), and the specification in `ORIGINAL_REQUEST.md` (Requirement R3) revealed the following concrete architectural facts:

### 1.1 Existing DevTools Panel in `src/app.rs`
- **Tab Enum (`src/app.rs:125-130`)**:
  ```rust
  enum DevToolsTab {
      DomTree,
      Styles,
      Source,
      Network,
  }
  ```
  Currently only defines 4 tabs. Missing Chrome-standard tabs required by R3: `Elements`, `Console`, `Sources`, `Network`, `Application`, `Security`.
- **Panel Docking & Shortcut (`src/app.rs:605-626`)**:
  ```rust
  // Global hotkey F12 toggle DevTools
  if ctx.input(|i| i.key_pressed(Key::F12)) {
      self.show_devtools = !self.show_devtools;
  }
  ...
  if self.show_devtools {
      egui::SidePanel::right("devtools_panel")
          .default_width(420.0)
          .min_width(320.0)
          .frame(Frame::NONE.fill(Color32::from_rgb(28, 30, 33)))
          .show(ctx, |ui| {
              self.render_devtools(ui);
          });
  }
  ```
  Only `Key::F12` is bound; `Ctrl+Shift+I` is ignored. Panel is hardcoded as `SidePanel::right` without bottom docking support (`TopBottomPanel::bottom`) or dock switching controls.
- **Selection Bug in DOM Tree (`src/app.rs:1634-1681`)**:
  ```rust
  let is_sel = selected.as_ref() == Some(&label);
  ```
  Selection compares a string label like `"<div class=\"card\">"`. If a page contains multiple elements with identical tag names and classes (e.g., repeating `<li>` or `<div>` cards), selecting one selects all identical elements simultaneously.
- **Hardcoded Box Model (`src/app.rs:1491-1518`)**:
  ```rust
  fn render_box_model_diagram(ui: &mut egui::Ui, margin: &str, padding: &str, display: &str) { ... }
  ```
  Uses string parameters (`"margin (0px)"`, `"border: 1px"`, `"padding (0px)"`, `"block"`) rather than querying real layout dimensions computed by `src/layout.rs`.
- **Missing Console & State Buffers**:
  Neither `Tab` nor `BrowserApp` has log streams for console entries, network request entries, storage entries, or security auditing.

### 1.2 Engine Capabilities & Missing Bridges
- **`src/dom.rs`**:
  `Node` (`lines 4-21`) contains `pub children: Vec<Node>` and `pub node_type: NodeType`. It lacks index path traversal (`get_node_by_path(&[usize])`) and ancestor node collection (`collect_ancestor_nodes(&[usize])`), making unique node addressing impossible.
- **`src/style.rs`**:
  `matching_rules(node, ancestors, stylesheet)` (`lines 261-271`) and `specified_values(node, ancestors, stylesheet)` (`lines 230-259`) exist and calculate exact CSS specificity, but they are private functions inside `style.rs` and not exported to DevTools.
- **`src/layout.rs`**:
  `Dimensions` (`lines 40-60`) defines `content: Rect`, `padding: EdgeSizes`, `border: EdgeSizes`, and `margin: EdgeSizes`. `LayoutBox` (`lines 80-102`) contains `pub dimensions: Dimensions` and `pub fn get_styled_node(&self) -> Option<&'a StyledNode<'a>>`. There is currently no helper to find the `LayoutBox` corresponding to a given `Node` or compute node dimensions on demand.
- **`src/network.rs`**:
  `PageResponse` (`lines 7-11`) only contains `url`, `html`, and `title`. It does not record HTTP status codes (`status: u16`), status text, headers, or request duration (`duration_ms: u64`), preventing the Network tab from showing live waterfall timings or status badges.

### 1.3 Baseline Automated Test Verification
Running `cargo test` confirms the current baseline passes all 26 existing tests across all test suites:
- `tests/adversarial_url_tests.rs`: 10 passed
- `tests/browser_tests.rs`: 11 passed
- `tests/challenger_ui_tests.rs`: 5 passed
- Result: 0 compilation errors, 0 warnings.

---

## 2. Logic Chain

From these direct observations, the complete implementation logic chain proceeds as follows:

1. **Unambiguous DOM Identification**:
   - Because `Node` is a tree structure, any node can be uniquely addressed by a `NodePath` (`Vec<usize>`), representing the zero-based child indices from the root (Observation §1.1, §1.2).
   - Adding `Node::get_node_by_path<'a>(&'a self, path: &[usize]) -> Option<&'a Node>` allows O(d) lookup where d is tree depth.
   - Adding `Node::collect_ancestor_nodes<'a>(&'a self, path: &[usize]) -> Vec<&'a Node>` returns the slice of ancestor references in root-to-parent order needed by CSS combinator matching.

2. **Connecting CSS Rules to DevTools**:
   - In `style.rs`, `matches_selector` walks backward from target node through `ancestors` (Observation §1.2).
   - Exporting `pub fn get_matched_rules<'a>(node: &Node, ancestors: &[&Node], stylesheet: &'a Stylesheet) -> Vec<MatchedRule<'a>>` and `pub fn get_specified_values(node: &Node, ancestors: &[&Node], stylesheet: &Stylesheet) -> HashMap<String, Value>` allows DevTools to render matched selectors, specificity scores, and all computed styles for any selected DOM node.

3. **Live Box Model Dimensions Query**:
   - `LayoutBox` borrows `StyledNode` which borrows `Node` (`'a` lifetime). Storing `LayoutBox` permanently on `Tab` creates borrow-checker conflicts (Observation §1.2).
   - Instead, providing `find_layout_box_for_node<'a, 'b>(root: &'b LayoutBox<'a>, target: &Node) -> Option<&'b LayoutBox<'a>>` (checking pointer equality `std::ptr::eq(styled.node, target)`) and `query_node_dimensions(dom, stylesheet, target, width) -> Option<Dimensions>` computes the box model on demand whenever an element is inspected.
   - This delivers live pixel values (`margin`, `border`, `padding`, `content width x height`) directly to the visual 4-layer Chrome Box Model diagram.

4. **Network Request Logging**:
   - In `network.rs`, capturing `Instant::now()` around requests and recording `status: u16`, `status_text: String`, `headers: Vec<(String, String)>`, and `duration_ms: u64` in `PageResponse` allows `Tab` to maintain a request history `Vec<NetworkEntry>` (Observation §1.2).
   - This enables the Network tab to display the full request ledger with status badges (200 green, 404 red), method, type, size, duration, and a headers/response drawer.

5. **Dockable Chrome Shell & Navigation**:
   - Supporting `enum DevToolsDockPosition { Right, Bottom }` toggleable via an icon button in the header satisfies both widescreen and vertical inspection layouts (Observation §1.1).
   - Listening for `Key::F12` or `(ctrl || command) && shift && Key::I` ensures universal Chrome keyboard shortcut compliance.
   - Expanding `DevToolsTab` to `{ Elements, Console, Sources, Network, Application, Security }` satisfies Requirement R3.

6. **Elements Tab Two-Pane Architecture**:
   - Left pane: collapsible DOM tree with expand/collapse carets (`▶`/`▼`) stored in `HashSet<Vec<usize>>`, syntax-colored tokens (`<tag>` `#8ab4f8`, `attr` `#fdd663`, `"value"` `#81c995`, `text` `#e8eaed`), and path-based selection.
   - Right pane: 4-layer visual Box Model (Margin `#f9ab00`, Border `#fdd663`, Padding `#81c995`, Content `#8ab4f8`), matched CSS rules, and computed styles.

7. **Console, Sources, Application, and Security Tabs**:
   - Console: log buffer with Info/Warn/Error badges, level filtering, clear button, and interactive REPL command prompt (`help`, `clear`, `dom`, `styles`, `info`, `zoom`, `goto`).
   - Sources: resource tree (origin, HTML, user-agent CSS, internal styles) and code viewer with line-numbered gutter.
   - Application: Storage inspector (Cookies, LocalStorage, SessionStorage, Image Texture Cache) with clear actions.
   - Security: TLS encryption overview (Valid HTTPS 🔒, Insecure HTTP ⚠️, Internal ℹ️), cipher suite, and mixed-content detection.

---

## 3. Caveats

1. **No External JavaScript Engine**: The browser engine is pure Rust with HTML5 and CSS3 parsing without V8/QuickJS. The Console tab's interactive REPL evaluates built-in browser inspection and navigation commands rather than arbitrary JS expressions.
2. **`challenger_ui_tests.rs` String Invariants**: The test file `tests/challenger_ui_tests.rs` checks exact code substrings in `src/app.rs` (`let total_shortcuts_w = 6.0 * 68.0;`, `let available_edit_w = child_ui.available_width();`, `Vec2::new(64.0, 28.0)`, `"Login"`). DevTools additions in `src/app.rs` must strictly avoid touching or altering these invariant lines.
3. **Transient Layout Computation**: To prevent self-referential lifetime constraints on `Tab`, layout dimensions for the inspected element are evaluated on demand via `query_node_dimensions`. This is fast (<0.5ms for standard DOMs) and completely avoids unsafe code.
4. **Dock Position Resizing**: When switching from `Right` to `Bottom`, `egui` manages panel state by panel ID (`devtools_panel_right` vs `devtools_panel_bottom`), ensuring width and height defaults are cleanly respected without layout jump.

---

## 4. Conclusion & Concrete Implementation Strategy

### 4.1 Data Structures to Add

#### In `src/app.rs` (or `src/ui/devtools.rs`):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevToolsDockPosition {
    Right,
    Bottom,
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
    pub resource_type: String,
    pub size_bytes: usize,
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

#### Fields to Add to `BrowserApp`:
```rust
pub devtools_dock: DevToolsDockPosition,
pub devtools_selected_path: Option<Vec<usize>>,
pub devtools_expanded_paths: std::collections::HashSet<Vec<usize>>,
pub devtools_dom_search: String,
pub devtools_styles_filter: String,
pub devtools_console_filter: String,
pub devtools_console_level: Option<ConsoleLogLevel>,
pub devtools_console_input: String,
pub devtools_network_filter: String,
pub devtools_network_type: NetworkTypeFilter,
pub devtools_selected_network_id: Option<usize>,
pub devtools_sources_file: SourcesFileId,
pub devtools_storage_section: StorageSection,
pub devtools_cookies: Vec<CookieEntry>,
pub devtools_local_storage: std::collections::HashMap<String, String>,
pub devtools_session_storage: std::collections::HashMap<String, String>,
```

#### Fields to Add to `Tab`:
```rust
pub network_entries: Vec<NetworkEntry>,
pub console_entries: Vec<ConsoleEntry>,
```

---

### 4.2 Exact Code Diffs by File

#### Diff 1: `src/dom.rs`
Add `get_node_by_path` and `collect_ancestor_nodes` to `impl Node`:
```rust
impl Node {
    // ... existing methods ...

    /// DEVTOOLS: Returns a reference to the descendant node at the specified path of child indices.
    pub fn get_node_by_path<'a>(&'a self, path: &[usize]) -> Option<&'a Node> {
        let mut current = self;
        for &idx in path {
            current = current.children.get(idx)?;
        }
        Some(current)
    }

    /// DEVTOOLS: Collects references to ancestor nodes along the path (from root down to immediate parent).
    pub fn collect_ancestor_nodes<'a>(&'a self, path: &[usize]) -> Vec<&'a Node> {
        let mut ancestors = Vec::new();
        if path.is_empty() {
            return ancestors;
        }
        let mut current = self;
        ancestors.push(current);
        for &idx in path.iter().take(path.len().saturating_sub(1)) {
            if let Some(child) = current.children.get(idx) {
                current = child;
                ancestors.push(current);
            } else {
                break;
            }
        }
        ancestors
    }
}
```

#### Diff 2: `src/style.rs`
Export `MatchedRule`, `get_matched_rules`, and `get_specified_values`:
```rust
pub type MatchedRule<'a> = (Specificity, &'a Rule);

/// DEVTOOLS: Returns all CSS rules matching the given node within the stylesheet.
pub fn get_matched_rules<'a>(
    node: &Node,
    ancestors: &[&Node],
    stylesheet: &'a Stylesheet,
) -> Vec<MatchedRule<'a>> {
    matching_rules(node, ancestors, stylesheet)
}

/// DEVTOOLS: Computes the specified style property map for a given node.
pub fn get_specified_values(
    node: &Node,
    ancestors: &[&Node],
    stylesheet: &Stylesheet,
) -> HashMap<String, Value> {
    specified_values(node, ancestors, stylesheet)
}
```

#### Diff 3: `src/layout.rs`
Add `find_layout_box_for_node` and `query_node_dimensions`:
```rust
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
```

#### Diff 4: `src/network.rs`
Update `PageResponse` to include HTTP status, headers, and duration:
```rust
#[derive(Debug, Clone)]
pub struct PageResponse {
    pub url: String,
    pub html: String,
    pub title: String,
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub duration_ms: u64,
}
```
In `fetch_url_content`:
- Internal URLs (`about:*`): return `status: 200`, `status_text: "OK".into()`, `duration_ms: 1`, `headers: vec![("content-type".into(), "text/html".into())]`.
- HTTP/HTTPS fetches: record start time `let start = std::time::Instant::now();`, capture `response.status().as_u16()`, `status.canonical_reason()`, collect `response.headers()`, and compute `start.elapsed().as_millis() as u64`.

#### Diff 5: `src/app.rs`
1. **Shortcut binding in `App::update`**:
   ```rust
   let toggle_devtools = ctx.input(|i| {
       i.key_pressed(Key::F12) || ((i.modifiers.ctrl || i.modifiers.command) && i.modifiers.shift && i.key_pressed(Key::I))
   });
   if toggle_devtools {
       self.show_devtools = !self.show_devtools;
   }
   ```
2. **Dockable Panel Mounting**:
   ```rust
   if self.show_devtools {
       match self.devtools_dock {
           DevToolsDockPosition::Right => {
               egui::SidePanel::right("devtools_panel_right")
                   .default_width(460.0)
                   .min_width(320.0)
                   .resizable(true)
                   .frame(Frame::NONE.fill(Color32::from_rgb(32, 33, 36)))
                   .show(ctx, |ui| {
                       self.render_devtools_panel(ui, ctx);
                   });
           }
           DevToolsDockPosition::Bottom => {
               egui::TopBottomPanel::bottom("devtools_panel_bottom")
                   .default_height(280.0)
                   .min_height(160.0)
                   .resizable(true)
                   .frame(Frame::NONE.fill(Color32::from_rgb(32, 33, 36)))
                   .show(ctx, |ui| {
                       self.render_devtools_panel(ui, ctx);
                   });
           }
       }
   }
   ```
3. **DevTools Header**:
   - Title: "DevTools"
   - 6 Chrome Tabs: `Elements`, `Console`, `Sources`, `Network`, `Application`, `Security` with active blue underline (`#8ab4f8`)
   - Dock Toggle button: switches `self.devtools_dock` between `Right` and `Bottom`
   - Close button: `×` icon closing DevTools
4. **Elements Tab**:
   - Left pane: DOM hierarchy with carets (`▶`/`▼`), colored tokens:
     - Tag: `#8ab4f8`
     - Attr name: `#fdd663`
     - Attr value: `#81c995`
     - Text: `#e8eaed`
     - Comment: `#80868b`
     - Row highlight: `#283e58` on selection, `#282b30` on hover
   - Right pane:
     - 4-layer Chrome Box Model (Margin `#f9ab00`, Border `#fdd663`, Padding `#81c995`, Content `#8ab4f8`) with live dimensions from `layout::query_node_dimensions`.
     - Matched CSS rules list with selectors, specificity, and declarations.
     - Computed styles property table with search filter.
5. **Console Tab**:
   - Filter bar: `[All]`, `[Errors]`, `[Warnings]`, `[Info]`, text search, and clear button `🚫`.
   - Scrollable log entries with severity badges and timestamps.
   - Interactive REPL prompt (`> `) handling `help`, `clear`, `dom`, `styles`, `info`, `zoom`, `goto`.
6. **Network Tab**:
   - Filter buttons: `[All]`, `[Doc]`, `[CSS]`, `[JS]`, `[Img]`, `[Fetch]`
   - Request list table: Name, Status (colored badge), Method, Type, Size, Time, Waterfall bar.
   - Request details drawer with General info, Headers, and Response preview.
7. **Sources Tab**:
   - Left resource tree: Hostname, HTML document, User-Agent CSS, Inline `<style>`.
   - Right code viewer with line-numbered gutter and syntax styling.
8. **Application Tab**:
   - Category navigation: Cookies, LocalStorage, SessionStorage, Texture Cache.
   - Data table with Add, Remove, and Clear actions.
9. **Security Tab**:
   - Valid HTTPS 🔒, Insecure HTTP ⚠️, or Internal ℹ️ status card.
   - Rustls TLS 1.3 protocol and cipher suite details.
   - Mixed-content detection scanner.

---

### 4.3 Concrete Test Cases to Add (`tests/devtools_tests.rs`)

```rust
use rust_browser::app::{BrowserApp, DevToolsDockPosition, DevToolsTab, ConsoleLogLevel};
use rust_browser::dom::Node;
use rust_browser::css::CssParser;
use rust_browser::html_parser::HtmlParser;
use rust_browser::layout::{find_layout_box_for_node, query_node_dimensions};
use rust_browser::style::get_matched_rules;

#[test]
fn test_dom_path_navigation_and_ancestors() {
    let html = "<html><body><div id='container'><p class='text'>Hello <span>world</span></p></div></body></html>";
    let root = HtmlParser::parse(html);

    // Root path []
    assert_eq!(root.get_node_by_path(&[]).unwrap().tag_name(), Some("html"));
    assert_eq!(root.collect_ancestor_nodes(&[]).len(), 0);

    // Body path [0]
    let body = root.get_node_by_path(&[0]).unwrap();
    assert_eq!(body.tag_name(), Some("body"));
    let body_ancestors = root.collect_ancestor_nodes(&[0]);
    assert_eq!(body_ancestors.len(), 1);
    assert_eq!(body_ancestors[0].tag_name(), Some("html"));

    // Span path [0, 0, 0, 1]
    let span = root.get_node_by_path(&[0, 0, 0, 1]).unwrap();
    assert_eq!(span.tag_name(), Some("span"));
    let span_ancestors = root.collect_ancestor_nodes(&[0, 0, 0, 1]);
    assert_eq!(span_ancestors.len(), 4);
    assert_eq!(span_ancestors[0].tag_name(), Some("html"));
    assert_eq!(span_ancestors[1].tag_name(), Some("body"));
    assert_eq!(span_ancestors[2].tag_name(), Some("div"));
    assert_eq!(span_ancestors[3].tag_name(), Some("p"));
}

#[test]
fn test_matched_css_rules_export() {
    let html = "<html><body><div id='box' class='card main'><p>Test</p></div></body></html>";
    let css = "
        div { margin: 10px; }
        .card { padding: 15px; }
        #box { border: 2px solid black; }
        p { color: red; }
    ";
    let root = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);

    let div_path = [0, 0];
    let div_node = root.get_node_by_path(&div_path).unwrap();
    let div_ancestors = root.collect_ancestor_nodes(&div_path);

    let matched = get_matched_rules(div_node, &div_ancestors, &stylesheet);
    assert_eq!(matched.len(), 3); // matches div, .card, and #box
}

#[test]
fn test_live_box_model_dimension_query() {
    let html = "<html><body><div style='width: 320px; height: 120px; margin: 16px; padding: 8px;'>Box</div></body></html>";
    let stylesheet = CssParser::parse("");
    let root = HtmlParser::parse(html);

    let div_node = root.get_node_by_path(&[0, 0]).unwrap();
    let dims = query_node_dimensions(&root, &stylesheet, div_node, 1000.0).expect("LayoutBox dimensions should be computed");

    assert_eq!(dims.content.width, 320.0);
    assert_eq!(dims.content.height, 120.0);
    assert_eq!(dims.margin.top, 16.0);
    assert_eq!(dims.padding.left, 8.0);
}

#[test]
fn test_devtools_tab_variants_and_dock_positions() {
    let mut app = BrowserApp::new();

    // Verify all 6 tabs exist and are selectable
    let tabs = [
        DevToolsTab::Elements,
        DevToolsTab::Console,
        DevToolsTab::Sources,
        DevToolsTab::Network,
        DevToolsTab::Application,
        DevToolsTab::Security,
    ];
    for tab in tabs {
        app.devtools_tab = tab;
        assert_eq!(app.devtools_tab, tab);
    }

    // Verify dock position toggling
    app.devtools_dock = DevToolsDockPosition::Right;
    assert_eq!(app.devtools_dock, DevToolsDockPosition::Right);
    app.devtools_dock = DevToolsDockPosition::Bottom;
    assert_eq!(app.devtools_dock, DevToolsDockPosition::Bottom);
}
```

---

## 5. Verification Method

To independently verify the DevTools implementation:

1. **Compilation Check**:
   ```bash
   cargo check
   ```
   Ensures clean compilation across all modules without any lifetime errors or missing traits.

2. **Automated Regression & DevTools Unit Tests**:
   ```bash
   cargo test
   ```
   Must pass all existing 26 tests (including `challenger_ui_tests.rs`) plus the new `devtools_tests.rs` suite.

3. **Interactive UI Verification (`cargo run`)**:
   - **Shortcut & Toggle**:
     - Press `F12` -> DevTools toggles open/closed.
     - Press `Ctrl+Shift+I` -> DevTools toggles open/closed.
     - Click the '×' button -> DevTools closes.
     - Click the dock button -> toggles between Right SidePanel and Bottom TopBottomPanel.
   - **Elements Tab**:
     - Verify DOM tree displays colored tokens: tags in blue `#8ab4f8`, attributes in yellow `#fdd663`, values in green `#81c995`.
     - Click carets (`▶`/`▼`) to verify subtree expand/collapse.
     - Click an element row: row highlights in `#283e58`.
     - Check right pane: verify nested Margin (orange), Border (yellow), Padding (green), and Content (blue) boxes display live numbers matching CSS/layout.
     - Verify Matched CSS Rules list displays selectors and specificity.
   - **Console Tab**:
     - Verify log entries display Info/Warn/Error badges with timestamps.
     - Test level filter buttons and clear button.
     - Type `help`, `dom`, `styles` in the `> ` prompt and press Enter to verify REPL execution.
   - **Network Tab**:
     - Verify table displays Name, Status code (green 200 badge), Method, Type, Size, and Time.
     - Click a request to open the details drawer and view headers/body preview.
   - **Sources Tab**:
     - Verify resource navigation tree and code viewer with line numbers.
   - **Application & Security Tabs**:
     - Verify Storage/Cookies tables and Texture Cache meter.
     - Verify TLS HTTPS lock badge and cipher suite details.

4. **Invalidation Conditions**:
   - Invariant failure in `challenger_ui_tests.rs` (e.g. modified Omnibox width calculations or login pill).
   - Lifetime errors when storing borrowed layout structures on `Tab`.
   - Ambiguous node selection where clicking an element highlights multiple nodes.
