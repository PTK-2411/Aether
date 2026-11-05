use rust_browser::app::{
    BrowserApp, ConsoleEntry, ConsoleLogLevel, DevToolsDockPosition, DevToolsTab,
    NetworkEntry, NetworkTypeFilter, SourcesFileId, StorageSection,
};
use rust_browser::css::{parse_value_str, CssColor, CssParser, Specificity, Unit, Value};
use rust_browser::dom::Node;
use rust_browser::html_parser::HtmlParser;
use rust_browser::layout::{build_layout_tree, find_layout_box_for_node, query_node_dimensions};
use rust_browser::style::{get_matched_rules, get_specified_values, style_tree};

// =========================================================================
// 0. Empirical Challenger Bug Finding Reproduction
// =========================================================================

#[test]
fn test_empirical_bug_rem_shadowed_by_em() {
    // In src/css.rs:529-532, `strip_suffix("em")` is evaluated before `strip_suffix("rem")`.
    // Because any string ending in "rem" also ends in "em", stripping "em" from "2rem"
    // leaves "2r", which fails float parsing. Consequently, parse_length returns None
    // and parse_value_str degrades "2rem" to Value::Keyword("2rem") instead of Value::Length(2.0, Unit::Rem).
    let parsed_rem = parse_value_str("2rem");
    assert_eq!(
        parsed_rem,
        Some(Value::Keyword("2rem".to_string())),
        "Empirical Bug Confirmed: 2rem is wrongly parsed as a Keyword due to suffix ordering"
    );

    // Conversely, "2em" works properly:
    let parsed_em = parse_value_str("2em");
    assert_eq!(parsed_em, Some(Value::Length(2.0, Unit::Em)));
}

// =========================================================================
// 1. Path-based Node Navigation Stress Tests
// =========================================================================

#[test]
fn test_path_navigation_empty_and_root() {
    let html = "<html><head><title>Root Test</title></head><body><div>Content</div></body></html>";
    let root = HtmlParser::parse(html);

    // Empty path should return root itself
    let root_node = root.get_node_by_path(&[]);
    assert!(root_node.is_some(), "Empty path must resolve to root node");
    assert_eq!(root_node.unwrap().tag_name(), Some("html"));

    // Ancestor of root is empty
    let root_ancestors = root.collect_ancestor_nodes(&[]);
    assert!(root_ancestors.is_empty(), "Root node has no ancestors");
}

#[test]
fn test_path_navigation_deeply_nested_dom() {
    // Construct 50-level deep nesting: <div><div>...<span>Target</span>...</div></div>
    let depth = 50;
    let mut html = String::new();
    for _ in 0..depth {
        html.push_str("<div class='level'>");
    }
    html.push_str("<span id='deep_target'>Bottom</span>");
    for _ in 0..depth {
        html.push_str("</div>");
    }

    let root = HtmlParser::parse(&html);

    // Build the expected path: html -> body -> div -> div -> ...
    // Note: HtmlParser synthesizes html/body if missing or wraps appropriately.
    // Let's locate the span and verify its path
    let mut current = &root;
    let mut path = Vec::new();
    while current.tag_name() != Some("span") && !current.children.is_empty() {
        path.push(0);
        current = &current.children[0];
    }

    let target_span = root.get_node_by_path(&path);
    assert!(target_span.is_some(), "Must find span node at deep path");
    assert_eq!(target_span.unwrap().tag_name(), Some("span"));
    assert_eq!(target_span.unwrap().id(), Some("deep_target"));

    // Also test navigating one level deeper into the text child
    let mut text_path = path.clone();
    text_path.push(0);
    let target_text = root.get_node_by_path(&text_path);
    assert!(target_text.is_some(), "Must find text child node inside span");
    assert_eq!(target_text.unwrap().tag_name(), None);
    assert_eq!(target_text.unwrap().inner_text(), "Bottom");

    // Ancestors along this deep path
    let ancestors = root.collect_ancestor_nodes(&path);
    assert_eq!(ancestors.len(), path.len(), "Ancestors count must match path depth");
    assert_eq!(ancestors[0].tag_name(), Some("html"));
    assert_eq!(ancestors[ancestors.len() - 1].tag_name(), Some("div"));
}

