# Smart Clipboard

Windows intelligent clipboard manager built with Tauri, React, TypeScript, Rust, and SQLite.

Learning-oriented repo notes live in `docs/project-retrospective-and-structure.md`.

## Features

- Clipboard history for text and images, stored locally in SQLite.
- Smart `Win+V` mode: intercepts `Win+V` and opens this app's panel; tray menu can switch back to native Windows `Win+V`.
- Native-feeling paste flow: restores focus to the previous window, writes text or image back to clipboard, then sends paste.
- Text history search plus AI semantic search.
- Local folders, starred records, delete, edit text records, and Markdown/math rendering.
- Quick pool for repeated text snippets, with temporary retention and star-to-history promotion.
- Image history with screenshot deduplication, inline preview, manual OCR, and OCR text stored on the image record.
- OCR text paste is separate: clicking the OCR text panel pastes text; clicking the rest of the image record pastes the image.
- AI settings panel for OpenAI-compatible or Anthropic-compatible search and archive. Image OCR uses Windows locally.
- Chinese/English UI language setting.
- Custom data directory setting for the SQLite database, image cache, and future local data files, with restart-time migration.
- Single-instance protection so duplicate launches focus the existing window instead of creating another tray/runtime process.
- System tray controls: show app, switch native/software `Win+V`, restart app, quit.
- Current-user startup support through the Windows Startup folder shortcut; legacy registry Run entries are cleaned up automatically.
- Startup launches with a hidden `--startup` mode, hidden shortcut show command, and no-window restart behavior so login does not open the main app window or a console.
- Best-effort auto restart on Rust panic, disabled once the app is already exiting.

## Install And Run

1. Download `SmartClipboard-v0.3.3-windows-x64.zip` from GitHub Releases.
2. Extract the zip to a stable folder, for example `H:\Clipboard` or `D:\Apps\SmartClipboard`.
3. Run `SmartClipboard.exe`.
4. Open the tray icon and choose `Show Smart Clipboard`.
5. In Settings, configure API fields if AI search or AI archive is needed. OCR uses the local Windows language pack and does not need an API key.

Do not run the exe directly from inside the zip file. Extract it first so startup shortcuts can point to a stable path.

## Windows OCR requirements

The portable exe calls `Windows.Media.Ocr` directly. This release does **not** install a certificate, register MSIX, or bundle models. The ordinary unpackaged executable was verified on the development Windows machine; this is not a guarantee for stripped-down Windows images or every OS build.

- Windows 10/11 x64 with Microsoft Edge WebView2 Runtime.
- Install the desired OCR language capability in **Windows Settings > Time & language > Language options > Optical character recognition**. A missing Chinese pack cannot be replaced by an English pack for Chinese screenshots.
- If no recognizer is installed, the OCR button reports a missing language pack instead of calling a paid service. Installing Windows optional components may require administrator rights/network access; recognition itself is offline.
- OCR is serialized and created only on demand; no polling, model download, or persistent engine. Results preserve line breaks and are stored as `ocr_text` on the existing image. Very large images are resized to the OS limit and a 16-megapixel bitmap budget; tiny text on huge screenshots may lose accuracy.
- The legacy SQLite `ocr_model` column remains only so an older exe can still read the database during rollback. It is absent from settings IPC and UI and is never used for OCR.

