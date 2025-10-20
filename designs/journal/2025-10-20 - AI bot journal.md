# AI Bot Dev Journal\n\nDate: 2025-10-20\n\n- Reviewed Stage 2 wrap-up plan and repo status.\n- Plan: add moon state machine, void-aware follow-ups, and decision logging.\n
- Implemented UnseenTracker extensions: per-seat suit voids and moon state.\n- Wired controller: marks voids on fail-to-follow; basic moon commit/abort updates after tricks.\n- Re-exported MoonState and integrated style persistence in determine_style.\n- Simulation follow-ups now accept tracker (scaffolding for void-aware responses).\n- Added debug candidate scoring logs in PlayPlanner gated by MDH_DEBUG_LOGS.\n
- Added debug logs: per-candidate pass/play scoring when MDH_DEBUG_LOGS=1.\n
- Improved follow-up simulation: when void, prefer dumping hearts if broken, then QS, else max-penalty; consult tracker to skip hearts when hearts-void is known.\n
- Next: add seeded golden tests and tune weights once basic behaviors stabilize.\n
- Added endgame nuance: broader feed-to-leader near 100, stronger avoid-capture when our score is high; uses remaining card count as small factor.\n
- Added hearts-app lib target and golden seeded test for 2? enforcement without building GUI bin; fixed Windows registry API signatures in win32.rs and borrow error in controller to allow tests. All tests passing for lib/tests.\n
- Added second golden test for endgame feeding to leader near 100 using direct BotContext and constructed RoundState. All golden tests green.\n
- Improved follow-up simulation to avoid self-dumps by provisional winner check; added helper to compute provisional winner. All tests still green.\n
- Added two unit tests for follow-up simulation: (1) void-aware dump prefers hearts when broken; (2) avoid self-dump when our seat is provisional winner. Full workspace tests are green.\n
