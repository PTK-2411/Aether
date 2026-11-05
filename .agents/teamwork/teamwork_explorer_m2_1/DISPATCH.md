## 2026-11-05T08:04:20Z
You are teamwork_explorer assigned to Milestone M2 (Browser UI Polish - Tab & Window Geometry).
Your working directory is /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_explorer_m2_1/
Your parent conversation ID is ede09244-cd85-447e-82fe-12ac1159a1a2.

MANDATORY FIRST STEP: Read /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/ORIGINAL_REQUEST.md.
Also read:
- /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/orchestrator/PROJECT.md
- /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_spec_miner_survey_ui_2/handoff.md
- /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_explorer_survey_ui_1/handoff.md

Task:
Formulate the exact, concrete implementation strategy for Requirement R1 (Tab Title Selection Prevention and 4-Corner Rounding):
1. Exactly how to update src/app.rs in render_tab_strip (lines 660-710):
   - Replace CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 } with CornerRadius::same(8) (or nw:8, ne:8, sw:8, se:8).
   - Add .selectable(false) to egui::Label::new(...) so clicking/dragging on a tab title does NOT select text.
   - Maintain tab click, tab switch, and tab drag semantics without breaking tests.
2. Exactly how to update src/main.rs:
   - Configure ViewportBuilder with .with_transparent(true) and .with_decorations(false).
3. Exactly how to update src/app.rs for the 4-corner rounded window frame:
   - Implement clear_color returning [0.0, 0.0, 0.0, 0.0] on BrowserApp (or handling transparency).
   - Outer container frame with CornerRadius::same(10) (or 12) and 1px border stroke Stroke::new(1.0, Color32::from_rgb(60, 64, 67)).
   - Verify compatibility with existing challenger_ui_tests.rs string invariant checks.
4. Detail the exact code diffs and test cases to be implemented by the worker.
5. Write your comprehensive fix strategy report to /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_explorer_m2_1/handoff.md.
6. Call send_message to notify parent (ede09244-cd85-447e-82fe-12ac1159a1a2) of completion.
