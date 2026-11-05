use eframe::egui::Color32;
use rust_browser::css::CssParser;
use rust_browser::default_style::DEFAULT_USER_AGENT_CSS;
use rust_browser::dom::Node;
use rust_browser::html_parser::HtmlParser;
use rust_browser::layout::{
    build_layout_tree, BoxType, Dimensions, EdgeSizes, LayoutBox, Rect as LayoutRect,
};
use rust_browser::network::{decode_body, visible_text_len};
use rust_browser::render::{build_display_list, DisplayItem};
use rust_browser::style::{style_tree, StyledNode};

/// Builds a real layout tree for a fragment. The DOM and styled tree are leaked
/// because the layout tree borrows them; test-only, a handful of pages at most.
fn layout_html(html: &str, css: &str, width: f32) -> LayoutBox<'static> {
    let dom: &'static Node = Box::leak(Box::new(HtmlParser::parse(html)));
    let stylesheet = CssParser::parse(&format!("{}{}", DEFAULT_USER_AGENT_CSS, css));
    let styled: &'static StyledNode<'static> = Box::leak(Box::new(style_tree(dom, &stylesheet)));
    let mut root = build_layout_tree(styled).unwrap();
    root.layout(Dimensions {
        content: LayoutRect::new(0.0, 0.0, width, 0.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    });
    root
}

fn painted_colors(root: &LayoutBox) -> Vec<Color32> {
    build_display_list(root)
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::SolidColor { color, .. } => Some(*color),
            _ => None,
        })
        .collect()
}

fn text_runs(root: &LayoutBox) -> Vec<(String, f32, f32)> {
    build_display_list(root)
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Text { text, pos, .. } if !text.trim().is_empty() => {
                Some((text.trim().to_string(), pos.x, pos.y))
            }
            _ => None,
        })
        .collect()
}

/// First box in the tree whose computed `display` is `kind`.
fn find_display<'a, 'b>(root: &'b LayoutBox<'a>, kind: &str) -> Option<&'b LayoutBox<'a>> {
    let is_match = matches!(
        root.get_styled_node().and_then(|s| s.value("display")),
        Some(rust_browser::css::Value::Keyword(k)) if k == kind
    );
    if is_match {
        return Some(root);
    }
    root.children.iter().find_map(|c| find_display(c, kind))
}

/// The element children of a container, skipping the anonymous text runs that
/// sit between tags in the source.
fn element_boxes<'a, 'b>(row: &'b LayoutBox<'a>) -> Vec<&'b LayoutBox<'a>> {
    row.children
        .iter()
        .filter(|c| matches!(c.box_type, BoxType::BlockNode(_)))
        .collect()
}

// ---------------------------------------------------------------- visible text

#[test]
fn script_and_style_payloads_are_not_visible_text() {
    let html = "<html><body><script>var x='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';</script><style>.a{color:red}</style><p>Short note</p></body></html>";
    // "Short note" has 9 non-whitespace characters; whitespace is not visible text.
    assert_eq!(visible_text_len(html), 9);
}

#[test]
fn comments_are_not_visible_text() {
    let html = "<html><body><!-- aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa --><p>real</p></body></html>";
    assert_eq!(visible_text_len(html), 4);
}

#[test]
fn js_only_page_reads_as_almost_empty() {
    // Models the YouTube case: a huge script-bearing document with a footer.
    let mut html = String::from("<html><body><div id=\"app\"></div><script>");
    html.push_str(&"document.write('.cls{margin:0}');".repeat(400));
    html.push_str("</script><footer>About Help Press Copyright</footer></body></html>");
    assert!(html.len() > 10_000, "fixture should be script-heavy");
    assert!(
        visible_text_len(&html) < 250,
        "visible_text_len = {}",
        visible_text_len(&html)
    );
}

#[test]
fn static_page_keeps_its_real_text_length() {
    let html = "<html><body><h1>Chapter One</h1><p>".to_string()
        + &"word ".repeat(120)
        + "</p></body></html>";
    assert!(visible_text_len(&html) > 250);
}

// --------------------------------------------------------------------- charset

