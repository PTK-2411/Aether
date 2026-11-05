Clean up and refine the Rust browser UI and search behavior for production use, routing web searches to Google, changing the profile pill to Login, and removing subtitle text, action chips, and all AI-related buttons across the interface.

Working directory: /home/ptk/Documents/Projects/Rust-browser
Integrity mode: demo
The team must NOT: Use pre-built libraries or frameworks for core functionality

## Requirements

### R1. Google Search Navigation
Configure search queries in the Omnibox and the New Tab search bar to navigate to Google search (`https://www.google.com/search?q=...`). If the entered query is a valid URL or domain (e.g., beginning with a scheme or matching standard domain formats), navigate directly to that URL instead of performing a search.

### R2. Header and New Tab Page Simplification
In `src/app.rs`:
- Replace the profile pill ("Hi, Khoa" with avatar "K") with a clean "Login" button/pill (the team decides the click interaction/behavior).
- Remove the subtitle text "100% Pure Rust Web Browser • Zero Chromium" under the Aether logo on the New Tab page.
- Remove the two action chips underneath the search box on the New Tab page ("Aether AI Assistant - Instant search, summaries & intelligence" and "Explore Open Web - Acid test suite & layout verification").

### R3. Removal of AI UI Elements
Remove all AI-related buttons, links, and shortcuts across the UI in `src/app.rs`:
- The "✦ AI Mode" pill inside the Omnibox.
- The "✦ AI Mode" button inside the New Tab search bar.
- The "✦ Aether AI" link in the top-right header of the New Tab page.
- The "Aether AI" quick shortcut circle from the New Tab page.

### R4. Request Parameter Handling
The team may decide whether any action is needed regarding `/graphify` mentioned in the request, or treat it as a non-operative command.

## Verification Resources
- Rust toolchain commands: `cargo check` and `cargo test` in `/home/ptk/Documents/Projects/Rust-browser`.
- Visual inspection of `src/app.rs` UI components to verify layout integrity and element removal.

## Acceptance Criteria

### Search & Navigation
- [ ] Entering a search query into the Omnibox navigates to `https://www.google.com/search?q=<query>`.
- [ ] Entering a search query into the New Tab search bar navigates to `https://www.google.com/search?q=<query>`.
- [ ] Entering a valid URL or domain navigates directly to the target URL.

### UI Simplification & Cleanup
- [ ] The top bar profile displays a "Login" button/pill in place of "Hi, Khoa" and avatar "K".
- [ ] The New Tab page does not display the subtitle "100% Pure Rust Web Browser • Zero Chromium".
- [ ] The New Tab page does not display the two action chips ("Aether AI Assistant" and "Explore Open Web").
- [ ] The Omnibox does not contain the "✦ AI Mode" pill.
- [ ] The New Tab search bar does not contain the "✦ AI Mode" button.
- [ ] The New Tab page header does not contain the "✦ Aether AI" link.
- [ ] The New Tab page shortcuts do not contain the "Aether AI" circle.

### Code Quality & Build Verification
- [ ] `cargo check` completes with no compilation errors.
- [ ] `cargo test` runs and passes existing tests.
- [ ] The modified UI renders cleanly without layout overlap or visual breakage.
