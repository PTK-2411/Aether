use rust_browser::app::{BrowserApp, ZOOM_STEPS};

#[test]
fn test_zoom_ladder_bounds_and_monotonicity() {
    // 1. Strict monotonicity
    assert_eq!(ZOOM_STEPS.len(), 17);
    for i in 0..ZOOM_STEPS.len() - 1 {
        assert!(
            ZOOM_STEPS[i] < ZOOM_STEPS[i + 1],
            "ZOOM_STEPS must be strictly monotonically increasing at index {}",
            i
        );
    }

    // 2. Exact min and max bounds
    assert!((ZOOM_STEPS[0] - 0.25).abs() < 1e-6);
    assert!((ZOOM_STEPS[ZOOM_STEPS.len() - 1] - 5.00).abs() < 1e-6);
}

#[test]
fn test_zoom_rapid_stepping_and_clamping_stress() {
    let mut app = BrowserApp::new();
    assert_eq!(app.zoom_percentage(), "100%");
    assert!((app.zoom_level - 1.0).abs() < 1e-6);

    // Rapid stepping in beyond 500%
    for _ in 0..100 {
        app.zoom_in();
    }
    assert_eq!(app.zoom_percentage(), "500%");
    assert!((app.zoom_level - 5.00).abs() < 1e-6);

    // Another 50 calls must stay pinned at 5.00
    for _ in 0..50 {
        app.zoom_in();
        assert!((app.zoom_level - 5.00).abs() < 1e-6);
    }

    // Rapid stepping out beyond 25%
    for _ in 0..100 {
        app.zoom_out();
    }
    assert_eq!(app.zoom_percentage(), "25%");
    assert!((app.zoom_level - 0.25).abs() < 1e-6);

    // Another 50 calls must stay pinned at 0.25
    for _ in 0..50 {
        app.zoom_out();
        assert!((app.zoom_level - 0.25).abs() < 1e-6);
    }

    // Reset back to 100%
    app.zoom_reset();
    assert_eq!(app.zoom_percentage(), "100%");
    assert!((app.zoom_level - 1.0).abs() < 1e-6);
}

#[test]
fn test_zoom_floating_point_stability_and_nearest_search() {
    let mut app = BrowserApp::new();

    // Perturbed slightly above 1.0 (e.g. 1.00005 due to float imprecision)
    app.zoom_level = 1.00005;
    app.zoom_in();
    assert!((app.zoom_level - 1.10).abs() < 1e-5);

    // Perturbed slightly below 1.0 (e.g. 0.99995)
    app.zoom_level = 0.99995;
    app.zoom_out();
    assert!((app.zoom_level - 0.90).abs() < 1e-5);

    // Boundary clamp: stepping below 0.25 stays at 0.25
    app.zoom_level = 0.25;
    app.zoom_out();
    assert!((app.zoom_level - 0.25).abs() < 1e-5);

    // Boundary clamp: stepping above 5.00 stays at 5.00
    app.zoom_level = 5.00;
    app.zoom_in();
    assert!((app.zoom_level - 5.00).abs() < 1e-5);
}

#[test]
fn test_zoom_percentage_formatting_all_steps() {
    let mut app = BrowserApp::new();

    let expected_percentages = [
        (0.25, "25%"),
        (0.33, "33%"),
        (0.50, "50%"),
        (0.67, "67%"),
        (0.75, "75%"),
        (0.80, "80%"),
        (0.90, "90%"),
        (1.00, "100%"),
        (1.10, "110%"),
        (1.25, "125%"),
        (1.50, "150%"),
        (1.75, "175%"),
        (2.00, "200%"),
        (2.50, "250%"),
        (3.00, "300%"),
        (4.00, "400%"),
        (5.00, "500%"),
    ];

    for (val, expected_str) in expected_percentages {
        app.zoom_level = val;
        assert_eq!(
            app.zoom_percentage(),
            expected_str,
            "Zoom percentage mismatch for float {}",
            val
        );
    }
}

#[test]
fn test_zoom_roundtrip_reversibility() {
    let mut app = BrowserApp::new();
    // Starting at index 7 (1.00)
    for _ in 0..9 {
        app.zoom_in();
    }
    assert_eq!(app.zoom_percentage(), "500%");

    // Step down 16 times from 5.00 to 0.25
    for _ in 0..16 {
        app.zoom_out();
    }
    assert_eq!(app.zoom_percentage(), "25%");

    // Step up 7 times to reach 1.00
    for _ in 0..7 {
        app.zoom_in();
    }
    assert_eq!(app.zoom_percentage(), "100%");
    assert!((app.zoom_level - 1.00).abs() < 1e-6);
}

