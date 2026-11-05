## 2026-11-05T07:55:22Z
You are teamwork_spec_miner assigned to specify requirements and architecture for the Chrome-Style Native DevTools Panel.
Your working directory is /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_spec_miner_survey_ui_3/
Your parent conversation ID is ede09244-cd85-447e-82fe-12ac1159a1a2.

MANDATORY FIRST STEP: Read /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/ORIGINAL_REQUEST.md.
Reference repository: https://github.com/ChromeDevTools/devtools-frontend

Task:
1. Specify DevTools panel integration in the Rust browser:
   - Panel container (e.g. bottom or right dockable panel in egui, or resizable bottom panel), toggleable via F12, Ctrl+Shift+I, or 3-dots menu "More tools -> Developer Tools" and close button ('×').
   - Panel header styling: Chrome DevTools dark theme (#202124 / #292a2d), icon buttons (inspect cursor, device toggle, close button '×'), top tab bar.
2. Specify top tab bar with 6 tabs:
   - Elements
   - Console
   - Sources
   - Network
   - Application
   - Security
3. Specify the Elements tab (Two-pane layout):
   - Left pane: Interactive DOM tree hierarchy with colored tokens (tag name e.g. `<div ...>` in purple/blue, attribute names in yellow/orange, attribute values in green, text nodes in light grey/white). Expand/collapse carets (▶ / ▼) for elements with children. Node selection state.
   - Right pane: Styles and Box Model inspector.
     - Styles section: Computed/matched CSS rules for selected element (e.g. display, color, font-size, background, etc.).
     - Box Model visual diagram: Nested colored rectangles matching Chrome DevTools: Margin (orange, showing top/right/bottom/left values), Border (yellow, showing border width), Padding (green, showing padding values), Content (blue, showing width x height values).
4. Specify the Console tab:
   - Interactive console output log with log levels (Info, Warn, Error), timestamps, colored text/icons, and filter/clear buttons.
5. Specify the Network tab:
   - Request table: Waterfall/list with columns (Name/URL, Status e.g. 200, Method e.g. GET, Type e.g. document/html, Size e.g. 1.2 KB, Time e.g. 45ms).
6. Specify the Sources tab:
   - Resource navigation tree (HTML, CSS, JS) and code viewer with syntax highlighting and line numbers.
7. Specify Application & Security tabs:
   - Application tab: Storage, Cookies, Cache manifest display.
   - Security tab: Connection security overview (valid HTTPS / TLS cert, mixed content status).
8. Detail data structures, state models, and integration bridges with Rust browser engine (src/dom.rs, src/layout.rs, src/style.rs, src/network.rs, src/app.rs).
9. Write your detailed specification to /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_spec_miner_survey_ui_3/handoff.md.
10. Call send_message to notify parent (ede09244-cd85-447e-82fe-12ac1159a1a2) of completion.
