# Win32 / Direct2D UI Plan

The desktop client will use classic Win32 APIs (window class + message loop) with Direct2D/DirectWrite for rendering. This avoids WinUI/XAML dependencies and keeps the executable self-contained.

## Stage 1. Window Bootstrap
- Register `WNDCLASSEXW`, create the main window, and run the message loop.
- Handle core messages: `WM_CREATE`, `WM_SIZE`, `WM_DESTROY`, `WM_PAINT`.
- Set up Direct2D factory, render target, and DirectWrite text format in `WM_CREATE`.
- Exit Criteria: empty window rendering a background color/text.

## Stage 2. Rendering Backbone
- Load card assets directly from `assets/cards.png` + `assets/cards.json`.
- Implement double-buffered drawing (Direct2D swap chain or off-screen render target).
- Draw table background, player areas, and sample cards.
- Exit Criteria: static table view with cards positioned for all seats; window resizes gracefully.

## Stage 3. Interaction & Game Loop
- Map mouse clicks/movement and keyboard events to game actions.
- Wire the Hearts match controller to the rendering layer (state snapshots -> repaint on change).
- Implement passing overlay, trick animations (basic), and status text.
- Exit Criteria: full offline match playable with AI opponents.

## Stage 4. Polish & Accessibility
- Improve animations, transitions, and highlight states.
- Add settings overlay, theme switching, and high-DPI asset scaling.
- Integrate basic accessibility (keyboard navigation, readable text, optional UI automation hooks).
- Exit Criteria: feature-complete UI aligned with design spec.

## Stage 5. Packaging & QA
- Bundle assets with the executable (installer script / self-extract path).
- Add Win32 UI smoke tests (WinAppDriver or custom harness).
- Document build and packaging steps for release.
- Exit Criteria: signed installer/MSIX (if desired) and documented QA workflow.