#[test]
fn lone_quote_in_a_css_value_is_not_a_panic() {
    // Real stylesheets ship truncated `content:"` values; stripping the quotes
    // from a one-character string used to slice 1..0 and abort the process.
    for raw in ["\"", "'", "\"\"", "''", " "] {
        let _ = rust_browser::css::parse_value_str(raw);
    }
}

#[test]
fn header_charset_decodes_windows_1252() {
    // 0xE9 = é in both latin-1 and cp1252; 0x93 = “ only in cp1252.
    let bytes = [b'c', b'a', b'f', 0xE9, b' ', 0x93, b'x', 0x94];
    let decoded = decode_body(&bytes, Some("text/html; charset=windows-1252"));
    assert_eq!(decoded, "café “x”");
}

#[test]
fn latin1_charset_is_respected() {
    let bytes = [0xE8, 0xF4, 0xEA];
    assert_eq!(decode_body(&bytes, Some("text/html;charset=iso-8859-1")), "èôê");
}

#[test]
fn meta_charset_is_sniffed_when_header_omits_it() {
    let mut bytes = b"<html><head><meta http-equiv=\"Content-Type\" content=\"text/html; charset=windows-1252\"></head><body>caf".to_vec();
    bytes.extend_from_slice(&[0xE9]);
    bytes.extend_from_slice(b"</body></html>");
    let decoded = decode_body(&bytes, None);
    assert!(decoded.contains("café"), "decoded: {}", decoded);
}

#[test]
fn valid_utf8_passes_through_unchanged() {
    let text = "Xin chào — 日本語";
    assert_eq!(decode_body(text.as_bytes(), Some("text/html")), text);
}

#[test]
fn unknown_charset_falls_back_to_lossy_utf8() {
    let bytes = [0xF0, 0x9F, 0x98, 0x80];
    assert_eq!(decode_body(&bytes, Some("text/html; charset=x-unknown")), "😀");
}

// ---------------------------------------------------------------- flex sizing

#[test]
fn flex_basis_zero_items_share_the_row_instead_of_overflowing() {
    let html = r#"<html><body><div style="display: flex; width: 800px;">
        <div style="flex: 1 1 0;">Alpha</div>
        <div style="flex: 1 1 0;">Bravo</div>
        <div style="flex: 1 1 0;">Charlie</div>
        <div style="flex: 1 1 0;">Delta</div>
    </div></body></html>"#;
    let root = layout_html(html, "", 1000.0);

    let flex_row = find_display(&root, "flex").expect("flex container laid out");
    // The whitespace between the tags is not an anonymous flex item.
    let items: Vec<&LayoutBox> = element_boxes(flex_row);
    assert_eq!(items.len(), 4);
    for item in &items {
        assert!(
            (item.dimensions.content.width - 200.0).abs() < 1.0,
            "each flex:1 item should be a quarter of 800px, got {}",
            item.dimensions.content.width
        );
    }
    // All on one line: same y, and the row must not be taller than one item.
    let first_y = items[0].dimensions.content.y;
    for item in &items[1..] {
        assert_eq!(item.dimensions.content.y, first_y);
    }
    assert!(flex_row.dimensions.content.height < 60.0);
}

#[test]
fn auto_basis_flex_items_shrink_to_fit_the_container() {
    let html = r#"<html><body><div style="display: flex; width: 300px;">
        <div style="flex: 1;">wide label that is longer than its share</div>
        <div style="flex: 1;">other</div>
    </div></body></html>"#;
    let root = layout_html(html, "", 1000.0);
    let row = find_display(&root, "flex").expect("flex container laid out");
    let total: f32 = element_boxes(row)
        .iter()
        .map(|c| c.dimensions.content.width)
        .sum();
    assert!(
        total <= 300.5,
        "items should not exceed the 300px container, summed to {}",
        total
    );
}

// --------------------------------------------------------------- z-index order

