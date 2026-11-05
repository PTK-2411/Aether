use rust_browser::app::{BrowserApp, DevToolsDockPosition, DevToolsTab};
use rust_browser::css::CssParser;
use rust_browser::html_parser::HtmlParser;
use rust_browser::layout::query_node_dimensions;
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
