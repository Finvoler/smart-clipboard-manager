# Smart Clipboard v0.1.7

## Windows Download

Asset: SmartClipboard-v0.1.7-windows-x64.zip

SHA256: `31A5581DF9BF2D46326954CA5DF7868D4D4FEBF01131DC20CC2914FBB9A6DDCC`

Size: 6,347,559 bytes

Main executable inside the zip:

- SmartClipboard.exe

## Highlights

- Hardens app exit handling for Windows shutdown, logout, tray quit, and manual restart paths.
- Keeps the normal close-button behavior: clicking X still hides Smart Clipboard to the tray.
- Allows real app exits to proceed without treating them as tray-hide requests.
- Disables best-effort panic auto-restart once the app is already exiting, so shutdown does not accidentally spawn a restart helper.
- Keeps the v0.1.6 fixes for single-instance protection, safe restart path handling, and H-drive data-directory migration.

## Shutdown Warning Investigation

- Recent Windows Application logs did not show SmartClipboard crashes, PowerShell restart-helper failures, or path parsing errors.
- Recent System logs showed normal user-initiated restart/shutdown events.
- Current runtime process audit showed exactly one SmartClipboard process, launched from `H:\Clipboard\SmartClipboard.exe --startup`.
- The likely risk was code-level: the app previously intercepted every main-window close request to hide to tray, even though true exit paths should be allowed to continue.

## Verified

- `npm run build` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml` passed.
- `npm run tauri -- build` passed.
- The release package was created from the E-drive build output, without modifying the daily-use H-drive exe or H-drive data.

## Install

1. Download and extract SmartClipboard-v0.1.7-windows-x64.zip.
2. Run SmartClipboard.exe from a stable folder, for example `H:\Clipboard`.
3. Use the tray icon to show the main window.
4. Configure API fields only if AI features or OCR are needed.
5. Optional: set File save path to a custom data directory and click `Save path and restart`.
6. Enable Start with Windows if startup is desired.