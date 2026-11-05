# Project: Rust Browser UI Polish, Chrome 3-Dots Menu & Native DevTools Panel

## Architecture
Aether is a pure-Rust web browser built with `eframe` (egui) and standard networking crates.
- UI layer (`src/app.rs`, `src/main.rs`): Renders the browser frame, tabs, Omnibox, navigation toolbar, Chrome 3-dots popup menu, and Chrome-style DevTools panel.
- Engine layer (`src/dom.rs`, `src/style.rs`, `src/layout.rs`, `src/network.rs`): Provides DOM hierarchy, CSS rule matching, layout dimension calculations, and network request handling.
- Test suite (`tests/`): Automated regression, adversarial, and integration tests verifying parsers, layout calculations, URL normalization, UI layout invariants, and DevTools features.

## Feature Inventory
| # | Feature | Description | Milestone | Source | Status |
|---|---------|-------------|-----------|--------|--------|
| 1 | Tab Title Selection Prevention | Tab title text labels cannot be highlighted or selected via mouse click/drag (`egui::Label::selectable(false)`), allowing clean tab switching and window dragging. | M2 | ORIGINAL_REQUEST § R1 | DONE |
| 2 | 4-Corner Rounded Tab Pills | Every tab pill in the tab strip has all four corners rounded (`CornerRadius::same(8)`). | M2 | ORIGINAL_REQUEST § R1 | DONE |
| 3 | 4-Corner Rounded Window Frame | Frameless browser window has all four corners rounded (`CornerRadius::same(10)`) with transparent viewport configuration (`with_transparent(true)`) and 1px border stroke. | M2 | ORIGINAL_REQUEST § R1 | DONE |
| 4 | Chrome 3-Dots Menu Trigger & Frame | Vertical 3-dots button (⋮) on top-right toolbar toggles dark-themed floating popup menu (`#292a2d`, `CornerRadius::same(8)`, 1px border stroke, drop shadow). | M2 | ORIGINAL_REQUEST § R2 | DONE |
| 5 | Chrome 3-Dots Menu Full Catalog | Menu includes profile card ("Khoa", "Signed in"), navigation rows (New tab, New window, New Incognito window), tool rows (History, Downloads, Passwords, Bookmarks, Extensions, Clear browsing data), page actions (Print, Search, Translate), and exit action with dividers. | M2 | ORIGINAL_REQUEST § R2 | DONE |
| 6 | Chrome 3-Dots Menu Right-Aligned Shortcuts | Keyboard shortcut hints (`Ctrl+T`, `Ctrl+N`, `Ctrl+Shift+N`, `Ctrl+H`, `Ctrl+J`, `Ctrl+Shift+Del`, `Ctrl+P`) rendered right-aligned in muted grey (`#9aa0a6`). | M2 | ORIGINAL_REQUEST § R2 | DONE |
| 7 | Interactive Zoom Control Row | Composite zoom row in 3-dots menu with `[-]`, percentage display (e.g. `100%`), `[+]`, and fullscreen toggle `⛶` with discrete zoom stepping; keeps menu open when clicked. | M2 | ORIGINAL_REQUEST § R2 | DONE |
| 8 | 3-Dots Menu Dismissal Logic | Menu automatically closes when clicking outside the menu rect or pressing Escape; closes upon executing standard menu actions. | M2 | ORIGINAL_REQUEST § R2 | DONE |
| 9 | Native DevTools Shell & Docking | Dockable DevTools container supporting Right SidePanel and Bottom TopBottomPanel, toggleable via `F12`, `Ctrl+Shift+I`, 3-dots menu "More tools -> Developer Tools", and close button (`×`). | M2 | ORIGINAL_REQUEST § R3 | DONE |
| 10 | DevTools 6-Tab Chrome Navigation Bar | Header tab bar supporting Elements, Console, Sources, Network, Application, and Security tabs styled in Chrome DevTools dark theme with active blue underline (`#8ab4f8`). | M2 | ORIGINAL_REQUEST § R3 | DONE |
| 11 | DevTools Elements Tab DOM Tree | Two-pane layout with left pane rendering interactive DOM tree with colored syntax tokens, expand/collapse carets (`▶`/`▼`), and path-based unique node selection (`Vec<usize>`). | M2 | ORIGINAL_REQUEST § R3 | DONE |
| 12 | DevTools Elements Tab Live Box Model & Styles | Right pane rendering visual 4-layer Chrome Box Model diagram (Margin, Border, Padding, Content) with live dimensions, and matched CSS rules inspector. | M2 | ORIGINAL_REQUEST § R3 | DONE |
| 13 | DevTools Console Tab | Console log buffer with Info, Warn, Error severity badges, level filtering, clear button, and interactive REPL command prompt. | M2 | ORIGINAL_REQUEST § R3 | DONE |
| 14 | DevTools Network Tab | HTTP request table with columns (Name, Status badge, Method, Type, Size, Time) and detail inspector drawer. | M2 | ORIGINAL_REQUEST § R3 | DONE |
| 15 | DevTools Sources Tab | Resource navigation tree (HTML, CSS) and code viewer with line-numbered gutter and syntax styling. | M2 | ORIGINAL_REQUEST § R3 | DONE |
| 16 | DevTools Application & Security Tabs | Storage/cookies/cache inspector and TLS/HTTPS security status overview. | M2 | ORIGINAL_REQUEST § R3 | DONE |
| 17 | Test Suite Verification & Expansion | Full workspace compiles with `cargo check` and passes all existing and new automated tests with `cargo test` (59 passed across 7 suites). | M2 | ORIGINAL_REQUEST § Verification | DONE |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | UI Simplification & Google Search Refinement | Features 1-12 from previous request | none | DONE |
| M2 | Browser UI Polish, Chrome 3-Dots Menu & Native DevTools Panel | Features 1-17 across `src/main.rs`, `src/app.rs`, `src/dom.rs`, `src/style.rs`, `src/layout.rs`, `src/network.rs`, `tests/` | M1 | DONE |