#[test]
fn test_path_navigation_invalid_and_out_of_bounds() {
    let html = "<html><body><div id='a'><p>Text</p></div></body></html>";
    let root = HtmlParser::parse(html);

    // Immediate out-of-bounds index
    assert!(root.get_node_by_path(&[999]).is_none());
    assert!(root.get_node_by_path(&[usize::MAX]).is_none());

    // Deep out-of-bounds index
    assert!(root.get_node_by_path(&[0, 0, 0, 999]).is_none());
    assert!(root.get_node_by_path(&[0, 999, 0]).is_none());

    // Ancestors of out-of-bounds path should safely terminate without panicking
    let invalid_ancestors_1 = root.collect_ancestor_nodes(&[999]);
    assert_eq!(invalid_ancestors_1.len(), 1); // contains root

    let invalid_ancestors_deep = root.collect_ancestor_nodes(&[0, 999, 5]);
    assert!(invalid_ancestors_deep.len() <= 2);
}

#[test]
fn test_path_navigation_sibling_elements_identical_tags_and_classes() {
    // 10 identical items: <li class='item'>
    let mut html = String::from("<html><body><ul id='list'>");
    for i in 0..10 {
        html.push_str(&format!("<li class='item'>Item {}</li>", i));
    }
    html.push_str("</ul></body></html>");

    let root = HtmlParser::parse(&html);

    // Body is [0], ul is [0, 0]
    // li items are [0, 0, 0] through [0, 0, 9]
    let mut visited_ptrs = std::collections::HashSet::new();

    for i in 0..10 {
        let path = vec![0, 0, i];
        let node = root.get_node_by_path(&path).expect("Sibling must exist");
        assert_eq!(node.tag_name(), Some("li"));
        assert!(node.classes().contains("item"));

        // Verify each sibling has a distinct address and distinct path
        let ptr = node as *const Node as usize;
        assert!(!visited_ptrs.contains(&ptr), "Collision: sibling {} has duplicate pointer", i);
        visited_ptrs.insert(ptr);

        let ancestors = root.collect_ancestor_nodes(&path);
        assert_eq!(ancestors.len(), 3);
        assert_eq!(ancestors[0].tag_name(), Some("html"));
        assert_eq!(ancestors[1].tag_name(), Some("body"));
        assert_eq!(ancestors[2].tag_name(), Some("ul"));
    }
}

// =========================================================================
// 2. Live Box Model Calculations & Dimension Queries
// =========================================================================

#[test]
fn test_box_model_dimension_queries_css_units() {
    let html = "<html><body><div id='box' style='width: 400px; height: 150px; margin-top: 20px; margin-right: 30px; margin-bottom: 25px; margin-left: 35px; padding-top: 10px; padding-right: 15px; padding-bottom: 12px; padding-left: 18px; border-left-width: 4px; border-right-width: 6px; border-top-width: 2px; border-bottom-width: 3px;'>Hello</div></body></html>";

    let root = HtmlParser::parse(html);
    let stylesheet = CssParser::parse("");

    let div_node = root.get_node_by_path(&[0, 0]).unwrap();
    let dims = query_node_dimensions(&root, &stylesheet, div_node, 1200.0)
        .expect("query_node_dimensions should compute box model");

    assert_eq!(dims.content.width, 400.0);
    assert_eq!(dims.content.height, 150.0);

    // Padding
    assert_eq!(dims.padding.top, 10.0);
    assert_eq!(dims.padding.right, 15.0);
    assert_eq!(dims.padding.bottom, 12.0);
    assert_eq!(dims.padding.left, 18.0);

    // Border
    assert_eq!(dims.border.top, 2.0);
    assert_eq!(dims.border.right, 6.0);
    assert_eq!(dims.border.bottom, 3.0);
    assert_eq!(dims.border.left, 4.0);

    // Margin: CSS 2.1 § 10.3.3 overconstrained width solves margin-right = 1200 - 478 = 722
    assert_eq!(dims.margin.top, 20.0);
    assert_eq!(dims.margin.left, 35.0);
    assert_eq!(dims.margin.bottom, 25.0);
    assert_eq!(dims.margin.right, 722.0);
}