#[test]
fn positive_z_index_paints_above_later_dom_siblings() {
    let html = r#"<html><body>
        <div style="position: absolute; z-index: 5; background-color: rgb(255,0,0); width: 50px; height: 50px;"></div>
        <div style="background-color: rgb(0,255,0); width: 50px; height: 50px;"></div>
        <div style="position: absolute; z-index: 20; background-color: rgb(0,0,255); width: 50px; height: 50px;"></div>
    </body></html>"#;
    let root = layout_html(html, "", 1000.0);
    let colors = painted_colors(&root);
    let red = Color32::from_rgb(255, 0, 0);
    let green = Color32::from_rgb(0, 255, 0);
    let blue = Color32::from_rgb(0, 0, 255);

    let pos_of = |c: Color32| colors.iter().position(|x| *x == c).expect("color painted");
    assert!(pos_of(green) < pos_of(red), "positioned box should paint above flow");
    assert!(pos_of(red) < pos_of(blue), "z-index 20 must paint after z-index 5");
}

#[test]
fn negative_z_index_paints_below_normal_flow() {
    let html = r#"<html><body>
        <div style="position: absolute; z-index: -1; background-color: rgb(255,0,0); width: 50px; height: 50px;"></div>
        <div style="background-color: rgb(0,255,0); width: 50px; height: 50px;"></div>
    </body></html>"#;
    let root = layout_html(html, "", 1000.0);
    let colors = painted_colors(&root);
    let red_at = colors
        .iter()
        .position(|c| *c == Color32::from_rgb(255, 0, 0))
        .unwrap();
    let green_at = colors
        .iter()
        .position(|c| *c == Color32::from_rgb(0, 255, 0))
        .unwrap();
    assert!(red_at < green_at);
}

// ----------------------------------------------------------------- positioning

#[test]
fn absolutely_positioned_box_anchored_by_right_edge_shrinks_to_fit() {
    let html = r#"<html><body style="position: relative;">
        <div style="position: absolute; right: 20px; background-color: rgb(1,2,3);">short</div>
    </body></html>"#;
    let root = layout_html(html, "", 500.0);
    let body = &root.children[0];
    let is_absolute = |b: &&LayoutBox| -> bool {
        matches!(
            b.get_styled_node().and_then(|s| s.value("position")),
            Some(rust_browser::css::Value::Keyword(k)) if k == "absolute"
        )
    };
    let abs = body.children.iter().find(is_absolute).expect("absolute box laid out");
    let width = abs.dimensions.content.width;
    // The containing block is body's content box, not the 500px viewport.
    let cb_right = body.dimensions.content.x + body.dimensions.content.width;
    assert!(
        width > 10.0 && width < 200.0,
        "shrunk to {}px",
        width
    );
    assert!(
        (abs.dimensions.content.x + width - (cb_right - 20.0)).abs() < 1.5,
        "right edge should sit 20px inside the containing block, x={width} cb_right={cb_right}"
    );
}

#[test]
fn fixed_position_uses_the_viewport_not_the_parent() {
    rust_browser::layout::set_viewport(1200.0, 700.0);
    let html = r#"<html><body>
        <div style="position: fixed; top: 0; right: 0; width: 100px; height: 40px; background-color: rgb(9,9,9);"></div>
    </body></html>"#;
    let root = layout_html(html, "", 500.0);
    let is_fixed = |b: &&LayoutBox| -> bool {
        matches!(
            b.get_styled_node().and_then(|s| s.value("position")),
            Some(rust_browser::css::Value::Keyword(k)) if k == "fixed"
        )
    };
    let fixed = root
        .children
        .iter()
        .flat_map(|b| b.children.iter())
        .find(is_fixed)
        .expect("fixed box laid out");
    assert!(
        (fixed.dimensions.content.x - (1200.0 - 100.0)).abs() < 2.0,
        "fixed right:0 should hug the 1200px viewport, got x={}",
        fixed.dimensions.content.x
    );
}

// ----------------------------------------------------------- inline text flow

