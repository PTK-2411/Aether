use rust_browser::css::{parse_color, CssColor, CssParser, Unit, Value};
use rust_browser::html_parser::HtmlParser;
use rust_browser::layout::{build_layout_tree, Dimensions, EdgeSizes, Rect};
use rust_browser::network::{normalize_url, resolve_relative_url};
use rust_browser::style::style_tree;

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

