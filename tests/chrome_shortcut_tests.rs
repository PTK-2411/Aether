use eframe::egui::{Event, InputState, Key, Modifiers};
use rust_browser::app::{detect_chrome_shortcut as detect, ChromeShortcut as Sc, ScrollIntent};

fn press(modifiers: Modifiers, key: Key) -> InputState {
    let mut input = InputState::default();
    input.modifiers = modifiers;
    input.events.push(Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    });
    input
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Default::default()
    }
}

fn ctrl_shift() -> Modifiers {
    Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    }
}

fn alt() -> Modifiers {
    Modifiers {
        alt: true,
        ..Default::default()
    }
}

fn none() -> Modifiers {
    Modifiers::NONE
}

#[test]
fn tab_shortcuts_match_chrome() {
    assert_eq!(detect(&press(ctrl(), Key::T), false), Some(Sc::NewTab));
    assert_eq!(detect(&press(ctrl_shift(), Key::T), false), Some(Sc::ReopenClosedTab));
    assert_eq!(detect(&press(ctrl(), Key::W), false), Some(Sc::CloseTab));
    assert_eq!(detect(&press(ctrl(), Key::Tab), false), Some(Sc::NextTab));
    assert_eq!(detect(&press(ctrl_shift(), Key::Tab), false), Some(Sc::PrevTab));
}

#[test]
fn omnibox_and_reload_shortcuts_work_while_typing() {
    // Chrome keeps these live even when the address bar already has focus.
    assert_eq!(detect(&press(ctrl(), Key::L), true), Some(Sc::FocusOmnibox));
    assert_eq!(detect(&press(ctrl(), Key::F6), true), Some(Sc::FocusOmnibox));
    assert_eq!(detect(&press(ctrl(), Key::R), true), Some(Sc::Reload));
    assert_eq!(detect(&press(none(), Key::F5), true), Some(Sc::Reload));
    assert_eq!(detect(&press(none(), Key::F5), false), Some(Sc::Reload));
}

#[test]
fn plain_letters_are_never_browser_commands() {
    // Typing "w" or "t" into the omnibox must not close or open a tab.
    assert_eq!(detect(&press(none(), Key::W), true), None);
    assert_eq!(detect(&press(none(), Key::T), true), None);
    assert_eq!(detect(&press(none(), Key::L), true), None);
}

#[test]
fn tab_number_shortcuts() {
    assert_eq!(detect(&press(ctrl(), Key::Num1), false), Some(Sc::SelectTab(0)));
    assert_eq!(detect(&press(ctrl(), Key::Num3), false), Some(Sc::SelectTab(2)));
    assert_eq!(detect(&press(ctrl(), Key::Num8), false), Some(Sc::SelectTab(7)));
    assert_eq!(detect(&press(ctrl(), Key::Num9), false), Some(Sc::LastTab));
    // Ctrl+0 is zoom reset, not tab 0.
    assert_eq!(detect(&press(ctrl(), Key::Num0), false), Some(Sc::ZoomReset));
}

#[test]
fn zoom_shortcuts_cover_both_keycodes() {
    assert_eq!(detect(&press(ctrl(), Key::Equals), false), Some(Sc::ZoomIn));
    assert_eq!(
        detect(&press(ctrl_shift(), Key::Plus), false),
        Some(Sc::ZoomIn)
    );
    assert_eq!(detect(&press(ctrl(), Key::Minus), false), Some(Sc::ZoomOut));
}

#[test]
fn history_navigation_uses_alt_arrows() {
    assert_eq!(detect(&press(alt(), Key::ArrowLeft), false), Some(Sc::Back));
    assert_eq!(detect(&press(alt(), Key::ArrowRight), false), Some(Sc::Forward));
    // Bare arrows scroll the page instead.
    assert_eq!(
        detect(&press(none(), Key::ArrowDown), false),
        Some(Sc::Scroll(ScrollIntent::Lines(3)))
    );
    assert_eq!(
        detect(&press(none(), Key::PageDown), false),
        Some(Sc::Scroll(ScrollIntent::Pages(0.9)))
    );
    assert_eq!(
        detect(&press(none(), Key::Home), false),
        Some(Sc::Scroll(ScrollIntent::Top))
    );
    assert_eq!(
        detect(&press(none(), Key::End), false),
        Some(Sc::Scroll(ScrollIntent::Bottom))
    );
}

#[test]
fn page_scrolling_is_suppressed_inside_text_fields() {
    assert_eq!(detect(&press(none(), Key::ArrowDown), true), None);
    assert_eq!(detect(&press(none(), Key::PageDown), true), None);
    assert_eq!(detect(&press(none(), Key::Home), true), None);
    assert_eq!(detect(&press(none(), Key::Escape), true), Some(Sc::Escape));
}

#[test]
fn devtools_combo_is_left_to_the_existing_handler() {
    assert_eq!(detect(&press(ctrl_shift(), Key::I), false), None);
}

#[test]
fn unmodified_keys_without_bindings_are_ignored() {
    assert_eq!(detect(&press(none(), Key::A), false), None);
    assert_eq!(detect(&InputState::default(), false), None);
}