#[test]
fn inline_text_is_laid_out_in_a_flow_not_piled_at_the_origin() {
    let html = r##"<html><body>
        <p><span>alpha</span> <span>beta</span> <span>gamma</span></p>
        <p><a href="#">link one</a> <a href="#">link two</a></p>
    </body></html>"##;
    let root = layout_html(html, "", 600.0);
    let runs = text_runs(&root);

    assert!(
        runs.len() >= 5,
        "expected the five text runs, got {:?}",
        runs
    );
    for (_, x, y) in &runs {
        assert!(
            !(*x == 0.0 && *y == 0.0),
            "no text may sit at the page origin: {:?}",
            runs
        );
    }

    let x_of = |needle: &str| -> f32 {
        runs.iter()
            .find(|(text, _, _)| text.contains(needle))
            .unwrap_or_else(|| panic!("missing {:?}", needle))
            .1
    };
    let y_of = |needle: &str| -> f32 {
        runs.iter()
            .find(|(text, _, _)| text.contains(needle))
            .unwrap_or_else(|| panic!("missing {:?}", needle))
            .2
    };

    // The first paragraph lays out left-to-right on a single line.
    assert!(x_of("alpha") < x_of("beta"), "{:?}", runs);
    assert!(x_of("beta") < x_of("gamma"), "{:?}", runs);
    assert_eq!(y_of("alpha"), y_of("gamma"), "same line needs same y");

    // The link paragraph follows below it, not on top of it. Text is laid out
    // per word, so the second link is the runs "link" then "two" after "one".
    assert!(y_of("two") > y_of("alpha"), "{:?}", runs);
    assert!(x_of("two") > x_of("one"), "{:?}", runs);
}

// ------------------------------------------------------------------ scroll area

#[test]
fn fixed_subtree_paints_in_the_pinned_layer() {
    let html = r#"<html><body>
        <div style="position: fixed; top: 0; left: 0; width: 200px; height: 50px; background-color: rgb(3,3,3);">
            <a href="/x">pinned link</a>
        </div>
        <div style="background-color: rgb(4,4,4); height: 300px;"></div>
    </body></html>"#;
    let root = layout_html(html, "", 800.0);
    let list = build_display_list(&root);

    let pinned_bars = list
        .fixed
        .iter()
        .filter(|i| matches!(i, DisplayItem::SolidColor { color, .. } if *color == Color32::from_rgb(3, 3, 3)))
        .count();
    assert_eq!(pinned_bars, 1, "the fixed bar belongs to the viewport layer");

    let flowing = list
        .items
        .iter()
        .filter(|i| matches!(i, DisplayItem::SolidColor { color, .. } if *color == Color32::from_rgb(4, 4, 4)))
        .count();
    assert_eq!(flowing, 1, "normal flow stays in the document layer");

    assert!(
        list.links.iter().all(|l| l.fixed),
        "a link inside the fixed bar must be hit-tested against the viewport origin"
    );
}

#[test]
fn content_height_reaches_boxes_outside_the_flow() {
    let html = r#"<html><body>
        <p>first line</p>
        <div style="position: absolute; top: 900px; height: 40px;"></div>
    </body></html>"#;
    let root = layout_html(html, "", 800.0);
    let root_height = root.dimensions.margin_box().height;
    let needed = rust_browser::layout::content_height(&root);
    assert!(
        needed > root_height + 800.0,
        "the abs box at 900px must extend the scroll area: root={root_height} needed={needed}"
    );
}

/// First box in the tree belonging to an element with this tag name.
fn find_tag<'a, 'b>(root: &'b LayoutBox<'a>, tag: &str) -> Option<&'b LayoutBox<'a>> {
    if root.get_styled_node().and_then(|s| s.node.tag_name()) == Some(tag) {
        return Some(root);
    }
    root.children.iter().find_map(|c| find_tag(c, tag))
}

#[test]
fn auto_side_margins_center_a_width_capped_block() {
    // The example.com idiom: no width, max-width plus margin:auto. The used
    // width has to be capped before the leftover space is handed to the
    // margins, otherwise the box hugs the left edge instead of centering.
    let html = "<body><div><span>text</span></div></body>";
    let css = "div { max-width: 400px; margin: auto; padding: 0 20px; }";
    let root = layout_html(html, css, 1200.0);
    let div = find_tag(&root, "div").expect("div should generate a box");
    let line = find_tag(&root, "body")
        .expect("body should generate a box")
        .dimensions
        .content
        .width;

    assert!(
        (div.dimensions.content.width - 400.0).abs() < 1.0,
        "content width must stop at max-width, got {}",
        div.dimensions.content.width
    );
    assert!(
        (div.dimensions.margin.left - div.dimensions.margin.right).abs() < 0.5,
        "auto margins must be equal, got {} vs {}",
        div.dimensions.margin.left,
        div.dimensions.margin.right
    );
    assert!(
        (div.dimensions.margin.left * 2.0 + 400.0 + 40.0 - line).abs() < 1.0,
        "the two auto margins must consume the leftover line width ({line}), got left={}",
        div.dimensions.margin.left
    );
    assert!(
        div.dimensions.margin.left > 100.0,
        "a 400px box on a {line}px line must be pushed well in from the edge"
    );
}

