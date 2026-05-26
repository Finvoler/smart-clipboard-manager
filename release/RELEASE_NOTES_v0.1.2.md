# Smart Clipboard v0.1.2

## Windows Download

Asset: SmartClipboard-v0.1.2-windows-x64.zip

Main executable inside the zip:

- SmartClipboard.exe

SHA256 of release exe:

- EC8017CCFD8B38CE41224328891DFBC86AF0C9DCEA364DB404147DC4A8472A03

SHA256 of release zip:

- EA584002868B9551BF8FEB568D340DBCE5D7547C3CBCA8F2E2D7552C7FFA1E8F

## Highlights

- Adds a custom file save path setting for the local SQLite database, image cache, and future data files.
- Supports both `Choose folder` and manual path entry in Settings.
- Migrates existing data on restart before SQLite opens, using a small AppData bootstrap file.
- Fixes the Tauri dialog permission so the native folder picker opens in packaged release builds.
- Makes data-directory migration more robust: BOM-tolerant bootstrap parsing and idempotent pending migration cleanup.
- Keeps startup/restart behavior hidden so release builds do not open a stray console window.
- Keeps the packaged release loading embedded frontend assets instead of trying to connect to the dev server.

## Verified Data Directory Tests

- Ran the final release exe from `H:\Clipboard\SmartClipboard.exe`.
- Switched the data path to `D:\ClipboardPathTest`, restarted, and confirmed `storage-bootstrap.json` pointed to that path.
- Copied `D_DRIVE_CLIPBOARD_TEST_20260526_1114` and confirmed it was stored in `D:\ClipboardPathTest\smart_clipboard.sqlite` only.
- Switched back to `H:\Clipboard\data`, restarted, and confirmed `storage-bootstrap.json` pointed back to `H:\Clipboard\data`.
- Copied `H_RETURN_CLIPBOARD_TEST_20260526_1115` and confirmed it was stored in `H:\Clipboard\data\smart_clipboard.sqlite` only.
- The default AppData SQLite database did not receive either test record.

The test machine did not have a physical D: drive, so D: was temporarily created with Windows `subst` and removed after the test.

## Install

1. Download and extract SmartClipboard-v0.1.2-windows-x64.zip.
2. Run SmartClipboard.exe from a stable folder, for example `H:\Clipboard`.
3. Use the tray icon to show the main window.
4. In Settings, configure API fields only if AI features or OCR are needed.
5. Optional: set File save path to a custom data directory such as `H:\Clipboard\data`.
6. Enable Start with Windows if startup is desired.

## Security Software

If startup is blocked, add the extracted SmartClipboard.exe and this startup shortcut to the trust list:

%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\Smart Clipboard Manager.lnk
