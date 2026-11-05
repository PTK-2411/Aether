use rust_browser::css::{parse_color, CssColor, CssParser, Unit, Value};
use rust_browser::html_parser::HtmlParser;
use rust_browser::layout::{build_layout_tree, Dimensions, EdgeSizes, Rect};
use rust_browser::network::{normalize_url, resolve_relative_url};
use rust_browser::style::{style_tree, Display};

#[test]
fn test_html_parser_basic() {
    let html = r#"<div id="main" class="container dark"><p>Hello <b>World</b>!</p></div>"#;
    let root = HtmlParser::parse(html);

    let div = &root.children[0];
    assert_eq!(div.tag_name(), Some("div"));
    assert_eq!(div.id(), Some("main"));
    assert!(div.classes().contains("container"));
    assert!(div.classes().contains("dark"));

    let p = &div.children[0];
    assert_eq!(p.tag_name(), Some("p"));
    assert_eq!(p.inner_text(), "Hello World!");
}

#[test]
fn test_html_entities_and_void_tags() {
    let html = r#"<p>&lt;Rust&gt; &amp; &quot;Aether&quot; &copy; 2026<br><img src="ferris.png"></p>"#;
    let root = HtmlParser::parse(html);
    let p = &root.children[0];

    let text = p.children[0].inner_text();
    assert!(text.contains("<Rust> & \"Aether\" © 2026"));

    // Check void tags have no children
    let img = &p.children[2];
    assert_eq!(img.tag_name(), Some("img"));
    assert_eq!(img.get_attribute("src"), Some("ferris.png"));
    assert_eq!(img.children.len(), 0);
}

#[test]
fn test_html_style_raw_content() {
    let html = r#"<head><style>h1 > span { color: red; }</style></head>"#;
    let root = HtmlParser::parse(html);
    let style = root.find_first_by_tag("style").unwrap();
    assert_eq!(style.inner_text(), "h1 > span { color: red; }");
}

