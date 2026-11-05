## 2026-11-05T06:17:30Z
You are a teamwork_worker.
Your working directory is: /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_worker_m1_1/
You must update your progress in: /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_worker_m1_1/progress.md
You must write your final handoff report to: /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_worker_m1_1/handoff.md

MANDATORY: You MUST read the original user request at:
/home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/ORIGINAL_REQUEST.md
before starting your work.

Read the project specification at:
/home/ptk/Documents/Projects/Rust-browser/PROJECT.md

Read the detailed implementation plans and verified before/after diffs from the three Explorers:
- /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_explorer_m1_1/handoff.md (Network & tests)
- /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_explorer_m1_2/handoff.md (Header & New Tab simplification)
- /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_explorer_m1_3/handoff.md (AI element removals & autocomplete dropdown)

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Exclusive file ownership: You own:
- `src/network.rs`
- `src/app.rs`
- `tests/browser_tests.rs`

Your implementation tasks:
1. `src/network.rs`:
   - Import `std::net::Ipv4Addr;`.
   - Implement `is_valid_domain_or_ip(trimmed: &str) -> bool` to accurately detect valid domains (RFC standard labels, alphabetic TLD length >= 2, no whitespace) and IPv4 addresses (with optional ports).
   - In `normalize_url`:
     * Preserve schemes (`about:`, `file://`, `http://`, `https://`).
     * Route `localhost`, `localhost:<port>`, `localhost/<path>` to `http://localhost...`.
     * Direct navigation to `https://{}` if valid domain or IP.
     * Route all search queries to `https://www.google.com/search?q={encoded}` using `url::form_urlencoded::byte_serialize`.
   - Update search result page titles and branding from "Aether Search" to "Google Search" at lines 367, 436, and 455.

2. `src/app.rs`:
   - Autocomplete dropdown (line 418): change `self.navigate_active_tab(&format!("https://search.aether.local/?q={}", query));` to `self.navigate_active_tab(&query);`.
   - Omnibox (line 888): update `available_edit_w = child_ui.available_width();` (remove `- 85.0`).
   - Omnibox "✦ AI Mode" pill (lines 910-925): remove the entire AI Mode pill block.
   - Top bar profile pill (lines 930-945): replace "Hi, Khoa" with avatar "K" with a clean "Login" pill:
     * Size: 64x28 stadium pill (corner radius 14.0).
     * Text: "Login" centered (`Align2::CENTER_CENTER`), `FontId::proportional(12.0)`, `Color32::from_rgb(232, 234, 237)`.
     * Click behavior: `self.navigate_active_tab("https://accounts.google.com");`.
   - New Tab page header (lines 1053-1058): remove "✦ Aether AI" link and preceding `ui.add_space(8.0);`.
   - New Tab page subtitle (lines 1083-1089): remove `ui.add_space(2.0);` and subtitle text "100% Pure Rust Web Browser • Zero Chromium".
   - New Tab search bar (line 1112): update `input_width = inner_ui.available_width() - 65.0;`.
   - New Tab search bar "✦ AI Mode" button (lines 1139-1153): remove button and trailing space. Camera icon click at line 1162 should not navigate to `about:ai` (make hover-only or no-op).
   - New Tab action chips (lines 1179-1235): remove both action chips ("Aether AI Assistant" and "Explore Open Web"), replacing the block with a single `ui.add_space(28.0);`.
   - New Tab shortcuts row (line 1241): update `total_shortcuts_w = 6.0 * 68.0;`.
   - New Tab shortcut circle (lines 1281-1288): remove "Aether AI" vector shortcut item.

3. `tests/browser_tests.rs`:
   - Update and expand `test_url_normalization_and_resolution` to comprehensively test Google search navigation, queries with spaces, single-word queries, queries with dots/numbers (e.g. `1.0`, `1.2.3`), valid domains, localhost, IPv4, and explicit schemes.

4. Build and Test Verification:
   - Run `cargo check` and verify 0 compilation errors and 0 warnings.
   - Run `cargo test` and verify that all unit/integration tests pass.
   - Document verification commands and full outputs in your handoff report.