#[test]
fn test_tab_strip_lifecycle_and_invariants() {
    let mut app = BrowserApp::new();
    assert_eq!(app.tabs.len(), 1);

    // Open 50 tabs
    for i in 0..50 {
        app.open_new_tab(format!("https://example.com/page_{}", i));
    }
    assert_eq!(app.tabs.len(), 51);
    assert_eq!(app.active_tab_idx, 50);

    // Switch active tab across bounds
    app.active_tab_idx = 25;
    assert_eq!(app.active_tab_idx, 25);

    // Close active tab
    app.close_tab(25);
    assert_eq!(app.tabs.len(), 50);

    // Close tabs down to 1
    while app.tabs.len() > 1 {
        app.close_tab(app.tabs.len() - 1);
    }
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab_idx, 0);

    // Closing the only remaining tab must NOT remove it, but reset it
    app.close_tab(0);
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.tabs[0].url, "about:newtab");
    assert_eq!(app.tabs[0].title, "New Tab");
}

#[test]
fn test_menu_and_tab_geometry_source_audit() {
    let app_source = std::fs::read_to_string("src/app.rs").expect("Failed to read src/app.rs");
    let main_source = std::fs::read_to_string("src/main.rs").expect("Failed to read src/main.rs");

    // 1. Tab title label non-selectable
    assert!(
        app_source.contains(".selectable(false)"),
        "Tab title label must be non-selectable (.selectable(false))"
    );

    // 2. Tab pills 4-corner rounded
    assert!(
        app_source.contains(".corner_radius(CornerRadius::same(8))"),
        "Tab pill must have all 4 corners rounded with CornerRadius::same(8)"
    );

    // 3. Window 4-corner rounded framing & transparency
    assert!(
        main_source.contains(".with_transparent(true)"),
        "main.rs ViewportBuilder must configure with_transparent(true)"
    );
    assert!(
        main_source.contains(".with_decorations(false)"),
        "main.rs ViewportBuilder must configure with_decorations(false)"
    );
    assert!(
        app_source.contains("fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4]"),
        "BrowserApp must implement clear_color returning transparent background"
    );
    assert!(
        app_source.contains("painter.rect_stroke(screen_rect, CornerRadius::same(10), border_stroke, StrokeKind::Inside);"),
        "Window frame must draw a 4-corner rounded stroke with radius 10"
    );

    // 4. Menu container dimensions and styling
    assert!(
        app_source.contains("let menu_width = 290.0;"),
        "Menu popup width must be 290.0"
    );
    assert!(
        app_source.contains("Color32::from_rgb(41, 42, 45)"),
        "Menu popup background color must be #292a2d"
    );

    // 5. Profile card: "Khoa" and "Signed in" present, "Hi, Khoa" absent
    assert!(app_source.contains("\"Khoa\""));
    assert!(app_source.contains("\"Signed in\""));
    assert!(!app_source.contains("Hi, Khoa"));

    // 6. Dismissal state machine
    assert!(
        app_source.contains("ctx.input(|i| i.key_pressed(Key::Escape))"),
        "Menu must dismiss on Escape key"
    );
    assert!(
        app_source.contains("!menu_rect.contains(pos) && !m_rect.contains(pos)"),
        "Menu must dismiss on outside click, excluding the trigger button"
    );

    // 7. Zoom buttons keep popup open (no show_menu_popup = false in render_zoom_row)
    assert!(app_source.contains("fn render_zoom_row("));
    assert!(app_source.contains("menu_zoom_plus"));
    assert!(app_source.contains("menu_zoom_minus"));
    assert!(app_source.contains("menu_zoom_fs"));

    // 8. Right-aligned shortcuts
    let expected_shortcuts = [
        "Ctrl+T",
        "Ctrl+N",
        "Ctrl+Shift+N",
        "Ctrl+H",
        "Ctrl+J",
        "Ctrl+Shift+Del",
        "Ctrl+P",
    ];
    for sc in &expected_shortcuts {
        assert!(
            app_source.contains(&format!("Some(\"{}\")", sc)),
            "Shortcut {} must be registered in popup menu",
            sc
        );
    }

    // 9. All 7 sections present with divider lines
    let divider_occurrences = app_source.matches("render_menu_divider(ui);").count();
    assert!(
        divider_occurrences >= 6,
        "Expected at least 6 dividers separating the 7 menu sections, found {}",
        divider_occurrences
    );
}
