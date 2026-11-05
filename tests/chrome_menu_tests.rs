use rust_browser::app::{BrowserApp, ZOOM_STEPS};

#[test]
fn test_chrome_menu_zoom_stepping_ladder() {
    let mut app = BrowserApp::new();
    assert_eq!(app.zoom_percentage(), "100%");

    // Verify ZOOM_STEPS ladder bounds
    assert_eq!(ZOOM_STEPS.first(), Some(&0.25));
    assert_eq!(ZOOM_STEPS.last(), Some(&5.00));
    assert_eq!(ZOOM_STEPS.len(), 17);

    // Test stepping in
    let expected_steps_up = [1.10, 1.25, 1.50, 1.75, 2.00, 2.50, 3.00, 4.00, 5.00];
    for &expected in &expected_steps_up {
        app.zoom_in();
        assert!((app.zoom_level - expected).abs() < 0.001);
    }
    // At maximum 500%, stepping in clamps
    app.zoom_in();
    assert!((app.zoom_level - 5.00).abs() < 0.001);

    // Test stepping out down to 25%
    let expected_steps_down = [
        4.00, 3.00, 2.50, 2.00, 1.75, 1.50, 1.25, 1.10,
        1.00, 0.90, 0.80, 0.75, 0.67, 0.50, 0.33, 0.25,
    ];
    for &expected in &expected_steps_down {
        app.zoom_out();
        assert!((app.zoom_level - expected).abs() < 0.001);
    }
    // At minimum 25%, stepping out clamps
    app.zoom_out();
    assert!((app.zoom_level - 0.25).abs() < 0.001);

    // Reset to 100%
    app.zoom_reset();
    assert_eq!(app.zoom_percentage(), "100%");
}

#[test]
fn test_chrome_menu_source_invariants() {
    let source = std::fs::read_to_string("src/app.rs").expect("Failed to read src/app.rs");

    // Profile card: "Khoa" present, "Hi, Khoa" strictly absent
    assert!(source.contains("\"Khoa\""), "Profile card name 'Khoa' missing");
    assert!(!source.contains("Hi, Khoa"), "'Hi, Khoa' must not be in src/app.rs");
    assert!(source.contains("\"Signed in\""), "Profile card badge 'Signed in' missing");

    // Navigation section
    assert!(source.contains("\"New tab\""));
    assert!(source.contains("\"Ctrl+T\""));
    assert!(source.contains("\"New window\""));
    assert!(source.contains("\"Ctrl+N\""));
    assert!(source.contains("\"New Incognito window\""));
    assert!(source.contains("\"Ctrl+Shift+N\""));

    // Tools section
    assert!(source.contains("\"Passwords and autofill\""));
    assert!(source.contains("\"History\""));
    assert!(source.contains("\"Downloads\""));
    assert!(source.contains("\"Ctrl+J\""));
    assert!(source.contains("\"Bookmarks and lists\""));
    assert!(source.contains("\"Delete browsing data...\""));
    assert!(source.contains("\"Ctrl+Shift+Del\""));

    // Interactive Zoom row
    assert!(source.contains("\"Zoom\""));
    assert!(source.contains("zoom_in"));
    assert!(source.contains("zoom_out"));

    // Actions & System
    assert!(source.contains("\"Print...\""));
    assert!(source.contains("\"Ctrl+P\""));
    assert!(source.contains("\"More tools\""));
    assert!(source.contains("\"Settings\""));
    assert!(source.contains("\"Exit\""));

    // Dismissal logic
    assert!(source.contains("Key::Escape"));
    assert!(source.contains("menu_rect.contains(pos)"));
}