API documentation: [Windows OcrEngine](https://learn.microsoft.com/en-us/uwp/api/windows.media.ocr.ocrengine), [Microsoft PowerToys OCR implementation](https://github.com/microsoft/PowerToys/blob/main/src/modules/AdvancedPaste/AdvancedPaste/Helpers/OcrHelpers.cs).

## Recommended Settings

- `Record clipboard history`: keep on for normal clipboard capture.
- `Start with Windows`: keep on if the app should run after login.
- `Hide console window`: kept for compatibility; release builds are compiled as a Windows GUI app, and startup shortcuts use hidden `--startup` mode.
- These three switches save immediately. API fields and other editable settings use the `Save` button; `Test` saves the entered API fields before checking the connection.
- `Language`: choose Chinese or English UI.
- `Protocol`: choose OpenAI compatible or Anthropic compatible.
- `OpenAI base URL`: default is `https://api.xiaomimimo.com/v1`.
- `Anthropic base URL`: default is `https://api.xiaomimimo.com/anthropic`.
- `API key`: paste your provider key locally.
- `Search / archive model`: model used by AI search and AI archive.
- AI semantic search checks the full saved history in bounded requests; AI archive processes up to 300 recent uncategorized records per run. Large histories can require multiple paid model requests and take longer to finish. Ordinary local search does not call the model.
- Ordinary history expires after 30 days, regardless of record count; copying the same item again refreshes its retention time. Starred and foldered records are exempt. Removing a record from a folder or un-starring it starts a fresh 30-day period unless it remains protected by the other condition. Temporary-pool TTL only removes the temporary entry; if its source history record expires first, the temporary entry is removed with it.
- `Image OCR`: runs locally on demand using installed Windows OCR language packs. Prefers Chinese (also reads Latin letters), then the user's Windows language, then another installed recognizer. No API key or bundled OCR engine is used.
- `File save path`: optional custom directory for the local database, image cache, and later data files. Choose or type an **existing folder**, then click `Save path and restart` and confirm the destination. Invalid or nonexistent paths fail in the button without creating a folder or restarting. Unsaved edits are discarded when the panel is closed and reopened. `Current active data directory` only changes after a successful restart.

## Huorong / Security Software Notes

If startup is blocked by security software, add these to the trust list:

- The extracted release exe, for example `D:\Apps\SmartClipboard\smart_clipboard.exe`.
- The startup shortcut at `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\Smart Clipboard Manager.lnk`.

You can open the startup folder with `Win + R`, then enter `shell:startup`.

The startup shortcut should point to `smart_clipboard.exe` and include the `--startup` argument. If you move the app to a new folder, run it once and toggle `Start with Windows` off and on to refresh the shortcut.

For the current machine state verified in this repo session:

- The only startup entry is `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\Smart Clipboard Manager.lnk`.
- It points to `H:\Clipboard\SmartClipboard.exe` with `--startup`.
- There are no Smart Clipboard registry `Run` entries and no Smart Clipboard scheduled tasks.

## Local Data

By default, app data is stored under the Tauri app data directory for identifier `com.local.smartclipboard`, usually:

`%APPDATA%\com.local.smartclipboard\`

Important files include:

- `smart_clipboard.sqlite`: local settings, history metadata, folders, quick pool.
- `images\`: image clipboard history files.
- `storage-bootstrap.json`: a tiny bootstrap file kept in `%APPDATA%\com.local.smartclipboard\` so the app can find a custom data directory before opening SQLite.

If `File save path` is set in Settings, `smart_clipboard.sqlite`, `images\`, and future local data files are moved to that custom directory. For a portable-style local setup, this project has been tested with:

`H:\Clipboard\SmartClipboard.exe`

`H:\Clipboard\smart_clipboard.sqlite`

This same-directory layout is now supported intentionally. Migration only copies or removes `smart_clipboard.sqlite*` and `images\`, so the exe can stay in `H:\Clipboard` safely while data is switched away and back.

The app still keeps `storage-bootstrap.json` in AppData because the database path must be known before the database can be opened.

API keys are stored locally in this SQLite database. Do not upload your personal database to GitHub.

## Development

Install dependencies:

```powershell
npm install
```

Run in development:

```powershell
npm run tauri:dev
```

Verify before release:

```powershell
npm run verify
npm run test:ui # with npm run dev running in another terminal
cd src-tauri
cargo test
```

Build release exe:

```powershell
npm run tauri -- build
```

For Rust-only `cargo test` runs, `src-tauri/build.rs` will auto-create a tiny placeholder `dist/` directory if it is missing. Real app UI assets still come from `npm run build`.

The release exe is generated at:

`src-tauri\target\release\smart_clipboard.exe`

## GitHub Upload

### First-time repository upload

```powershell
git init
git add README.md package.json package-lock.json tsconfig.json vite.config.ts index.html src src-tauri docs scripts
git commit -m "Release Smart Clipboard v0.1.9"
git branch -M main
git remote add origin https://github.com/<your-name>/<repo-name>.git
git push -u origin main
```

### Publish a release asset

1. Build the release exe with `npm run tauri -- build`.
2. Create a zip containing `SmartClipboard.exe`, this README, and the release notes.
3. On GitHub, open the repository, go to `Releases`, choose `Draft a new release`.
4. Tag version: `v0.3.3`.
5. Upload `SmartClipboard-v0.3.3-windows-x64.zip`.
6. Paste the feature list and install notes into the release description.

Avoid uploading these folders or files:

- `node_modules/`
- `dist/`
- `src-tauri/target/`
- `.env` or `.env.*`
- local SQLite databases or image history
