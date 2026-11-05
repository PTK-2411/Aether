# BRIEFING — 2026-11-05T08:00:50Z

## Mission
Inspect reference images and specify requirements for Tab/Window geometry polish and Chrome 3-dots popup menu.

## 🔒 My Identity
- Archetype: teamwork_spec_miner
- Roles: Specification Miner
- Working directory: /home/ptk/Documents/Projects/Rust-browser/.agents/teamwork/teamwork_spec_miner_survey_ui_2
- Original parent: ede09244-cd85-447e-82fe-12ac1159a1a2
- Milestone: UI Polishing - Window/Tab geometry & Chrome 3-dots Menu Specification

## 🔒 Key Constraints
- Read-only specification miner: do NOT implement code or modify codebase directly.
- Examine reference images thoroughly with view_file.
- MANDATORY FIRST STEP: Read ORIGINAL_REQUEST.md.
- Specify UI layout, colors, typography, state machine, interactions for Tab/Window polish and 3-dots popup menu.
- Output handoff.md in working directory and notify parent via send_message.

## Current Parent
- Conversation ID: ede09244-cd85-447e-82fe-12ac1159a1a2
- Updated: not yet

## Task Summary
- **What to build**: Comprehensive specification for Tab & Window frame geometry polish, non-selectable text, and Chrome 3-dots popup menu.
- **Success criteria**: Complete catalog of sections, rows, shortcuts, styling, state machine, corner radii, and egui implementation guidance documented in handoff.md.
- **Interface contracts**: handoff.md specification.
- **Code layout**: egui browser frontend in Rust-browser repo.

## Key Decisions Made
- Analyzed Reference Image 1: Identified root cause of tab title text selection (egui `Label` default `selectable` behavior), specified `.selectable(false)` fix, 4-corner rounded tab pills (`CornerRadius::same(8)`), and frameless window 4-corner rounding (`with_transparent(true)`, `clear_color: [0.0, 0.0, 0.0, 0.0]`, `CornerRadius::same(10)`).
- Analyzed Reference Image 2: Cataloged all 7 sections, 20+ rows, icons, right-aligned shortcuts (`Ctrl+T`, `Ctrl+N`, `Ctrl+Shift+N`, `Ctrl+J`, `Ctrl+Shift+Del`, `Ctrl+P`), profile banner ("Khoa", "Signed in"), and interactive zoom control row (`[-]`, `100%`, `[+]`, Fullscreen toggle).
- Defined state machine, dismiss logic (click-outside and Escape key), and action routing.
- Completed full specification and 5-component report in handoff.md.

## Artifact Index
- DISPATCH.md — Dispatch instructions
- BRIEFING.md — Working memory
- progress.md — Heartbeat and task progress
- handoff.md — Detailed specification handoff report