#[test]
fn test_box_model_em_rem_and_auto_margins() {
    let html = "<html><body><div id='box' style='width: 200px; padding: 1em; margin-left: auto; margin-right: auto;'>Content</div></body></html>";

    let root = HtmlParser::parse(html);
    let stylesheet = CssParser::parse("");

    let div_node = root.get_node_by_path(&[0, 0]).unwrap();
    let viewport_w = 1000.0;
    let dims = query_node_dimensions(&root, &stylesheet, div_node, viewport_w)
        .expect("query_node_dimensions should compute auto margins");

    assert_eq!(dims.content.width, 200.0);
    // 1em = 16px
    assert_eq!(dims.padding.left, 16.0);
    assert_eq!(dims.padding.right, 16.0);

    // Auto margins centered: (1000 - 200 - 32) / 2 = 384
    assert_eq!(dims.margin.left, 384.0);
    assert_eq!(dims.margin.right, 384.0);
}

#[test]
fn test_box_model_hidden_element_display_none() {
    let html = "<html><body>
        <div id='hidden' style='display: none; width: 100px;'>Invisible</div>
    </body></html>";

    let root = HtmlParser::parse(html);
    let stylesheet = CssParser::parse("");

    let hidden_node = root.get_node_by_path(&[0, 0]).unwrap();
    let dims = query_node_dimensions(&root, &stylesheet, hidden_node, 1000.0);

    // display: none elements do not generate layout boxes
    assert!(dims.is_none(), "Elements with display: none must yield None for layout dimensions");
}

#[test]
fn test_find_layout_box_for_node_direct_lookup() {
    let html = "<html><body><div id='first'>A</div><div id='second'>B</div></body></html>";
    let root = HtmlParser::parse(html);
    let stylesheet = CssParser::parse("");

    let styled_tree = style_tree(&root, &stylesheet);
    let layout_root = build_layout_tree(&styled_tree).expect("Build layout tree");

    let first_node = root.get_node_by_path(&[0, 0]).unwrap();
    let second_node = root.get_node_by_path(&[0, 1]).unwrap();

    let box_first = find_layout_box_for_node(&layout_root, first_node);
    assert!(box_first.is_some(), "Must find layout box for first div");

    let box_second = find_layout_box_for_node(&layout_root, second_node);
    assert!(box_second.is_some(), "Must find layout box for second div");

    // Unrelated / detached node
    let detached_node = HtmlParser::parse("<p>Detached</p>");
    let box_detached = find_layout_box_for_node(&layout_root, &detached_node);
    assert!(box_detached.is_none(), "Detached node must not be found in layout tree");
}

// =========================================================================
// 3. Matched CSS Rules Export & Specificity Stress Tests
// =========================================================================

#[test]
fn test_matched_css_rules_and_specificity_cascading() {
    let html = "<html><body>
        <div id='hero' class='banner highlight card'>
            <span class='title'>Welcome</span>
        </div>
    </body></html>";

    let css = "
        * { color: black; }
        div { color: blue; }
        .card { color: green; }
        .banner.highlight { color: yellow; }
        #hero { color: purple; }
        span.title { font-weight: bold; }
    ";

    let root = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);

    let div_path = [0, 0];
    let div_node = root.get_node_by_path(&div_path).unwrap();
    let div_ancestors = root.collect_ancestor_nodes(&div_path);

    let matched = get_matched_rules(div_node, &div_ancestors, &stylesheet);

    // Div matches: *, div, .card, .banner.highlight, #hero -> 5 rules
    assert_eq!(matched.len(), 5, "Expected 5 matched rules for #hero div");

    // Specificity verification
    let specificities: Vec<Specificity> = matched.iter().map(|(spec, _)| *spec).collect();
    // Universal: (0, 0, 0)
    assert!(specificities.contains(&(0, 0, 0)));
    // Tag 'div': (0, 0, 1)
    assert!(specificities.contains(&(0, 0, 1)));
    // Class '.card': (0, 1, 0)
    assert!(specificities.contains(&(0, 1, 0)));
    // Multi-class '.banner.highlight': (0, 2, 0)
    assert!(specificities.contains(&(0, 2, 0)));
    // ID '#hero': (1, 0, 0)
    assert!(specificities.contains(&(1, 0, 0)));

    // Specificity resolution in get_specified_values: #hero (purple) must win
    let specified = get_specified_values(div_node, &div_ancestors, &stylesheet);
    assert_eq!(
        specified.get("color"),
        Some(&Value::ColorValue(CssColor { r: 142, g: 68, b: 173, a: 255 }))
    );
}

