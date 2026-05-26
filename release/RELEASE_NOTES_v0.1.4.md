# Smart Clipboard v0.1.4

## Windows Download

Asset: SmartClipboard-v0.1.4-windows-x64.zip

Main executable inside the zip:

- SmartClipboard.exe

SHA256 of release exe:

- 98D9F92F348BDEE899EF2A05B0453A1C5F10462BBAAA5DFFF9651B87E82F3485

SHA256 of release zip:

- 9A4E2FDAA1457BC996C16A8B5C4994E4D9A2E9E615D02905626409A464A68138

## Highlights

- Increases the vertical spacing above the `File save path` section in Settings.
- Adds a subtle divider before the local storage section so it no longer visually sticks to the API/model controls.
- Keeps the v0.1.3 ordering: `Save`, `Test`, `Models`, then `File save path`.
- Keeps custom data directory behavior and H-drive storage intact.

## Verified

- `npm run verify` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml` passed: 11 tests.
- `npm run tauri -- build` produced the v0.1.4 release exe.
- Launched `H:\Clipboard\SmartClipboard.exe` successfully.
- Verified the running Settings panel has a 46px gap from `Models` to the `File save path` title.
- Verified current bootstrap data directory remains `H:\Clipboard\data` with no pending migration.

## Install

1. Download and extract SmartClipboard-v0.1.4-windows-x64.zip.
2. Run SmartClipboard.exe from a stable folder, for example `H:\Clipboard`.
3. Use the tray icon to show the main window.
4. Configure API fields only if AI features or OCR are needed.
5. Optional: set File save path to a custom data directory such as `H:\Clipboard\data`.
6. Enable Start with Windows if startup is desired.
