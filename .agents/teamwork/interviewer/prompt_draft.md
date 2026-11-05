[OPEN: Team structure and project purpose] Update the Rust browser to use Google search, replace the profile pill with Login, and remove subtitle text, action chips, and AI-related UI elements.

Working directory: /home/ptk/Documents/Projects/Rust-browser
Integrity mode: [OPEN: Integrity mode restrictions]

## Requirements

### R1. Google Search Integration
Configure search queries in the Omnibox and New Tab search bar to navigate to Google search (`https://www.google.com/search?q=...`) instead of the synthetic search. [OPEN: Direct URL navigation vs search]

### R2. UI Header and New Tab Simplification
In `src/app.rs`:
- Change the profile display ("Hi, Khoa" with avatar "K") to a clean "Login" button/pill. [OPEN: Login button action]
- Remove the subtitle "100% Pure Rust Web Browser • Zero Chromium" under the logo on the New Tab page.
- Remove the two action chips under the New Tab search box ("Aether AI Assistant" and "Explore Open Web").

### R3. Removal of AI Elements
Remove all AI-related buttons, links, and quick shortcuts across the interface:
- The "✦ AI Mode" pill inside the Omnibox.
- The "✦ AI Mode" button inside the New Tab search bar.
- The "✦ Aether AI" header link on the New Tab page.
- The "Aether AI" quick shortcut circle from the New Tab page.

### R4. Request Parameter Clarification
[OPEN: Handling of "/graphify"]

## Acceptance Criteria

### Search & Navigation
- [ ] Entering a search query navigates to `https://www.google.com/search?q=...`.

### UI Updates
- [ ] The top bar displays a "Login" button/pill in place of "Hi, Khoa".
- [ ] The New Tab page does not display the subtitle "100% Pure Rust Web Browser • Zero Chromium".
- [ ] The New Tab page does not display the two action chips.
- [ ] No AI mode buttons, header links, or quick shortcut circles appear in the Omnibox or New Tab page.

### Code Quality & Build
- [ ] The codebase builds cleanly with `cargo check` and passes existing tests with `cargo test`.
- [ ] [OPEN: Additional verification criteria]
