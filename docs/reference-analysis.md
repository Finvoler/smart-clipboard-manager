# Reference Analysis and Architecture Notes

This project was started after pulling and inspecting the required references under `_reference/`.

## PasteBar/PasteBarApp

Relevant observations:

- Tauri keeps platform integration in Rust and exposes small command handlers to React.
- Clipboard changes are pushed to the UI through backend events instead of frontend polling.
- SQLite is used as local durable storage, with history rows separated from user-created organization records.
- Image paths should be stored as stable local paths instead of embedding large payloads in every IPC response.

Applied decisions:

- Use Tauri commands for the requested IPC surface and Tauri events for `on_new_item` and `on_quick_pool_extracted`.
- Keep Rust as the single writer to SQLite.
- Keep the frontend state thin: it reloads history/folders/quick pool and accepts push events.

## sabrogden/Ditto

Relevant observations:

- Clipboard monitoring on Windows uses `AddClipboardFormatListener` where available and older viewer-chain APIs as fallback.
- Paste is performed by temporarily putting the selected data on the system clipboard and simulating paste input.
- Ditto marks internally generated clipboard writes with an ignore format to avoid re-capturing its own paste operation.
- SendKeys logic releases modifier keys before sending paste, reducing failures when a global shortcut was just pressed.

Applied decisions:

- Use native Windows APIs for `Win+V` interception and clipboard update notifications.
- Keep an `ignore_next_clipboard` flag in app state while executing paste.
- Set text to the system clipboard, hide the window, then send `Ctrl+V` through `enigo`.

## hluk/CopyQ

Relevant observations:

- Clipboard data is separated by MIME/format, with text, HTML, URI, and image formats handled independently.
- Preferred editable text format is UTF-8 plain text with plain text fallback.
- Binary and image payloads need size limits and should not be blindly duplicated into every view model.

Applied decisions:

- Model `items.kind` as `text` or `image`, with `content` for editable text and `image_path` for image records.
- Store folder and starred state independently from raw capture data so retention policy can whitelist records.
- Keep image preview rendering lazy on the frontend; Rust exposes stored paths as metadata.

## Dependency Choices

- `rusqlite`: smaller and simpler than Diesel/sqlx for a single local SQLite database, no async runtime required for CRUD.
- `windows`: used for low-level Windows hotkey interception, clipboard update listener, and window focus restoration hooks.
- `arboard`: cross-platform clipboard read/write helper for text and image payloads.
- `enigo`: pragmatic paste keystroke simulation. Native `SendInput` can replace it later if more control is needed.
- React renderer stack: `react-markdown`, `remark-gfm`, `remark-math`, `rehype-katex`, `rehype-highlight`.

## Safety Rules Preserved

- The app never hides on blur. It hides only through `hide_window`, `execute_paste`, or the Escape key.
- LLM functions are command-only stubs until the user provides a provider/key. There is no timer, watcher, or background polling path for AI.
- Retention cleanup physically deletes only non-starred records outside folders older than 30 days.