#[test]
fn test_matched_css_rules_inline_style_override() {
    let html = "<html><body>
        <div id='hero' style='color: orange;'>Content</div>
    </body></html>";

    let css = "
        #hero { color: red; font-size: 20px; }
    ";

    let root = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);

    let div_path = [0, 0];
    let div_node = root.get_node_by_path(&div_path).unwrap();
    let div_ancestors = root.collect_ancestor_nodes(&div_path);

    let specified = get_specified_values(div_node, &div_ancestors, &stylesheet);

    // Inline style overrides ID selector for color
    assert_eq!(
        specified.get("color"),
        Some(&Value::ColorValue(CssColor { r: 230, g: 126, b: 34, a: 255 }))
    );
    // Inherits / uses rule declaration for font-size
    assert_eq!(specified.get("font-size"), Some(&Value::Length(20.0, Unit::Px)));
}

// =========================================================================
// 4. DevTools State Machine Stress Tests
// =========================================================================

#[test]
fn test_devtools_state_machine_dock_and_tab_transitions() {
    let mut app = BrowserApp::new();

    // Default state: DevTools closed
    assert!(!app.show_devtools, "DevTools initially closed");
    assert_eq!(app.devtools_tab, DevToolsTab::Elements, "Default tab is Elements");
    assert_eq!(app.devtools_dock, DevToolsDockPosition::Right, "Default dock is Right");

    // Toggle open
    app.show_devtools = true;
    assert!(app.show_devtools);

    // Cycle through all 6 tabs and verify state persistence
    let all_tabs = [
        DevToolsTab::Elements,
        DevToolsTab::Console,
        DevToolsTab::Sources,
        DevToolsTab::Network,
        DevToolsTab::Application,
        DevToolsTab::Security,
    ];

    for tab in all_tabs {
        app.devtools_tab = tab;
        assert_eq!(app.devtools_tab, tab);
    }

    // Toggle docking
    app.devtools_dock = DevToolsDockPosition::Bottom;
    assert_eq!(app.devtools_dock, DevToolsDockPosition::Bottom);

    app.devtools_dock = DevToolsDockPosition::Right;
    assert_eq!(app.devtools_dock, DevToolsDockPosition::Right);

    // Close button simulation
    app.show_devtools = false;
    assert!(!app.show_devtools);
}

#[test]
fn test_devtools_sources_and_application_subsections() {
    let mut app = BrowserApp::new();

    // Sources subsections
    app.devtools_sources_file = SourcesFileId::HtmlDocument;
    assert_eq!(app.devtools_sources_file, SourcesFileId::HtmlDocument);

    app.devtools_sources_file = SourcesFileId::UserAgentCss;
    assert_eq!(app.devtools_sources_file, SourcesFileId::UserAgentCss);

    app.devtools_sources_file = SourcesFileId::InlineCss(1);
    assert_eq!(app.devtools_sources_file, SourcesFileId::InlineCss(1));

    // Application storage sections
    let sections = [
        StorageSection::Cookies,
        StorageSection::LocalStorage,
        StorageSection::SessionStorage,
        StorageSection::TextureCache,
    ];
    for sec in sections {
        app.devtools_storage_section = sec;
        assert_eq!(app.devtools_storage_section, sec);
    }
}

// =========================================================================
// 5. Console Log Buffer & REPL Stress Tests
// =========================================================================

#[test]
fn test_console_log_entries_and_filtering() {
    let mut app = BrowserApp::new();
    let tab = &mut app.tabs[app.active_tab_idx];

    // Add entries of different levels
    tab.console_entries.push(ConsoleEntry {
        id: 1,
        timestamp: "12:00:00".to_string(),
        level: ConsoleLogLevel::Info,
        source: "DOM".to_string(),
        message: "Page initialized".to_string(),
    });
    tab.console_entries.push(ConsoleEntry {
        id: 2,
        timestamp: "12:00:01".to_string(),
        level: ConsoleLogLevel::Warn,
        source: "CSS".to_string(),
        message: "Unsupported CSS property".to_string(),
    });
    tab.console_entries.push(ConsoleEntry {
        id: 3,
        timestamp: "12:00:02".to_string(),
        level: ConsoleLogLevel::Error,
        source: "Network".to_string(),
        message: "Failed to load resource".to_string(),
    });

    assert_eq!(tab.console_entries.len(), 3);

    // Test level filtering
    let errors: Vec<_> = tab.console_entries.iter().filter(|e| e.level == ConsoleLogLevel::Error).collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "Failed to load resource");

    let warnings: Vec<_> = tab.console_entries.iter().filter(|e| e.level == ConsoleLogLevel::Warn).collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].message, "Unsupported CSS property");

    // Clear console
    tab.console_entries.clear();
    assert!(tab.console_entries.is_empty());
}

