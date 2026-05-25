# Smart Clipboard v0.1.0

## Windows Download

Asset: SmartClipboard-v0.1.0-windows-x64.zip

Main executable inside the zip:

- smart_clipboard.exe

SHA256 of release exe:

- EF90E3FDFD70DEF0388A3CD85DC16CF67E6E2C34780FD84AC1AA5A9F27E352DF

## Highlights

- Local clipboard history for text and images.
- Smart Win+V panel with tray toggle back to native Windows Win+V.
- Native paste behavior with focus restore and Ctrl+V dispatch.
- Text editing, star, delete, folders, Markdown and math rendering.
- Repeated text quick pool with temporary retention.
- Image deduplication, preview, manual OCR, and separate OCR text paste.
- AI search, AI archive, API settings, model discovery, OpenAI-compatible and Anthropic-compatible modes.
- Chinese and English UI.
- Tray controls for show, Win+V mode, restart, and quit.
- Startup shortcut support for current user startup folder, with registry fallback.
- Release executable is built as Windows GUI subsystem, so it should not show a cmd window.
- Startup shortcut uses hidden `--startup` mode and a minimized launch style so login does not show the main app window.

## Install

1. Download and extract SmartClipboard-v0.1.0-windows-x64.zip.
2. Run smart_clipboard.exe.
3. Use the tray icon to show the main window.
4. Configure API settings only if AI features or OCR are needed.
5. Enable Start with Windows if startup is desired.

## Security Software

If startup is blocked, add the extracted smart_clipboard.exe and this startup shortcut to the trust list:

%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\Smart Clipboard Manager.lnk

## GitHub Release Description

Copy the Highlights and Install sections above into the GitHub release page, then upload SmartClipboard-v0.1.0-windows-x64.zip as the release asset.