#[test]
fn hidden_inputs_get_no_box_and_no_painted_field() {
    let html = r#"<body><form>
        <input type="hidden" name="secret" value="hunter2">
        <input type="text" name="q" value="visible">
    </form></body>"#;
    let root = layout_html(html, "", 800.0);
    let mut field_boxes = 0usize;
    fn count_inputs(b: &LayoutBox, n: &mut usize) {
        if b.get_styled_node().and_then(|s| s.node.tag_name()) == Some("input") {
            *n += 1;
        }
        for c in &b.children {
            count_inputs(c, n);
        }
    }
    count_inputs(&root, &mut field_boxes);
    assert_eq!(
        field_boxes, 1,
        "type=hidden must not generate a layout box; only the text field may have one"
    );

    let fields: Vec<String> = build_display_list(&root)
        .items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::InputBox { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        fields,
        vec!["visible".to_string()],
        "only the real text field may be painted; hidden values must stay out of the page"
    );
}

#[test]
fn search_results_come_from_a_live_engine_and_the_banner_escapes_the_query() {
    use rust_browser::network::{inject_after_opening_body, organic_result_links, search_source_notice};

    let shell = "<html><body><script>google.shim()</script></body></html>";
    assert_eq!(
        organic_result_links(shell),
        0,
        "a JavaScript shell has no results and must not be treated as a results page"
    );

    let live = r#"<html><body>
        <a href="https://en.wikipedia.org/wiki/Rust">Rust</a>
        <a href="https://www.rust-lang.org/">Rust</a>
        <a href="https://doc.rust-lang.org/book/">The Book</a>
        <a href="https://accounts.google.com/">Sign in</a>
    </body></html>"#;
    assert_eq!(
        organic_result_links(live),
        3,
        "links back to the engine itself must not count as results"
    );

    let banner = search_source_notice("<b>x</b>", "Bing", "https://www.bing.com/search?q=y");
    let page = inject_after_opening_body("<html><body>results</body></html>", &banner);
    assert!(
        page.find("<body").unwrap() < page.find("Bing").unwrap(),
        "the banner belongs at the top of the page, not inside the results"
    );
    assert!(
        page.contains("&lt;b&gt;x&lt;/b&gt;"),
        "the query must be escaped before it is dropped into the page: {page}"
    );
}

#[test]
fn raw_text_markup_never_reaches_the_paint() {
    // Every shape below carries CSS or JS source with braces. Real pages do
    // this (Google inlines a few hundred of them), and painting any of it is
    // the "stylesheet shows as text" defect.
    let cases = [
        "<html><body><style>.minidiv .plR5qb{height:24px}.hp{color:red}</style><p>hello</p></body></html>",
        "<html><body><STYLE>a{color:blue}</STYLE ><p>hi</p></body></html>",
        "<html><body><div><script>var s=\"a{b}\"; function f(){ return 1; }</script><p>ok</p></div></body></html>",
        "<html><body><noscript><style>b{x:y}</style>noscript fallback</noscript><p>go</p></body></html>",
        "<html><body><template><div>{not rendered}</div></template><p>tp</p></body></html>",
        "<html><body><h1>Title</h1><style>body{margin:0}.unclosed{color",
        "<html><body><img src=logo.png alt=Pic><style>.u{margin:0}</style><p>after</p></body></html>",
    ];
    for html in cases {
        let root = layout_html(html, "", 800.0);
        let runs = text_runs(&root);
        let leaks: Vec<&String> = runs
            .iter()
            .map(|(text, _, _)| text)
            .filter(|text| text.contains('{') || text.contains('}'))
            .collect();
        assert!(
            leaks.is_empty(),
            "raw-text content was painted for {html:?}, got {leaks:?}"
        );
        assert!(!runs.is_empty(), "page {html:?} lost all of its text");
    }
}

