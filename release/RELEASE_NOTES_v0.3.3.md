# Smart Clipboard v0.3.3

This Windows x64 release changes history retention for saved records.

- Records in a folder or marked as favorites no longer expire after 30 days.
- Removing a record from a folder starts a fresh 30-day countdown unless it is still a favorite. Removing a favorite starts the countdown unless it remains in a folder. Deleting a folder gives its non-favorite records a fresh 30 days.
- Temporary-pool entries keep their own expiry. When a temporary entry expires, its history record remains. When the source history record expires or is deleted first, its linked temporary entry and pending suggestion are removed too.
- Existing foldered and favorite records with old expiry timestamps are normalized on startup. Existing temporary entries are linked to matching history records where possible.
- Closing the panel without pasting preserves the current scroll position and expanded records. After a successful paste, the next open starts at the top with text, OCR, and image previews collapsed.

The zip contains `SmartClipboard.exe`, `README.md`, and these notes. It does not contain clipboard history, image caches, databases, or API keys. Extract the zip before running the app; existing data remains in the configured data directory.

Validation: Rust unit tests, frontend build, Windows x64 release build, and local startup with existing data.
