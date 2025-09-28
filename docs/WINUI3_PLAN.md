# WinUI 3 Implementation Plan

## Stage 1. Project Bootstrapping
- **Goal:** Replace the placeholder WinUI stub with a functioning WinUI 3 window.
- **Workstreams:**
  - Update `Cargo.toml` features to include required WinUI metadata.
  - Add a `App.xaml` and `MainWindow.xaml` (or WinUI markup equivalents) embedded via `include_str!` or packaged resources.
  - Implement `platform::winui::run()` with App/MainWindow types, hooking up dispatcher queues and activation logic.
  - Ensure assets directory is accessible (either copy to output or load via URI).
- **Exit Criteria:** `cargo run -p hearts-app --features winui-host` launches a WinUI window with a basic title and close handling.

## Stage 2. UI Shell & Layout
- **Goal:** Establish the main table layout and navigation structure.
- **Workstreams:**
  - Define visual tree: table surface, player seats, score panels.
  - Wire a view-model layer (Rust structs) to push state into WinUI controls (e.g., via `ObservableCollection` interop or message passing).
  - Integrate theme manifest to load card/table assets.
- **Exit Criteria:** Static mock UI displaying card placeholders, player names, and score boxes that resize appropriately.

## Stage 3. Interaction Wiring
- **Goal:** Hook core game logic to the UI.
- **Workstreams:**
  - Bridge hearts-core match state to WinUI view models (async messaging or state snapshots per tick).
  - Implement card selection, passing phase overlays, and trick resolution animations.
  - Add snapshot load/export hooks in the UI (developer menu / keyboard shortcuts).
- **Exit Criteria:** User can play through an offline match (AI opponents) entirely in the WinUI shell.

## Stage 4. Polish & Accessibility
- **Goal:** Bring the WinUI experience to production quality.
- **Workstreams:**
  - Implement final animations, themes, and high-DPI assets.
  - Add accessibility (keyboard navigation, narrator text, color-contrast options).
  - Integrate settings panel (audio toggles, house rules, theme selection).
- **Exit Criteria:** UI meets accessibility guidelines and mirrors the design spec (animations, theme switching, settings persistence).

## Stage 5. Packaging & QA
- **Goal:** Ready the WinUI build for distribution/testing.
- **Workstreams:**
  - Produce MSIX packaging scripts (AppxManifest, assets, signing instructions).
  - Add WinAppDriver smoke tests.
  - Document developer/testing workflow for the WinUI build.
- **Exit Criteria:** Signed MSIX package, automated UI smoke tests, and updated documentation.
