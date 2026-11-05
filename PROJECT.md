# Project: Rust Browser UI & Search Refinement

## Architecture
Aether is a pure-Rust web browser built with `eframe` (egui) and standard networking crates.
- UI layer (`src/app.rs`): Renders the browser frame, tabs, Omnibox, navigation toolbar, and New Tab page.
- Network & URL layer (`src/network.rs`): Manages URL normalization (`normalize_url`), page fetching, search result interception, and resource loading.
- Test suite (`tests/browser_tests.rs`): Automated regression tests verifying parsers, layout calculations, and URL normalization.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Google Search Navigation | Route non-URL/domain queries in Omnibox & New Tab search bar to `https://www.google.com/search?q=...` | M1 | ORIGINAL_REQUEST § R1 |
| 2 | Robust URL & Domain Detection | Accurately distinguish valid URLs/domains (schemes, localhost, IPv4, multi-label domains with valid TLDs) from search queries | M1 | ORIGINAL_REQUEST § R1 |
| 3 | Autocomplete Dropdown Search Routing | Update search suggestions click handler in Omnibox/New Tab to route via Google search | M1 | Survey Explorer 1 |
| 4 | Header Profile Pill Replacement | Replace profile pill ("Hi, Khoa" with avatar "K") with a clean "Login" pill routing to `https://accounts.google.com` | M1 | ORIGINAL_REQUEST § R2 |
| 5 | New Tab Subtitle Removal | Remove subtitle text "100% Pure Rust Web Browser • Zero Chromium" under the Aether logo | M1 | ORIGINAL_REQUEST § R2 |
| 6 | New Tab Action Chips Removal | Remove the two action chips ("Aether AI Assistant" and "Explore Open Web") and normalize vertical spacing | M1 | ORIGINAL_REQUEST § R2 |
| 7 | Omnibox AI Mode Pill Removal | Remove "✦ AI Mode" pill inside Omnibox and reclaim full width for address text edit | M1 | ORIGINAL_REQUEST § R3 |
| 8 | New Tab Search AI Mode Button Removal | Remove "✦ AI Mode" button in center search bar and expand search input field | M1 | ORIGINAL_REQUEST § R3 |
| 9 | New Tab Header Aether AI Link Removal | Remove "✦ Aether AI" link from New Tab top-right header | M1 | ORIGINAL_REQUEST § R3 |
| 10 | New Tab Shortcut Circle Removal | Remove "Aether AI" quick shortcut circle and center the remaining 6 shortcuts | M1 | ORIGINAL_REQUEST § R3 |
| 11 | Request Parameter Non-operative Handling | Treat `/graphify` as non-operative per R4 | M1 | ORIGINAL_REQUEST § R4 |
| 12 | Test Suite Verification & Expansion | Verify `cargo check` and `cargo test` pass; add tests for Google search URL normalization | M1 | ORIGINAL_REQUEST § Verification |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | UI Simplification & Google Search Refinement | Complete all features 1-12 in `src/app.rs`, `src/network.rs`, and `tests/browser_tests.rs` | none | IN_PROGRESS |

## Interface Contracts
### `src/network.rs` ↔ `src/app.rs`
- `pub fn normalize_url(input: &str) -> String`
  - Input: raw user input from Omnibox or New Tab search bar.
  - Return:
    - If `input` is empty: `"about:newtab"`
    - If `input` has known scheme (`about:`, `file://`, `http://`, `https://`): preserved verbatim.
    - If `input` is `localhost` or `localhost:<port>` or `localhost/<path>`: `"http://localhost..."`
    - If `input` is valid IPv4 or standard domain format (`<name>.<tld>` with alphabetic TLD length >= 2, no spaces): `"https://..."`
    - Otherwise (search query): `"https://www.google.com/search?q=<percent_encoded_query>"`

## Code Layout
- `src/app.rs`: Toolbar, Omnibox, New Tab page rendering and input dispatch.
- `src/network.rs`: `normalize_url` implementation and search request interception.
- `tests/browser_tests.rs`: Integration tests for URL normalization and search routing.