#[test]
fn test_css_parser_rules_and_specificity() {
    let css = r#"
        body { margin: 0; background-color: #ffffff; }
        .btn { padding: 8px 16px; border-radius: 4px; }
        #hero-header { font-size: 32px; color: rgb(26, 115, 232); }
        div.card > p { color: #333333; }
    "#;
    let stylesheet = CssParser::parse(css);
    assert_eq!(stylesheet.rules.len(), 4);

    // Specificity test: #hero-header should have (1, 0, 0)
    let hero_rule = &stylesheet.rules[2];
    assert_eq!(hero_rule.selectors[0].specificity(), (1, 0, 0));

    // Specificity of div.card > p: 1 class + 2 tags = (0, 1, 2)
    let card_p_rule = &stylesheet.rules[3];
    assert_eq!(card_p_rule.selectors[0].specificity(), (0, 1, 2));
}

#[test]
fn test_css_shorthands_and_colors() {
    let css = r#"
        div {
            margin: 10px 20px;
            padding: 5px 10px 15px 20px;
        }
    "#;
    let stylesheet = CssParser::parse(css);
    let decls = &stylesheet.rules[0].declarations;

    // Margin expanded to 4 declarations
    assert!(decls.iter().any(|d| d.name == "margin-top" && d.value == Value::Length(10.0, Unit::Px)));
    assert!(decls.iter().any(|d| d.name == "margin-right" && d.value == Value::Length(20.0, Unit::Px)));

    // Padding expanded
    assert!(decls.iter().any(|d| d.name == "padding-left" && d.value == Value::Length(20.0, Unit::Px)));

    // Colors
    assert_eq!(parse_color("#fff"), Some(CssColor::new(255, 255, 255, 255)));
    assert_eq!(parse_color("#1a73e8"), Some(CssColor::new(26, 115, 232, 255)));
    assert_eq!(parse_color("rgba(10, 20, 30, 0.5)"), Some(CssColor::new(10, 20, 30, 127)));
}

#[test]
fn test_style_cascading_and_inheritance() {
    let html = r#"
        <div style="color: blue;">
            <p class="highlight" style="font-size: 20px;">Text</p>
        </div>
    "#;
    let dom = HtmlParser::parse(html);
    let css = "p { color: red; font-size: 14px; } .highlight { color: green; }";
    let stylesheet = CssParser::parse(css);

    let styled_root = style_tree(&dom, &stylesheet);
    let styled_div = &styled_root.children[0];
    let styled_p = &styled_div.children[0];

    // .highlight has higher specificity than p, so color should be green
    assert_eq!(styled_p.value("color"), Some(&Value::ColorValue(CssColor::new(40, 167, 69, 255))));
    // Inline style overrides font-size to 20px
    assert_eq!(styled_p.value("font-size"), Some(&Value::Length(20.0, Unit::Px)));
}

#[test]
fn test_layout_box_model_calculation() {
    let html = r#"<div style="width: 400px; padding: 20px; margin: 10px; background-color: #eee;"></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse("");
    let styled_root = style_tree(&dom, &stylesheet);

    let mut layout_box = build_layout_tree(&styled_root).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };

    layout_box.layout(viewport);

    let div_box = &layout_box.children[0];
    assert_eq!(div_box.dimensions.content.width, 400.0);
    assert_eq!(div_box.dimensions.padding.left, 20.0);
    assert_eq!(div_box.dimensions.padding.right, 20.0);
    assert_eq!(div_box.dimensions.margin.left, 10.0);
    assert_eq!(div_box.dimensions.border_box().width, 440.0); // 400 content + 40 padding
    assert_eq!(div_box.dimensions.margin_box().width, 800.0); // CSS 2.1 spec: fills container width via margin-right underflow
}

#[test]
fn test_url_normalization_and_resolution() {
    // Empty & about: aliases
    assert_eq!(normalize_url(""), "about:newtab");
    assert_eq!(normalize_url("   "), "about:newtab");
    assert_eq!(normalize_url("about:welcome"), "about:newtab");
    assert_eq!(normalize_url("about:home"), "about:newtab");
    assert_eq!(normalize_url("about:test"), "about:test");

    // Standard schemes preserved verbatim
    assert_eq!(normalize_url("file:///etc/hosts"), "file:///etc/hosts");
    assert_eq!(normalize_url("http://example.com"), "http://example.com");
    assert_eq!(normalize_url("https://rust-lang.org"), "https://rust-lang.org");

    // Localhost routing (http)
    assert_eq!(normalize_url("localhost"), "http://localhost");
    assert_eq!(normalize_url("localhost:8080"), "http://localhost:8080");
    assert_eq!(normalize_url("localhost/dashboard"), "http://localhost/dashboard");

    // Valid domains (https)
    assert_eq!(normalize_url("example.com"), "https://example.com");
    assert_eq!(normalize_url("news.ycombinator.com"), "https://news.ycombinator.com");
    assert_eq!(normalize_url("rust-lang.org/learn"), "https://rust-lang.org/learn");
    assert_eq!(normalize_url("crates.io:443/crates/url"), "https://crates.io:443/crates/url");
    assert_eq!(normalize_url("sub-domain.example.co.uk"), "https://sub-domain.example.co.uk");

    // IPv4 addresses (https)
    assert_eq!(normalize_url("127.0.0.1"), "https://127.0.0.1");
    assert_eq!(normalize_url("192.168.1.1:8080"), "https://192.168.1.1:8080");
    assert_eq!(normalize_url("10.0.0.1/admin"), "https://10.0.0.1/admin");

    // Google Search routing for queries, words, phrases, and special characters
    assert_eq!(normalize_url("rust"), "https://www.google.com/search?q=rust");
    assert_eq!(
        normalize_url("rust programming language"),
        "https://www.google.com/search?q=rust+programming+language"
    );
    assert_eq!(
        normalize_url("c++ & rust"),
        "https://www.google.com/search?q=c%2B%2B+%26+rust"
    );

    // Queries with dots/numbers that are NOT valid domains (routed to Google Search)
    assert_eq!(normalize_url("1.0"), "https://www.google.com/search?q=1.0");
    assert_eq!(normalize_url("1.2.3"), "https://www.google.com/search?q=1.2.3");
    assert_eq!(normalize_url("v1.2"), "https://www.google.com/search?q=v1.2");
    assert_eq!(normalize_url("python3.10"), "https://www.google.com/search?q=python3.10");
    assert_eq!(normalize_url("hello..."), "https://www.google.com/search?q=hello...");
    assert_eq!(normalize_url("..."), "https://www.google.com/search?q=...");
    assert_eq!(normalize_url(".bashrc"), "https://www.google.com/search?q=.bashrc");

    // Relative URLs resolution
    let base = "https://example.com/blog/article.html";
    assert_eq!(resolve_relative_url(base, "about:newtab"), "about:newtab");
    assert_eq!(resolve_relative_url(base, "next.html"), "https://example.com/blog/next.html");
    assert_eq!(resolve_relative_url(base, "/images/logo.png"), "https://example.com/images/logo.png");
}

#[test]
fn test_metadata_elements_hidden() {
    let html = r#"<html><head><title>Example Domain</title><style>body { background: #fff; }</style><script>console.log(1);</script></head><body><p>Visible content</p></body></html>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(rust_browser::default_style::DEFAULT_USER_AGENT_CSS);
    let styled_root = style_tree(&dom, &stylesheet);

    let layout_root = build_layout_tree(&styled_root).unwrap();

    // Verify head, title, style, script nodes are NOT in layout tree
    fn contains_tag(lb: &rust_browser::layout::LayoutBox, tag: &str) -> bool {
        if let Some(sn) = lb.get_styled_node() {
            if sn.node.tag_name() == Some(tag) {
                return true;
            }
        }
        lb.children.iter().any(|c| contains_tag(c, tag))
    }

    assert!(!contains_tag(&layout_root, "head"));
    assert!(!contains_tag(&layout_root, "title"));
    assert!(!contains_tag(&layout_root, "style"));
    assert!(!contains_tag(&layout_root, "script"));
    assert!(contains_tag(&layout_root, "p"));
}

#[test]
fn test_text_word_spacing() {
    let html = r#"<div><p>This domain is for use</p></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(rust_browser::default_style::DEFAULT_USER_AGENT_CSS);
    let styled_root = style_tree(&dom, &stylesheet);

    let mut layout_root = build_layout_tree(&styled_root).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    // Find the p box
    let p_box = &layout_root.children[0].children[0];
    assert_eq!(p_box.children.len(), 5); // 5 words: This, domain, is, for, use

    // Ensure consecutive words have positive gap (never overlapping)
    for i in 0..p_box.children.len() - 1 {
        let cur = &p_box.children[i];
        let next = &p_box.children[i + 1];
        let cur_end_x = cur.dimensions.content.x + cur.dimensions.content.width;
        let next_start_x = next.dimensions.content.x;
        assert!(
            next_start_x > cur_end_x,
            "Word {} at {} collides with word {} ending at {}",
            i + 1,
            next_start_x,
            i,
            cur_end_x
        );
    }
}

#[test]
fn test_image_layout_and_display_item() {
    let html = r#"<div><img src="https://example.com/logo.png" width="120" height="80" alt="Logo"></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(rust_browser::default_style::DEFAULT_USER_AGENT_CSS);
    let styled_root = style_tree(&dom, &stylesheet);

    let mut layout_root = build_layout_tree(&styled_root).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    let display_list = rust_browser::render::build_display_list(&layout_root);
    let image_items: Vec<_> = display_list
        .items
        .iter()
        .filter_map(|item| match item {
            rust_browser::render::DisplayItem::Image { rect, src, alt } => Some((rect, src, alt)),
            _ => None,
        })
        .collect();

    assert_eq!(image_items.len(), 1);
    let (rect, src, alt) = image_items[0];
    assert_eq!(src, "https://example.com/logo.png");
    assert_eq!(alt, "Logo");
    assert_eq!(rect.width, 120.0);
    assert_eq!(rect.height, 80.0);
}

#[test]
fn test_css_attribute_selectors_and_not() {
    let css = r#"
        [hidden] { display: none !important; }
        input[type="search"] { background-color: #f0f0f0; }
        a[href^="https://"] { color: #0000ff; }
        div:not(.hidden) { display: block; }
    "#;
    let stylesheet = CssParser::parse(css);
    assert!(!stylesheet.rules.is_empty());

    let html = r#"
        <div>
            <div id="d1" hidden="true">Hidden Item</div>
            <input id="s1" type="search" />
            <a id="l1" href="https://wikipedia.org">Wiki</a>
        </div>
    "#;
    let dom = HtmlParser::parse(html);
    let styled = style_tree(&dom, &stylesheet);

    // Find hidden item
    fn find_styled_by_id<'a>(sn: &'a rust_browser::style::StyledNode<'a>, id: &str) -> Option<&'a rust_browser::style::StyledNode<'a>> {
        if sn.node.get_attribute("id") == Some(id) {
            return Some(sn);
        }
        for c in &sn.children {
            if let Some(found) = find_styled_by_id(c, id) {
                return Some(found);
            }
        }
        None
    }

    let d1 = find_styled_by_id(&styled, "d1").unwrap();
    assert_eq!(d1.display(), rust_browser::style::Display::None);

    let s1 = find_styled_by_id(&styled, "s1").unwrap();
    assert_eq!(s1.value("background-color"), Some(&Value::ColorValue(CssColor::new(0xf0, 0xf0, 0xf0, 255))));

    let l1 = find_styled_by_id(&styled, "l1").unwrap();
    assert_eq!(l1.value("color"), Some(&Value::ColorValue(CssColor::new(0, 0, 255, 255))));
}

#[test]
fn test_css_media_queries_and_important_override() {
    let css = r#"
        p { color: #111111; }
        @media screen {
            p { color: #222222; }
        }
        @media print {
            p { color: #999999 !important; }
        }
        .special { color: #333333; }
        p.special { color: #444444; }
        .override-rule { color: #555555 !important; }
    "#;
    let stylesheet = CssParser::parse(css);
    // Print media must be ignored, screen media must be kept
    let html = r#"<div><p class="special override-rule">Text</p></div>"#;
    let dom = HtmlParser::parse(html);
    let styled = style_tree(&dom, &stylesheet);

    let p = &styled.children[0].children[0];
    // !important overrides higher specificity class rule
    assert_eq!(p.value("color"), Some(&Value::ColorValue(CssColor::new(0x55, 0x55, 0x55, 255))));
}

#[test]
fn test_nested_inline_element_positioning_not_zero() {
    let html = r#"<div><p>Start <a href="https://example.com"><span>Click Here</span></a> End</p></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(rust_browser::default_style::DEFAULT_USER_AGENT_CSS);
    let styled = style_tree(&dom, &stylesheet);

    let mut layout_root = build_layout_tree(&styled).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    let display_list = rust_browser::render::build_display_list(&layout_root);
    let text_items: Vec<_> = display_list
        .items
        .iter()
        .filter_map(|item| match item {
            rust_browser::render::DisplayItem::Text { text, pos, .. } => Some((text.as_str(), pos.x, pos.y)),
            _ => None,
        })
        .collect();

    assert!(text_items.len() >= 4, "Expected at least 4 text fragments: {:?}", text_items);

    // Verify coordinates are strictly advancing horizontally
    let mut last_x = -1.0;
    for (txt, x, y) in &text_items {
        assert!(*x >= 0.0, "Text fragment '{}' has negative x", txt);
        assert!(*y >= 0.0, "Text fragment '{}' has negative y", txt);
        assert!(*x > last_x, "Text fragment '{}' at x={} overlaps or is before previous at x={}", txt, x, last_x);
        last_x = *x;
    }
}

#[test]
fn test_flexbox_row_layout() {
    let css = r#"
        .flex-container { display: flex; width: 600px; }
        .col-1 { width: 200px; height: 50px; }
        .col-2 { width: 300px; height: 80px; }
    "#;
    let html = r#"<div class="flex-container"><div class="col-1">Item1</div><div class="col-2">Item2</div></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let mut layout_root = build_layout_tree(&styled).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    let flex_container = &layout_root.children[0];
    assert_eq!(flex_container.children.len(), 2);
    let child1 = &flex_container.children[0];
    let child2 = &flex_container.children[1];

    assert_eq!(child1.dimensions.content.x, 0.0);
    assert_eq!(child1.dimensions.content.width, 200.0);
    assert!(child2.dimensions.content.x >= 200.0, "Flex child 2 must be positioned after flex child 1: x={}", child2.dimensions.content.x);
    assert_eq!(child2.dimensions.content.width, 300.0);
}

#[test]
fn test_protocol_relative_url_and_box_sizing() {
    let base = "https://en.wikipedia.org/wiki/Rust";
    assert_eq!(
        resolve_relative_url(base, "//upload.wikimedia.org/wikipedia/commons/logo.png"),
        "https://upload.wikimedia.org/wikipedia/commons/logo.png"
    );

    let css = r#"
        .box { box-sizing: border-box; width: 200px; padding-left: 20px; padding-right: 20px; border-left-width: 5px; border-right-width: 5px; }
    "#;
    let html = r#"<div class="box">Content</div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let mut layout_root = build_layout_tree(&styled).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    let box_elem = &layout_root.children[0];
    // Total border box width must equal specified 200px
    assert_eq!(box_elem.dimensions.border_box().width, 200.0);
    // Content width must be 200 - 40(padding) - 10(border) = 150px
    assert_eq!(box_elem.dimensions.content.width, 150.0);
}

#[test]
fn test_child_and_sibling_combinators() {
    let css = r#"
        div > p { color: #111111; }
        h2 + p { color: #222222; }
        h2 ~ span { color: #333333; }
    "#;
    let html = r#"
        <div>
            <p id="direct-child">Direct</p>
            <section><p id="nested-child">Nested</p></section>
            <h2>Heading</h2>
            <p id="adjacent-p">Adjacent</p>
            <p id="not-adjacent-p">Second</p>
            <span id="sibling-span">Span</span>
        </div>
    "#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let find_color = |id: &str| -> Option<CssColor> {
        fn search(node: &rust_browser::style::StyledNode, id: &str) -> Option<CssColor> {
            if node.node.id() == Some(id) {
                return Some(node.color(None));
            }
            for c in &node.children {
                if let Some(col) = search(c, id) {
                    return Some(col);
                }
            }
            None
        }
        search(&styled, id)
    };

    assert_eq!(find_color("direct-child"), Some(CssColor::new(17, 17, 17, 255)));
    assert_ne!(find_color("nested-child"), Some(CssColor::new(17, 17, 17, 255)));
    assert_eq!(find_color("adjacent-p"), Some(CssColor::new(34, 34, 34, 255)));
    assert_ne!(find_color("not-adjacent-p"), Some(CssColor::new(34, 34, 34, 255)));
    assert_eq!(find_color("sibling-span"), Some(CssColor::new(51, 51, 51, 255)));
}

#[test]
fn test_pseudo_elements_do_not_match_elements() {
    let css = r#"
        .heading::after { width: 10px; display: inline-block; }
        .heading { width: 300px; display: block; }
    "#;
    let html = r#"<div class="heading">Heading Title</div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let heading = &styled.children[0];
    assert_eq!(heading.display(), rust_browser::style::Display::Block);
    // Heading element must NOT match the ::after pseudo-element rule!
    let w = heading.value("width");
    assert_eq!(w, Some(&Value::Length(300.0, Unit::Px)));
}

#[test]
fn test_pseudo_classes_and_root() {
    let css = r#"
        :root { background-color: #123456; }
        p:first-child { color: #ff0000; }
        p:last-child { color: #0000ff; }
    "#;
    let html = r#"<html><body><div><p id="p1">First</p><p id="p2">Middle</p><p id="p3">Last</p></div></body></html>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    // Root (html) should have the background color
    assert_eq!(styled.background_color(), CssColor::new(18, 52, 86, 255));

    // Internal div should NOT match :root
    let body = &styled.children[0];
    let div = &body.children[0];
    assert_ne!(div.background_color(), CssColor::new(18, 52, 86, 255));

    let p1 = &div.children[0];
    let p2 = &div.children[1];
    let p3 = &div.children[2];

    assert_eq!(p1.color(None), CssColor::new(255, 0, 0, 255));
    assert_ne!(p2.color(None), CssColor::new(255, 0, 0, 255));
    assert_eq!(p3.color(None), CssColor::new(0, 0, 255, 255));
}

#[test]
fn test_css_variables_and_inheritance() {
    let css = r#"
        :root {
            --brand-color: #e63946;
            --brand-font-size: 24px;
        }
        .container {
            color: var(--brand-color);
            font-size: var(--brand-font-size);
            background-color: var(--unspecified-bg, #f1faee);
        }
        .inner-text {
            color: var(--brand-color);
        }
    "#;
    let html = r#"<html><body><div class="container"><p class="inner-text">Hello</p></div></body></html>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let body = &styled.children[0];
    let container = &body.children[0];
    let inner = &container.children[0];

    // Variables resolved on container
    assert_eq!(container.color(None), CssColor::new(230, 57, 70, 255));
    assert_eq!(container.font_size(None), 24.0);
    assert_eq!(container.background_color(), CssColor::new(241, 250, 238, 255));

    // Variable inherited and resolved on inner child
    assert_eq!(inner.color(None), CssColor::new(230, 57, 70, 255));
}

#[test]
fn test_css_grid_template_columns_layout() {
    let css = r#"
        .grid-container {
            display: grid;
            grid-template-columns: 200px 1fr;
            width: 800px;
        }
        .sidebar { height: 100px; }
        .main-content { height: 200px; }
    "#;
    let html = r#"<div class="grid-container"><div class="sidebar">Side</div><div class="main-content">Main</div></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let mut layout_root = build_layout_tree(&styled).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 600.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    let grid = &layout_root.children[0];
    let sidebar = &grid.children[0];
    let main = &grid.children[1];

    assert_eq!(sidebar.dimensions.content.x, 0.0);
    assert_eq!(sidebar.dimensions.content.width, 200.0);
    assert_eq!(main.dimensions.content.x, 200.0);
    assert_eq!(main.dimensions.content.width, 600.0);
}

#[test]
fn test_absolute_positioning_and_out_of_flow() {
    let css = r#"
        .container { width: 500px; }
        .jump-link { position: absolute; top: -9999px; left: 0px; height: 30px; }
        .header { height: 60px; }
    "#;
    let html = r#"<div class="container"><div class="jump-link">Jump to content</div><div class="header">Header</div></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let mut layout_root = build_layout_tree(&styled).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 0.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    let container = &layout_root.children[0];
    let jump_link = &container.children[0];
    let header = &container.children[1];

    // Jump link positioned offscreen
    assert_eq!(jump_link.dimensions.content.y, -9999.0);
    // Header must NOT be pushed down by the absolute jump link; starts at top (0.0)
    assert_eq!(header.dimensions.content.y, 0.0);
}

#[test]
fn test_hsl_and_hsla_colors() {
    let css = r#"
        .red { color: hsl(0, 100%, 50%); }
        .green { color: hsl(120, 100%, 50%); }
        .blue { color: hsl(240, 100%, 50%); }
        .alpha { background-color: hsla(0, 100%, 50%, 0.5); }
    "#;
    let stylesheet = CssParser::parse(css);
    let html = r#"<div><div class="red"></div><div class="green"></div><div class="blue"></div><div class="alpha"></div></div>"#;
    let dom = HtmlParser::parse(html);
    let styled = style_tree(&dom, &stylesheet);

    let red = &styled.children[0].children[0];
    let green = &styled.children[0].children[1];
    let blue = &styled.children[0].children[2];
    let alpha = &styled.children[0].children[3];

    assert_eq!(red.color(None), CssColor::new(255, 0, 0, 255));
    assert_eq!(green.color(None), CssColor::new(0, 255, 0, 255));
    assert_eq!(blue.color(None), CssColor::new(0, 0, 255, 255));
    assert_eq!(alpha.background_color(), CssColor::new(255, 0, 0, 127));
}

#[test]
fn test_calc_width_and_max_width() {
    let css = r#"
        .box-sub { width: calc(100% - 200px); }
        .box-add { width: calc(200px + 50px); }
    "#;
    let html = r#"<div><div class="box-sub">Sub</div><div class="box-add">Add</div></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let mut layout_root = build_layout_tree(&styled).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 0.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    let parent = &layout_root.children[0];
    let box_sub = &parent.children[0];
    let box_add = &parent.children[1];

    assert_eq!(box_sub.dimensions.content.width, 600.0);
    assert_eq!(box_add.dimensions.content.width, 250.0);
}

#[test]
fn test_absolute_positioning_translates_children() {
    let css = r#"
        .card { position: absolute; top: 120px; left: 80px; width: 300px; height: 100px; }
    "#;
    let html = r#"<div class="card"><p>Card Content</p></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let mut layout_root = build_layout_tree(&styled).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 1024.0, 0.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    let card = &layout_root.children[0];
    let p = &card.children[0];
    let text_fragment = &p.children[0];

    // Card positioned at (80, 120)
    assert_eq!(card.dimensions.content.x, 80.0);
    assert_eq!(card.dimensions.content.y, 120.0);

    // Child paragraph translated with parent card to (80, 120)
    assert_eq!(p.dimensions.content.x, 80.0);
    assert_eq!(p.dimensions.content.y, 120.0);

    // Child text fragment also translated with parent card
    assert_eq!(text_fragment.dimensions.content.x, 80.0);
    assert_eq!(text_fragment.dimensions.content.y, 120.0);
}

#[test]
fn test_inline_element_with_absolute_positioning() {
    let css = r#"
        a.jump-link { position: absolute; top: -9999px; left: 0px; }
    "#;
    let html = r##"<div><a class="jump-link" href="#main">Jump to content</a></div>"##;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let a_tag = &styled.children[0].children[0];
    // Inline element with position: absolute computes to Block per CSS spec
    assert_eq!(a_tag.display(), Display::Block);

    let mut layout_root = build_layout_tree(&styled).unwrap();
    let viewport = Dimensions {
        content: Rect::new(0.0, 0.0, 800.0, 0.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    };
    layout_root.layout(viewport);

    let parent = &layout_root.children[0];
    let jump_box = &parent.children[0];
    assert_eq!(jump_box.dimensions.content.y, -9999.0);

    // All text children must also be translated offscreen
    let text = &jump_box.children[0];
    assert_eq!(text.dimensions.content.y, -9999.0);
}

#[test]
fn test_font_size_percent_and_keywords() {
    let css = r#"
        .parent { font-size: 20px; }
        .scaled { font-size: 150%; }
        .small-kw { font-size: small; }
    "#;
    let html = r#"<div class="parent"><p class="scaled">Scaled</p><span class="small-kw">Small</span></div>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let parent = &styled.children[0];
    let scaled = &parent.children[0];
    let small_kw = &parent.children[1];

    assert_eq!(parent.font_size(None), 20.0);
    assert_eq!(scaled.font_size(Some(parent)), 30.0);
    assert_eq!(small_kw.font_size(Some(parent)), 13.0);
}

#[test]
fn test_background_and_border_with_css_variables() {
    let css = r#"
        :root {
            --bg-base: #f8f9fa;
            --border-clr: #a2a9b1;
        }
        .container {
            background: var(--bg-base) url(/some/icon.svg) no-repeat;
            border: 1px solid var(--border-clr);
        }
        .header {
            background: #e9ecef none repeat;
        }
    "#;
    let html = r#"<html><body><div class="container"><div class="header">Head</div></div></body></html>"#;
    let dom = HtmlParser::parse(html);
    let stylesheet = CssParser::parse(css);
    let styled = style_tree(&dom, &stylesheet);

    let body = &styled.children[0];
    let container = &body.children[0];
    let header = &container.children[0];

    // CSS variable extracted from multivalent background shorthand
    assert_eq!(container.background_color(), CssColor::new(248, 249, 250, 255));
    // Color extracted from shorthand with none repeat
    assert_eq!(header.background_color(), CssColor::new(233, 236, 239, 255));
    // Border color extracted from shorthand with CSS variable
    assert_eq!(container.value("border-top-color"), Some(&Value::ColorValue(CssColor::new(162, 169, 177, 255))));
}


