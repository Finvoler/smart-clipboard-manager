# Smart Clipboard v0.1.3

## Windows Download

Asset: SmartClipboard-v0.1.3-windows-x64.zip

Main executable inside the zip:

- SmartClipboard.exe

SHA256 of release exe:

- A2356420973B35F0C487DB6226CC970EC377D6CA1D6C4D86B4B4C6A8D00D042B

SHA256 of release zip:

- 1B49E398DABE79605A8934E6AA11CC07D8CBB8EA8B15919AF307901EF20F2EAF

## Highlights

- Moves the `File save path` setting to the end of the Settings panel, after the `Save`, `Test`, and `Models` controls.
- Keeps API/model configuration visually grouped separately from local storage configuration.
- Keeps the v0.1.2 custom data directory behavior: folder picker, manual path entry, restart-time migration, and H-drive data storage.
- Keeps the release startup behavior hidden and the packaged frontend embedded.

## Verified

- `npm run verify` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml` passed: 11 tests.
- `npm run tauri -- build` produced the v0.1.3 release exe.
- Launched `H:\Clipboard\SmartClipboard.exe` successfully.
- Verified Settings order in the running exe: `Save`, `Test`, `Models`, then `File save path`.
- Verified current bootstrap data directory remains `H:\Clipboard\data` with no pending migration.

## Install

1. Download and extract SmartClipboard-v0.1.3-windows-x64.zip.
2. Run SmartClipboard.exe from a stable folder, for example `H:\Clipboard`.
3. Use the tray icon to show the main window.
4. Configure API fields only if AI features or OCR are needed.
5. Optional: set File save path to a custom data directory such as `H:\Clipboard\data`.
6. Enable Start with Windows if startup is desired.
