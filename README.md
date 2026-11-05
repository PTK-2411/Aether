# 🦀 Aether — 100% Pure Rust Web Browser (Zero Chromium)

**Aether** is an independent, lightweight web browser built entirely from scratch in **Rust**, with zero dependencies on Chromium, Blink, WebKit, Gecko, CEF, or Electron.

The project implements the complete standard web browser pipeline: from secure HTTP(S) networking over Rustls, an HTML Tokenizer and DOM tree builder, a CSS parser with W3C specificity cascading and inheritance, a CSS Box Model and inline text flow layout engine, multithreaded image rendering (`<img>`), an optimized Display List painter, real-time search autocomplete suggestions, to a custom borderless dark theme UI and interactive DevTools Inspector (F12).

---

## 🏗 Browser Engine Pipeline

```
[ URL / Input ] 
       │
       ▼
1. Network Stack (network.rs) ──► Loads HTML over HTTP/HTTPS (Rustls) or internal protocols (about:, file://)
       │
       ▼
2. HTML Parser & DOM (html_parser.rs, dom.rs) ──► Tokenizes tags, attributes, void elements & builds DOM tree
       │
       ▼
3. CSS Parser & Cascade (css.rs, style.rs, default_style.rs) ──► Loads UA styles, matches selectors & computes specificity
       │
       ▼
4. Box Model Layout (layout.rs) ──► Resolves Content, Padding, Border, Margin, auto-centering, Word-Wrapping & image sizing
       │
       ▼
5. Display List & Painter (render.rs) ──► Translates layout into GPU draw commands, renders images & performs link hit-testing
       │
       ▼
6. Browser Chrome & DevTools (app.rs, main.rs) ──► Custom borderless window, Search Suggestions, Omnibox & F12 DevTools
```

---

## 🌟 Key Features

### 1. Complete Rendering Engine Pipeline
- **HTML Tokenizer & DOM Builder**:
  - Automatically parses standard HTML markup into a complete DOM object tree.
  - Handles void self-closing elements (`img`, `meta`, `link`, `br`, `hr`, `input`).
  - Correctly extracts raw text contents inside `<style>` and `<script>` blocks.
  - Decodes HTML entities (`&amp;`, `&lt;`, `&gt;`, `&copy;`, `&quot;`, `&#39;`).
  - Automatically hides metadata elements (`<head>`, `<title>`, `<style>`, `<script>`, `<meta>`, `<link>`, `<noscript>`, `<template>`).
  - Filters out invisible zero-width and bidirectional formatting characters (`\u{200E}`, `\u{200F}`, `\u{200B}`, `\u{FEFF}`) to prevent missing glyph box glyphs.
- **CSS Cascade & Style Engine**:
  - Selector matching: Universal `*`, Type/Tag (`h1`, `p`, `div`), Class (`.card`), ID (`#main`), Combinators (`div > p`, `nav a`).
  - Precise W3C specificity calculations: `(ID, Class, Tag)`.
  - Shorthand property expansion: `margin`, `padding`, `border`, `background`.
  - Style inheritance down the DOM hierarchy for typography, font size, and colors.
- **CSS Box Model & Flow Layout**:
  - Accurately computes all four box model layers: Content, Padding, Border, and Margin.
  - Implements automatic block centering with `margin: 0 auto`.
  - Intelligent inline flow layout with natural word-wrapping and calibrated character metrics.
- **Image Rendering (`<img>`)**:
  - Automatically parses `<img>` tags and respects explicit attributes (`width`, `height`) and CSS dimensions.
  - Background multithreaded fetching and decoding for PNG, JPEG, WebP, and Base64 Data URIs without freezing the UI.
  - Elegant vector placeholder fallback with `alt` text preview during network loading or offline states.
- **Display List & Hit Testing**:
  - Generates optimized draw commands before dispatching to the painter.
  - Detects mouse hover over hyperlinks (`<a>`), switches the cursor to `PointingHand`, and navigates upon click.

### 2. Aether Custom UI & Real-Time Search Suggestions
- **Custom Borderless Window (No OS Title Bar)**:
  - Native OS title bar disabled (`with_decorations(false)`) for a clean, unified browser window.
  - Built-in vector window controls: **Minimize**, **Maximize / Restore**, and **Close** with smooth hover highlights.
  - Seamless window dragging on empty tab bar space and double-click to maximize/restore.
- **Aether Dark Theme & Modern Start Page**:
  - Sleek dark aesthetic with capsule Omnibox, vector icons, and customizable quick shortcuts (Wikipedia, YouTube, GitHub, Rust Lang, Aether AI, Reddit, DuckDuckGo).
  - 100% Vector Icons: All controls (navigation chevrons, close tab 'x', magnifying glass, mic, camera, menus) are drawn geometrically with vector paths, eliminating missing symbol glyphs.
