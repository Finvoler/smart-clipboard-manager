# Smart Clipboard v0.1.1

## Windows Download

Asset: SmartClipboard-v0.1.1-windows-x64.zip

Main executable inside the zip:

- smart_clipboard.exe

SHA256 of release exe:

- E91BABCB5936E09346DAE87AF22FF99F702EAAE7DA28966B7044265D4D34ADC1

SHA256 of release zip:

- 7D53846CF6806D4A35B05DA671518E2DFF34BF2B53A8473594981E72AA0DE1F4

## Highlights

- Fixes the startup cmd / terminal window issue by using a hidden startup shortcut and no-window restart process creation.
- Removes the legacy registry Run fallback and keeps startup managed through the current-user Startup folder shortcut only.
- Fixes the packaged release app so it loads the embedded frontend instead of trying to connect to the dev server at 127.0.0.1:1420.
- Local clipboard history for text and images.
- Smart Win+V panel with tray toggle back to native Windows Win+V.
- Native paste behavior with focus restore and Ctrl+V dispatch.
- Text editing, star, delete, folders, Markdown and math rendering.
- Repeated text quick pool with temporary retention.
- Image deduplication, preview, manual OCR, and separate OCR text paste.
- AI search, AI archive, API settings, model discovery, OpenAI-compatible and Anthropic-compatible modes.
- Chinese and English UI.

## Install

1. Download and extract SmartClipboard-v0.1.1-windows-x64.zip.
2. Run smart_clipboard.exe.
3. Use the tray icon to show the main window.
4. Configure API settings only if AI features or OCR are needed.
5. Enable Start with Windows if startup is desired.

## Security Software

If startup is blocked, add the extracted smart_clipboard.exe and this startup shortcut to the trust list:

%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\Smart Clipboard Manager.lnk