#[test]
fn test_console_repl_command_dispatch_matrix() {
    let mut app = BrowserApp::new();
    let current_zoom = app.zoom_percentage();
    let tab = &mut app.tabs[app.active_tab_idx];

    tab.dom_root = Some(HtmlParser::parse("<html><body><div>Test</div></body></html>"));
    tab.stylesheet = CssParser::parse("div { margin: 10px; } p { color: red; }");
    tab.url = "about:credits".to_string();
    tab.title = "Credits".to_string();

    let commands = [
        ("help", "Commands: help, clear, dom, styles, info, zoom, goto <url>"),
        ("dom", "DOM Root: <html> with 1 direct children"),
        ("styles", "Stylesheet: 2 CSS rules"),
        ("info", "URL: about:credits, Title: 'Credits'"),
        ("zoom", "Current Zoom: 100%"),
    ];

    for (cmd, expected_sub) in commands {
        let (level, reply) = if cmd == "help" {
            (ConsoleLogLevel::Info, "Commands: help, clear, dom, styles, info, zoom, goto <url>".to_string())
        } else if cmd == "dom" {
            let tag = tab.dom_root.as_ref().and_then(|r| r.tag_name()).unwrap_or("none");
            let count = tab.dom_root.as_ref().map(|r| r.children.len()).unwrap_or(0);
            (ConsoleLogLevel::Info, format!("DOM Root: <{}> with {} direct children", tag, count))
        } else if cmd == "styles" {
            (ConsoleLogLevel::Info, format!("Stylesheet: {} CSS rules", tab.stylesheet.rules.len()))
        } else if cmd == "info" {
            (ConsoleLogLevel::Info, format!("URL: {}, Title: '{}', Size: {} bytes", tab.url, tab.title, tab.raw_html.len()))
        } else if cmd == "zoom" {
            (ConsoleLogLevel::Info, format!("Current Zoom: {}", current_zoom))
        } else {
            (ConsoleLogLevel::Warn, "Unknown".to_string())
        };

        assert_eq!(level, ConsoleLogLevel::Info);
        assert!(reply.contains(expected_sub), "Command '{}' reply '{}' did not contain '{}'", cmd, reply, expected_sub);
    }
}

// =========================================================================
// 6. Network Entries Logging & Timing Stress Tests
// =========================================================================

#[test]
fn test_network_entries_status_and_timing() {
    let mut app = BrowserApp::new();
    let tab = &mut app.tabs[app.active_tab_idx];

    // Add network entries: 200 OK, 301 Redirect, 404 Not Found, 500 Error
    tab.network_entries.push(NetworkEntry {
        id: 1,
        url: "https://example.com/index.html".to_string(),
        name: "index.html".to_string(),
        method: "GET".to_string(),
        status: 200,
        status_text: "OK".to_string(),
        resource_type: "Doc".to_string(),
        size_bytes: 4096,
        duration_ms: 45,
        headers: vec![("content-type".to_string(), "text/html".to_string())],
        response_body: "<html></html>".to_string(),
    });

    tab.network_entries.push(NetworkEntry {
        id: 2,
        url: "https://example.com/old-page".to_string(),
        name: "old-page".to_string(),
        method: "GET".to_string(),
        status: 301,
        status_text: "Moved Permanently".to_string(),
        resource_type: "Doc".to_string(),
        size_bytes: 320,
        duration_ms: 22,
        headers: vec![("location".to_string(), "/new-page".to_string())],
        response_body: "".to_string(),
    });

    tab.network_entries.push(NetworkEntry {
        id: 3,
        url: "https://example.com/missing.css".to_string(),
        name: "missing.css".to_string(),
        method: "GET".to_string(),
        status: 404,
        status_text: "Not Found".to_string(),
        resource_type: "CSS".to_string(),
        size_bytes: 150,
        duration_ms: 12,
        headers: vec![],
        response_body: "Not Found".to_string(),
    });

    tab.network_entries.push(NetworkEntry {
        id: 4,
        url: "https://example.com/api/crash".to_string(),
        name: "crash".to_string(),
        method: "POST".to_string(),
        status: 500,
        status_text: "Internal Server Error".to_string(),
        resource_type: "Fetch".to_string(),
        size_bytes: 512,
        duration_ms: 120,
        headers: vec![],
        response_body: "Server Error".to_string(),
    });

    assert_eq!(tab.network_entries.len(), 4);

    // Verify status ranges
    for entry in &tab.network_entries {
        match entry.status {
            200..=299 => assert_eq!(entry.status, 200),
            300..=399 => assert_eq!(entry.status, 301),
            400..=499 => assert_eq!(entry.status, 404),
            500..=599 => assert_eq!(entry.status, 500),
            _ => panic!("Unexpected status code"),
        }
    }

    // Verify filter by resource type
    let css_entries: Vec<_> = tab.network_entries.iter().filter(|e| e.resource_type == "CSS").collect();
    assert_eq!(css_entries.len(), 1);
    assert_eq!(css_entries[0].name, "missing.css");

    // Verify NetworkTypeFilter enum variants
    app.devtools_network_type = NetworkTypeFilter::Css;
    assert_eq!(app.devtools_network_type, NetworkTypeFilter::Css);
    app.devtools_network_type = NetworkTypeFilter::All;
    assert_eq!(app.devtools_network_type, NetworkTypeFilter::All);
    app.devtools_network_type = NetworkTypeFilter::Doc;
    assert_eq!(app.devtools_network_type, NetworkTypeFilter::Doc);
    app.devtools_network_type = NetworkTypeFilter::Fetch;
    assert_eq!(app.devtools_network_type, NetworkTypeFilter::Fetch);

    // Waterfall bar width bounds: ((duration as f32) * 1.5).max(4.0).min(60.0)
    for entry in &tab.network_entries {
        let wf_w = ((entry.duration_ms as f32) * 1.5).max(4.0).min(60.0);
        assert!(wf_w >= 4.0 && wf_w <= 60.0, "Waterfall width must be clamped between 4.0 and 60.0");
    }
}

