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
    let html = r#"<p>&lt;Rust&gt; &amp; &quot;IronSurf&quot; &copy; 2026<br><img src="ferris.png"></p>"#;
    let root = HtmlParser::parse(html);
    let p = &root.children[0];

    let text = p.children[0].inner_text();
    assert!(text.contains("<Rust> & \"IronSurf\" © 2026"));

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
    assert_eq!(normalize_url(""), "about:welcome");
    assert_eq!(normalize_url("about:test"), "about:test");
    assert_eq!(normalize_url("example.com"), "https://example.com");
    assert_eq!(normalize_url("https://rust-lang.org"), "https://rust-lang.org");

    // Relative URLs
    let base = "https://example.com/blog/article.html";
    assert_eq!(resolve_relative_url(base, "about:welcome"), "about:welcome");
    assert_eq!(resolve_relative_url(base, "next.html"), "https://example.com/blog/next.html");
    assert_eq!(resolve_relative_url(base, "/images/logo.png"), "https://example.com/images/logo.png");
}
