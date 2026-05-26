# Smart Clipboard v0.1.5

## Windows Download

Asset: SmartClipboard-v0.1.5-windows-x64.zip

Main executable inside the zip:

- SmartClipboard.exe

SHA256 of release exe:

- 0E36224686C730863B11F8A96B2FD5BD2D510BD2F34156622208D2819151C396

SHA256 of release zip:

- C253A0DB750F72CAB4D8E027D6A66D0004F634358E48DA567B027D9FE959B881

## Highlights

- Fixes the confusing data-directory workflow in Settings.
- Adds a dedicated `Save path and restart` button inside the `File save path` section.
- Shows a pending target when the typed or selected path differs from the active saved path.
- Clarifies that `Current active data directory` only changes after saving, confirming, migrating, and restarting.
- Keeps the local storage section visually separated from API/model settings.

## Verified

- `npm run verify` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml` passed: 11 tests.
- `npm run tauri -- build` produced the v0.1.5 release exe.
- Replaced and launched `H:\Clipboard\SmartClipboard.exe` successfully.
- In the running exe, entering `E:\SmartClipboardV015Test` showed the pending target and enabled the new path-apply button.
- Applying the path migrated the database to `E:\SmartClipboardV015Test`; a unique clipboard text was stored only in the E test database.
- Applying `H:\Clipboard\data` migrated the database back; a second unique clipboard text was stored only in the H database.
- The bootstrap file ended at `H:\Clipboard\data` with no pending migration.

## Install

1. Download and extract SmartClipboard-v0.1.5-windows-x64.zip.
2. Run SmartClipboard.exe from a stable folder, for example `H:\Clipboard`.
3. Use the tray icon to show the main window.
4. Configure API fields only if AI features or OCR are needed.
5. Optional: set File save path to a custom data directory and click `Save path and restart`.
6. Enable Start with Windows if startup is desired.