#[test]
fn a_page_revealed_only_by_an_animation_is_not_painted_blank() {
    // Brave's live results page ships body{opacity:0;animation-name:reveal;
    // animation-delay:3s;animation-fill-mode:forwards}. A renderer that paints
    // one static frame has no animation clock, so taking the pre-animation
    // opacity leaves 100% of the page invisible. The frame we can actually show
    // is the one the animation ends in.
    let revealed = layout_html(
        "<html><body><div id=card>Outer Wilds Wiki - results</div></body></html>",
        "body{opacity:0;animation-name:reveal;animation-duration:0s;animation-delay:3s;animation-fill-mode:forwards}",
        1280.0,
    );
    let runs = text_runs(&revealed);
    assert!(
        runs.iter().any(|(t, _, _)| t.contains("results")),
        "a reveal-animated body was dropped and the page painted blank: {runs:?}"
    );

    let shorthand = layout_html(
        "<html><body><p class=fade>primes in rust</p></body></html>",
        ".fade{opacity:0;animation:fade 1s .4s forwards}",
        800.0,
    );
    assert!(
        text_runs(&shorthand).iter().any(|(t, _, _)| t.contains("primes")),
        "animation shorthand must reveal the text too"
    );

    let hidden = layout_html(
        "<html><body><div class=modal>secret panel</div></body></html>",
        ".modal{opacity:0}",
        800.0,
    );
    assert!(
        text_runs(&hidden).is_empty(),
        "opacity:0 with no animation is a real hide and must stay invisible"
    );

    let collapsed = layout_html(
        "<html><body><div class=menu>dropdown only on hover</div></body></html>",
        ".menu{visibility:hidden}",
        800.0,
    );
    assert!(
        text_runs(&collapsed).is_empty(),
        "visibility:hidden with no animation must stay invisible"
    );

    // The reveal exemption is opacity-only: a hover menu animated in with
    // visibility must not be painted permanently open.
    let hover_menu = layout_html(
        "<html><body><nav class=drop>submenu items</nav></body></html>",
        ".drop{visibility:hidden;animation:pop .2s forwards}",
        800.0,
    );
    assert!(
        text_runs(&hover_menu).is_empty(),
        "visibility:hidden must stay hidden even with an animation"
    );
}

#[test]
fn a_percentage_height_needs_a_definite_containing_block() {
    fn content_of(root: &LayoutBox, class: &str) -> (f32, f32) {
        fn search(b: &LayoutBox, class: &str) -> Option<(f32, f32)> {
            let styled = b.get_styled_node()?;
            if styled.node.get_attribute("class") == Some(class) {
                return Some((b.dimensions.content.width, b.dimensions.content.height));
            }
            b.children.iter().find_map(|c| search(c, class))
        }
        search(root, class).expect("a box with that class")
    }

    // Brave's result thumbnails ship style="width:122px;height:100%" inside a
    // card with no declared height. CSS resolves that percentage to auto; using
    // the viewport as the base made every thumbnail 800px tall and stretched the
    // results page many screens beyond what Chrome shows.
    let thumb = layout_html(
        "<html><body><div class=card><img class=t src=t.jpg alt=T><p>caption</p></div></body></html>",
        ".card{width:600px}.t{width:122px;height:100%;display:inline-block}",
        1280.0,
    );
    let (tw, th) = content_of(&thumb, "t");
    assert_eq!(tw, 122.0, "a declared width still stands");
    assert!(
        th < 200.0,
        "percentage height in an auto-height parent must fall back to auto, got {th}"
    );

    // The declared pixel height still wins.
    let definite = layout_html(
        "<html><body><div class=wrap><div class=inner>x</div></div></body></html>",
        ".wrap{height:400px}",
        1280.0,
    );
    let (_, wh) = content_of(&definite, "wrap");
    assert_eq!(wh, 400.0, "a declared pixel height is unaffected");

    // Viewport units are a real definite base, so 50vh keeps its meaning.
    let vh = layout_html(
        "<html><body><div class=v>x</div></body></html>",
        ".v{height:50vh}",
        1280.0,
    );
    let (_, vh_h) = content_of(&vh, "v");
    assert!(vh_h > 300.0, "50vh must stay viewport-relative, got {vh_h}");
}