## Interface Contracts
### `src/main.rs`
- `NativeOptions.viewport`: configured with `with_transparent(true)` and `with_decorations(false)`.

### `src/app.rs`
- Tab title labels: `egui::Label::new(...).selectable(false).sense(egui::Sense::click())`.
- Tab frames: `CornerRadius::same(8)`.
- Browser outer frame: `CornerRadius::same(10)` with 1px border stroke (`#3c4043`) when unmaximized.
- 3-Dots popup menu: floating window anchored top-right with dark styling, profile banner ("Khoa", "Signed in"), full command catalog, right-aligned shortcuts, 17-step zoom controls, and outside click / Escape dismissal.
- DevTools panel: dockable panel (Right or Bottom) with 6 tabs, F12 / Ctrl+Shift+I toggle, and close button.

### `src/dom.rs` ↔ DevTools
- Path-based node navigation: `node.get_node_by_path(&[usize]) -> Option<&Node>`.
- Ancestor node collection for CSS cascading: `node.collect_ancestor_nodes(&[usize]) -> Vec<&Node>`.

### `src/style.rs` ↔ DevTools
- Matched rules helper: `pub fn get_matched_rules<'a>(node: &Node, ancestors: &[&Node], stylesheet: &'a Stylesheet) -> Vec<(Specificity, &'a Rule)>`.
- Specified values helper: `pub fn get_specified_values(node: &Node, ancestors: &[&Node], stylesheet: &Stylesheet) -> HashMap<String, Value>`.

### `src/layout.rs` ↔ DevTools
- Box model lookup: `pub fn find_layout_box_for_node<'a, 'b>(root: &'b LayoutBox<'a>, target: &Node) -> Option<&'b LayoutBox<'a>>`.
- Dimension query: `pub fn query_node_dimensions(dom: &Node, stylesheet: &Stylesheet, target: &Node, width: f32) -> Option<Dimensions>`.

## Code Layout
- `src/main.rs`: Entry point and viewport transparency configuration.
- `src/app.rs`: Tab strip, navigation bar, 3-dots popup menu, DevTools panel UI container, and event dispatch.
- `src/dom.rs`: DOM data structures and path navigation helpers.
- `src/style.rs`: CSS cascading and matched rules export.
- `src/layout.rs`: Layout box tree and box model dimension queries.
- `src/network.rs`: URL normalization, request handling, and network entry logging.
- `tests/`:
  - `browser_tests.rs`: Integration tests for URL normalization and search routing.
  - `adversarial_url_tests.rs`: Adversarial stress tests and fuzzing for URL/domain normalization.
  - `challenger_ui_tests.rs`: Adversarial UI layout and invariant tests.
  - `chrome_menu_tests.rs`: Chrome 3-dots menu catalog, zoom stepping ladder, and source invariant tests.
  - `devtools_tests.rs`: DevTools data structures, DOM path navigation, and Box Model query tests.
  - `adversarial_menu_geometry_tests.rs`: Adversarial zoom ladder and menu geometry tests.
  - `challenger_devtools_stress_tests.rs`: DevTools deep DOM, CSS cascading, and REPL stress tests.
