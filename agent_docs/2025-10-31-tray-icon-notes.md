ABOUTME: tray icon research log for rumble tauri app.
ABOUTME: captures decisions and follow-up questions.

# Tray Icon Update Notes

- 2025-02-14 18:48 PT — Observed macOS tray still shows default Tauri glyph; suspect we never instantiate a tray icon with custom artwork.
- 2025-02-14 18:52 PT — Tauri docs confirm using `TrayIconBuilder` requires enabling the `tray-icon` feature plus `image-png` when loading PNGs at runtime.
- 2025-02-14 19:02 PT — Added setup hook that builds a tray icon from the packaged `icons/icon.png` asset and stores the handle via `manage` so it persists.
- 2025-02-14 19:11 PT — Had to convert the default icon to an owned image (`to_owned`) to satisfy `'static` lifetime requirements when returning it from helper.
- 2025-02-14 19:28 PT — Imported brand `Elephant-Logo.svg`, rasterized to padded 1024×1024 PNG via `resvg`, and regenerated platform icon set so tray matches marketing asset.
- 2025-02-14 19:40 PT — Embedded `icons/icon.png` bytes directly with `include_bytes!` so dev builds stop falling back to the default Tauri tray asset.