#[test]
fn rem_units_are_lengths_and_not_dropped_declarations() {
    use rust_browser::css::{Unit, Value};

    fn find_styled<'a>(n: &'a StyledNode<'a>, class: &str) -> Option<&'a StyledNode<'a>> {
        if n.node.get_attribute("class") == Some(class) {
            return Some(n);
        }
        n.children.iter().find_map(|c| find_styled(c, class))
    }

    let dom: &'static Node = Box::leak(Box::new(HtmlParser::parse(
        "<html><body><p class=t>hi</p><img class=i src=x></body></html>",
    )));
    let sheet = CssParser::parse(&format!(
        "{}.i{{width:32px;height:1.1rem;display:inline-block}}.t{{font-size:1.5rem}}",
        DEFAULT_USER_AGENT_CSS
    ));
    let styled = Box::leak(Box::new(style_tree(dom, &sheet)));

    // "1.1rem" ends in "em" too, so a suffix check ordered em-before-rem left an
    // unparseable "1.1r" and the whole declaration degraded to a keyword.
    assert_eq!(
        find_styled(styled, "i").unwrap().value("height"),
        Some(&Value::Length(1.1, Unit::Rem)),
        "rem must parse as a length"
    );

    // The consequences: a favicon sized in rem got the placeholder box instead.
    let mut root = build_layout_tree(styled).unwrap();
    root.layout(rust_browser::layout::Dimensions {
        content: LayoutRect::new(0.0, 0.0, 1280.0, 0.0),
        padding: EdgeSizes::default(),
        border: EdgeSizes::default(),
        margin: EdgeSizes::default(),
    });
    let list = build_display_list(&root);
    let img_h = list
        .items
        .iter()
        .find_map(|i| match i {
            DisplayItem::Image { rect, .. } => Some(rect.height),
            _ => None,
        })
        .expect("an image box");
    assert!(
        (img_h - 17.6).abs() < 0.6,
        "height:1.1rem must be 17.6px, got {img_h}"
    );

    let font_size = list
        .items
        .iter()
        .find_map(|i| match i {
            DisplayItem::Text { size, .. } => Some(*size),
            _ => None,
        })
        .expect("a text run");
    assert!(
        (font_size - 24.0).abs() < 0.6,
        "font-size:1.5rem must be 24px, got {font_size}"
    );
}

#[test]
fn a_column_flex_item_keeps_the_height_its_content_produces() {
    fn height_of(root: &LayoutBox, class: &str) -> f32 {
        fn search(b: &LayoutBox, class: &str) -> Option<f32> {
            let styled = b.get_styled_node()?;
            if styled.node.get_attribute("class") == Some(class) && !matches!(b.box_type, BoxType::TextFragment { .. }) {
                return Some(b.dimensions.content.height);
            }
            b.children.iter().find_map(|c| search(c, class))
        }
        search(root, class).expect("a box with that class")
    }

    let sentence = "the quick brown fox jumps over the lazy dog while nobody watches at all ";
    let paragraph: String = sentence.repeat(12);
    let body = format!(
        "<html><body><div class=outer><p class=one>{p}</p><p class=two>{p}</p></div></body></html>",
        p = paragraph
    );

    let block = layout_html(&body, ".one { color: red }", 600.0);
    let flex = layout_html(
        &body,
        ".outer { display: flex; flex-direction: column } .one { color: red }",
        600.0,
    );

    let block_h = height_of(&block, "outer");
    let flex_h = height_of(&flex, "outer");
    assert!(
        flex_h > 400.0 && block_h > 400.0,
        "the fixture must actually wrap: block={block_h} flex={flex_h}"
    );
    assert!(
        (flex_h - block_h).abs() < 2.0,
        "a column flex container must be as tall as the same content in normal flow, got flex={flex_h} block={block_h}"
    );
    assert!(
        height_of(&flex, "one") < 400.0,
        "a wrapped paragraph in a flex column must not be forced to its summed line height, got {}",
        height_of(&flex, "one")
    );

    // A declared height still wins, so the escape hatch is unchanged.
    let pinned = layout_html(
        &body,
        ".outer { display: flex; flex-direction: column } .one { height: 300px }",
        600.0,
    );
    assert_eq!(height_of(&pinned, "one"), 300.0);
}
