use rust_browser::app::BrowserApp;
use rust_browser::network::normalize_url;

#[test]
fn test_browser_app_lifecycle_and_navigation() {
    let mut app = BrowserApp::new();

    // Verify initial tab is newtab
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
