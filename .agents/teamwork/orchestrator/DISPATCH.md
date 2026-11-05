# Dispatch Log

## 2026-11-05T07:52:10Z
Implement browser UI polish (non-selectable tab title text, 4-corner rounded tabs, 4-corner rounded window), a Chrome-inspired 3-dots popup menu, and a native Chrome DevTools panel for the Rust browser.

Working directory: /home/ptk/Documents/Projects/Rust-browser
Integrity mode: development

Reference material:
- Chrome DevTools Frontend repository: https://github.com/ChromeDevTools/devtools-frontend
- UI Reference Image 1 (tab selection & corner rounding): /home/ptk/.gemini/antigravity/brain/76bf5cd9-6ca4-421a-851e-9034dbe7d734/.user_uploaded/media_1793864095995_7f70a4e3.png
- UI Reference Image 2 (Chrome 3-dots menu layout & style): /home/ptk/.gemini/antigravity/brain/76bf5cd9-6ca4-421a-851e-9034dbe7d734/.user_uploaded/media_1793864186546_760d982c.png

## Requirements

### R1. Tab Title Selection Prevention and 4-Corner Rounding
- Tab title labels must not be text-selectable so that clicking or dragging on a tab interacts cleanly without text highlighting.
- All four corners of each tab pill must be rounded (top and bottom corners).
- The outer four corners of the browser window must be rounded.

### R2. Chrome-Style 3-Dots Popup Menu
- The 3-dots button (⋮) on the top right of the navigation bar must toggle a popup menu styled after Google Chrome's native menu (Image 2).
- The popup menu must feature a dark theme with rounded corners, subtle borders, grouped sections with divider lines, vector/icon styling, and right-aligned shortcut labels.
- Interactive controls must be wired for existing browser functions:
  - Zoom row with interactive `[-]`, percentage display, `[+]`, and fullscreen toggle.
  - Standard navigation/window actions (New Tab, New Window, History, Downloads, Settings, Exit/Quit).
- Other informational items (such as profile/account indicator) must be visually represented matching the reference layout.

### R3. Chrome-Style Native DevTools Panel
- A DevTools interface must be provided in the Rust browser, styled after Chrome DevTools with a dark theme and top tab navigation.
- DevTools tabs must include: Elements, Console, Sources, Network, Application, and Security.
- The Elements tab must provide a two-pane inspection layout:
  - An interactive DOM tree with colored tags, attributes, text content, and expand/collapse carets.
  - A Styles and Box Model inspector pane showing computed/CSS styles and margin/border/padding/content dimensions.
- The Console tab must display browser logs and output.
- The Network tab must display HTTP/network requests with status, method, URL, size, and timing information.
- The Sources tab must provide an HTML/CSS resource viewer.
- The DevTools panel must include a close button (`×`) and toggle shortcut/button.

## Verification Resources
- Build and test commands: `cargo check`, `cargo test`
- Interactive manual verification of UI components in the running browser (`cargo run`)

## Acceptance Criteria

### Tab and Window Geometry
- [ ] Clicking and dragging on a tab title does not select or highlight the tab text.
- [ ] Every browser tab has all four corners rounded.
- [ ] The browser window outer frame has four rounded corners.

### Chrome-Style 3-Dots Menu
- [ ] Clicking the 3-dots button opens a dark-themed popup menu styled like Image 2 with rounded corners and section dividers.
- [ ] Zoom buttons (`[-]` and `[+]`) actively adjust page zoom level, and percentage displays the current zoom.
- [ ] Selecting "New Tab" creates a new tab, and selecting "Exit" closes the application.
- [ ] Menu items display right-aligned keyboard shortcut hints (e.g. Ctrl+T, Ctrl+N, Ctrl+J).

### DevTools Layout and Features
- [ ] DevTools can be opened and toggled closed via UI button and/or shortcut.
- [ ] DevTools top tab bar allows switching between Elements, Console, Sources, Network, Application, and Security.
- [ ] Elements tab renders the DOM tree hierarchy with collapsible/expandable nodes.
- [ ] Selecting a node in the Elements DOM tree displays its styles and box model in the right/bottom pane.
- [ ] Network tab lists loaded resources with method, status code, and URL.
- [ ] Console tab displays logs and messages.
- [ ] Project compiles with `cargo check` and passes all existing automated tests with `cargo test`.
