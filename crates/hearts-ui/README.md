# hearts-ui

The UI crate exposes presentation metadata and shared configuration for the WinUI application.

## Asset integration notes
- Card and table textures are sourced from the mdsol Solitaire project (https://github.com/0x4D44/mdsol).
- Place processed assets under `assets/cards/` and `assets/themes/` in the root workspace; the crate will expose resource keys such as `cards/default`.
- A conversion script (TBD in `tools/`) will translate mdsol sprite sheets into WinUI-friendly PNGs at multiple DPI buckets.
- Resource keys intentionally decouple UI code from file locations so we can swap pipelines without changing code.
