Proposed Fixes
==============

- [x] Implement `FromStr` for `PassingDirection` and migrate existing call sites to use `str::parse` instead of the ad-hoc helper.
- [x] Collapse the nested `if` chains Clippy flagged in `model::round::apply_card` and `model::score::apply_hand` so linted builds succeed.
- [x] Ensure the first trick leader follows the current Two of Clubs holder after passes (added regression test in round core logic).
- [x] Update `WM_DPICHANGED` handling to read both X and Y DPI values (HIWORD/LOWORD) and propagate the correct scale across the UI.
- [x] Deduplicate the DPI layout math by introducing a shared helper (e.g., `layout_size_for`) used by both `AppState` and `AboutDialogState`.
- [x] Fix mouse hit-testing during the passing phase by converting cursor coordinates into DIPs using the same scale as the layout rectangles.
- [x] Optional polish: avoid rebuilding static hint strings each frame (use a borrowed string or `Cow` where possible).