- **Real-Time Search Suggestions**:
  - Typing in the Omnibox or the New Tab search box displays a floating suggestion dropdown.
  - Connected directly to search autocomplete APIs with an offline fallback dictionary.
  - Click any suggestion to instantly search.

### 3. Built-in Developer Tools (F12 Inspector)
- **Clearly Divided Panels**:
  - **DOM Hierarchy Tree Card**: Expandable and selectable DOM node tree with syntax-colored tags, IDs, classes, and text previews.
  - **Selected Element Inspector Card**:
    - **Chrome-style Box Model Visualizer**: Displays concentric layers for Margin (orange), Border (yellow), Padding (green), and Content (blue) with real-time dimensional values.
    - **HTML Attributes Table**: Key-value inspector for element attributes (`id`, `class`, `src`, `href`).
    - **Computed Styles Table**: Inspects computed properties (`display`, `color`, `font-size`, `background`).
- **CSS Rules Cascade Inspector**: Lists parsed stylesheet rules, selector specificity scores, and declaration blocks.
- **HTML Source Viewer**: Raw document viewer with byte metrics.
- **Diagnostics & Network**: Displays connection protocols, Rustls TLS state, document byte size, render height, and engine status.

### 4. Built-in Internal Pages
- `about:newtab`: Beautiful Aether Dark Mode start page with smart search and quick shortcuts.
- `about:ai`: Dedicated **Aether AI Mode** assistant with preloaded research prompts and architecture insights.
- `about:welcome`: Modern welcome dashboard detailing technical specifications and navigation guides.
- `about:test` / `about:acid`: Comprehensive CSS and DOM test suite (cards, buttons, blockquotes, syntax blocks).
- `about:credits`: In-depth documentation of the browser engine architecture and standard web pipeline.
- Built-in **Aether Search**: Automatically queries and synthesizes verified encyclopedia knowledge from Wikipedia.

---

## 🚀 Getting Started

### Prerequisites
- Operating System: Linux (X11 / Wayland), macOS, or Windows.
- [Rust toolchain](https://rustup.rs/) (version 1.80+ or later).

### Build & Run

Run Aether in release mode:
```bash
cargo run --release
```

Or run in debug mode:
```bash
cargo run
```

The **Aether** browser window will launch with the dark theme New Tab page and custom window controls!

### Running Unit Tests

Run the comprehensive test suite:
```bash
cargo test
```

All 11 unit and integration tests validate the HTML parser, void elements, CSS specificity, shorthands, style cascading, box model calculations, and image layout:
```
running 11 tests
test test_html_entities_and_void_tags ... ok
test test_css_shorthands_and_colors ... ok
test test_html_parser_basic ... ok
test test_css_parser_rules_and_specificity ... ok
test test_html_style_raw_content ... ok
test test_layout_box_model_calculation ... ok
test test_style_cascading_and_inheritance ... ok
test test_url_normalization_and_resolution ... ok
test test_image_layout_and_display_item ... ok
test test_metadata_elements_hidden ... ok
test test_text_word_spacing ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

---

## 📂 Project Structure

```
Rust-browser/
├── Cargo.toml               # Project manifest (eframe, reqwest with rustls-tls, url, image)
├── README.md                # Project documentation and architectural guide
├── src/
│   ├── lib.rs               # Library root and module exports
│   ├── main.rs              # Application entrypoint (eframe borderless window configuration)
│   ├── app.rs               # Browser UI: Tabs, Omnibox, Search Suggestions, DevTools, Aether New Tab
│   ├── dom.rs               # DOM tree structures: Node, NodeType, ElementData, Attribute map
│   ├── html_parser.rs       # Pure-Rust HTML tokenizer and DOM tree constructor
│   ├── css.rs               # CSS parser: Selectors, specificity, colors, and shorthand properties
│   ├── default_style.rs     # User-Agent default stylesheet for standard HTML elements
│   ├── style.rs             # Style tree generator: Cascading, specificity calculation & inheritance
│   ├── layout.rs            # CSS Box Model layout engine & flow inline word-wrapping
│   ├── render.rs            # Display list generator, image rendering, and link hit-testing
│   └── network.rs           # Network fetcher (Rustls HTTP/HTTPS, Aether Search, about:, file://)
└── tests/
    └── browser_tests.rs     # Automated test suite covering the entire rendering engine
```

---

## 🛡️ License & Principles

Aether is an open-source systems programming project built to demonstrate that a standards-compliant, lightweight, and memory-safe web browser can be engineered purely in Rust without relying on monolithic engines such as Chromium, Blink, or WebKit.