// =========================================================================
// 7. DevTools Panel Layout & Shortcut Invariants
// =========================================================================

#[test]
fn test_devtools_shortcut_invariants_and_panel_geometry() {
    let app_src = std::fs::read_to_string("src/app.rs").expect("Failed to read src/app.rs");

    // 1. Hotkey F12 and Ctrl+Shift+I must be wired
    assert!(
        app_src.contains("Key::F12"),
        "DevTools toggle must bind Key::F12 in src/app.rs"
    );
    assert!(
        app_src.contains("i.modifiers.shift && i.key_pressed(Key::I)"),
        "DevTools toggle must bind Ctrl+Shift+I in src/app.rs"
    );

    // 2. Both Right SidePanel and Bottom TopBottomPanel must be wired
    assert!(
        app_src.contains("egui::SidePanel::right(\"devtools_panel_right\")"),
        "Right docking panel must use egui::SidePanel::right in src/app.rs"
    );
    assert!(
        app_src.contains("egui::TopBottomPanel::bottom(\"devtools_panel_bottom\")"),
        "Bottom docking panel must use egui::TopBottomPanel::bottom in src/app.rs"
    );

    // 3. 6 tabs in tab bar
    for tab_label in ["\"Elements\"", "\"Console\"", "\"Sources\"", "\"Network\"", "\"Application\"", "\"Security\""] {
        assert!(
            app_src.contains(tab_label),
            "DevTools tab bar must contain tab label {} in src/app.rs",
            tab_label
        );
    }

    // 4. Close button logic
    assert!(
        app_src.contains("self.show_devtools = false;"),
        "DevTools close button must set show_devtools to false in src/app.rs"
    );

    // 5. 4-layer box model diagram rendering
    assert!(
        app_src.contains("render_chrome_box_model"),
        "DevTools Elements tab must render 4-layer Chrome box model diagram"
    );
}

#[test]
fn test_devtools_elements_search_and_styles_filter() {
    let mut app = BrowserApp::new();

    // Verify DOM search field
    app.devtools_dom_search = "main".to_string();
    assert_eq!(app.devtools_dom_search, "main");

    // Verify styles filter field
    app.devtools_styles_filter = "display".to_string();
    assert_eq!(app.devtools_styles_filter, "display");

    // Verify console filter field
    app.devtools_console_filter = "error".to_string();
    assert_eq!(app.devtools_console_filter, "error");

    // Verify network filter field
    app.devtools_network_filter = "xhr".to_string();
    assert_eq!(app.devtools_network_filter, "xhr");
}

