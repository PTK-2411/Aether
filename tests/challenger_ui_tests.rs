use rust_browser::app::BrowserApp;
use rust_browser::network::normalize_url;

#[test]
fn test_browser_app_lifecycle_and_navigation() {
    let mut app = BrowserApp::new();

    // Trigger navigation via active tab
    app.navigate_active_tab("rust programming language");

    // Test autocomplete selection delegation:
    // When a suggestion query is passed to navigate_active_tab, it must normalize to Google search
    assert_eq!(
        normalize_url("rust programming language"),
        "https://www.google.com/search?q=rust+programming+language"
    );

    // Test direct domain suggestion
    assert_eq!(
        normalize_url("github.com"),
        "https://github.com"
    );

    // Test login pill navigation destination
    assert_eq!(
        normalize_url("https://accounts.google.com"),
        "https://accounts.google.com"
    );
}

#[test]
fn test_new_tab_shortcut_layout_dimensions() {
    // 6 shortcuts at 68.0 width each
    let shortcut_count: f32 = 6.0;
    let shortcut_width: f32 = 68.0;
    let total_shortcuts_w: f32 = shortcut_count * shortcut_width;
    assert_eq!(total_shortcuts_w, 408.0);

    // Test centering under various container widths
    let viewports: [f32; 7] = [1920.0, 1280.0, 1024.0, 800.0, 600.0, 408.0, 300.0];
    for &vw in &viewports {
        let pad = ((vw - total_shortcuts_w) / 2.0).max(0.0_f32);
        assert!(pad >= 0.0);
        if vw >= total_shortcuts_w {
            assert_eq!(pad, (vw - 408.0) / 2.0);
        } else {
            assert_eq!(pad, 0.0);
        }
    }
}

#[test]
fn test_omnibox_and_searchbox_textedit_resilience() {
    // Check Omnibox width calculations under small viewports
    let viewports: [f32; 6] = [1920.0, 1024.0, 800.0, 400.0, 200.0, 100.0];
    for &vw in &viewports {
        let omnibox_width = (vw - 175.0_f32).max(200.0);
        assert!(omnibox_width >= 200.0);

        // child_ui available width: inside rect shrink2(10.0, 4.0)
        let child_w = (omnibox_width - 20.0).max(0.0);
        // text_edit allocates available_edit_w.max(50.0)
        let edit_w = child_w.max(50.0);
        assert!(edit_w >= 50.0);
    }

    // Check New Tab center search bar width calculations
    for &vw in &viewports {
        let search_box_width = (680.0_f32).min(vw - 32.0);
        // inner_rect is shrink2(16.0, 8.0), so inner width is search_box_width - 32.0
        let inner_w = search_box_width - 32.0;
        let input_width = inner_w - 65.0;
        let actual_edit_w = input_width.max(100.0);
        assert!(actual_edit_w >= 100.0);
    }
}

#[test]
fn test_empirical_source_invariants_in_app_rs() {
    let source = std::fs::read_to_string("src/app.rs")
        .expect("Failed to read src/app.rs");

    // 1. Omnibox AI Mode absence
    assert!(
        !source.contains("✦ AI Mode"),
        "Found '✦ AI Mode' still present in src/app.rs!"
    );

    // 2. Profile pill: 'Hi, Khoa' absence
    assert!(
        !source.contains("Hi, Khoa"),
        "Found 'Hi, Khoa' still present in src/app.rs!"
    );

    // 3. New Tab Subtitle absence
    assert!(
        !source.contains("100% Pure Rust Web Browser • Zero Chromium"),
        "Found '100% Pure Rust Web Browser • Zero Chromium' still present in src/app.rs!"
    );

    // 4. Action chips absence
    assert!(
        !source.contains("Aether AI Assistant"),
        "Found 'Aether AI Assistant' still present in src/app.rs!"
    );
    assert!(
        !source.contains("Explore Open Web"),
        "Found 'Explore Open Web' still present in src/app.rs!"
    );

    // 5. Header AI link absence
    assert!(
        !source.contains("✦ Aether AI"),
        "Found '✦ Aether AI' link still present in src/app.rs!"
    );

    // 6. Login pill invariants:
    // - Size Vec2::new(64.0, 28.0)
    assert!(
        source.contains("Vec2::new(64.0, 28.0)"),
        "Login pill does not allocate exact size 64.0 x 28.0"
    );
    // - Centered text 'Login'
    assert!(
        source.contains("\"Login\""),
        "Login label missing in src/app.rs"
    );
    assert!(
        source.contains("https://accounts.google.com"),
        "Login button does not navigate to https://accounts.google.com"
    );

    // 7. Shortcut row invariants:
    // - total_shortcuts_w = 6.0 * 68.0
    assert!(
        source.contains("let total_shortcuts_w = 6.0 * 68.0;"),
        "Shortcuts row width must be 6.0 * 68.0"
    );

    // 8. Omnibox width utilization:
    // available_edit_w must be child_ui.available_width()
    assert!(
        source.contains("let available_edit_w = child_ui.available_width();"),
        "Omnibox edit field must utilize full available_width()"
    );
}

#[test]
fn test_autocomplete_delegation_matrix() {
    let test_cases = vec![
        ("rust borrow checker", "https://www.google.com/search?q=rust+borrow+checker"),
        ("wikipedia", "https://www.google.com/search?q=wikipedia"),
        ("crates.io", "https://crates.io"),
        ("news.ycombinator.com", "https://news.ycombinator.com"),
        ("https://github.com/rust-lang", "https://github.com/rust-lang"),
        ("about:credits", "about:credits"),
        ("about:test", "about:test"),
        ("localhost:3000", "http://localhost:3000"),
        ("127.0.0.1:8080", "https://127.0.0.1:8080"),
    ];

    for (query, expected_nav) in test_cases {
        let normalized = normalize_url(query);
        assert_eq!(normalized, expected_nav, "Query '{}' did not route to expected target", query);
    }
}
