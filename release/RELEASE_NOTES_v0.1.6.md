# Smart Clipboard v0.1.6

## Windows Download

Asset: SmartClipboard-v0.1.6-windows-x64.zip

Main executable inside the zip:

- SmartClipboard.exe

## Highlights

- Adds single-instance protection so launching the H-drive exe again focuses the existing app instead of creating a second tray/runtime process.
- Replaces the fragile Windows `cmd /c start` restart helper with hidden PowerShell `Start-Process`.
- Fixes the `Windows 找不到 '\\' 文件` failure when using `Save path and restart`.
- Allows `H:\Clipboard` to be both the install directory and the active data directory.
- Restricts data migration to `smart_clipboard.sqlite*` and `images\`, so switching paths never moves or deletes `SmartClipboard.exe`.

## Verified

- `npm run build` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml` passed: 16 tests.
- Actual deployed exe was smoke-tested at `H:\Clipboard\SmartClipboard.exe`.
- Real UI path migration succeeded in both directions:
  - `H:\Clipboard -> E:\SmartClipboardRestartSmoke`
  - `E:\SmartClipboardRestartSmoke -> H:\Clipboard`
- Final bootstrap state returned to `H:\Clipboard` with `pendingMigration = null`.
- Final runtime state kept exactly one running `SmartClipboard.exe` process from H drive.
- Startup audit confirmed only one startup entry exists: `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\Smart Clipboard Manager.lnk` -> `H:\Clipboard\SmartClipboard.exe --startup`.

## Install

1. Download and extract SmartClipboard-v0.1.6-windows-x64.zip.
2. Run SmartClipboard.exe from a stable folder, for example `H:\Clipboard`.
3. Use the tray icon to show the main window.
4. Configure API fields only if AI features or OCR are needed.
5. Optional: set File save path to a custom data directory and click `Save path and restart`.
6. Enable Start with Windows if startup is desired.