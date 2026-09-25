# Smart Clipboard v0.3.2

This Windows x64 release improves the history layout and settings flow, and fixes rapid repeated-copy detection.

- Long text records use a consistent compact preview with a subtle fade and clear expand/collapse controls. Image OCR text has its own compact control; expanded OCR places the image above full-width text and keeps image preview and collapse available.
- Notifications, settings controls, folder creation, folder counts, and model choices use the updated glass interface. Folder counts continue to reflect all records while search results are filtered.
- The clipboard capture, startup, and console switches save immediately. The API connection test saves only the entered API configuration before testing; loading model choices does not save settings. Closing and reopening the panel discards unsaved text and path edits.
- A manually entered data directory must already exist. An invalid or missing directory reports failure in the path button before migration or restart; the app will not create a new folder from a typo.
- The quick-suggestion threshold remains five copies of the same normalized text within 24 hours. Clipboard updates now schedule a short capture from the first update instead of repeatedly resetting a 200 ms timer, reducing missed rapid copies.
- AI semantic search checks all saved history records in bounded batches, including complete text from long records. AI archive considers up to 300 recent uncategorized records per run, also in bounded batches.

These AI limits are independent of the virtualized history display. Searching a large history can make multiple paid model requests and take longer than before. Image OCR remains local through installed Windows OCR language packs and needs no API key.

The zip contains `SmartClipboard.exe`, `README.md`, and these notes. It does not contain personal clipboard data or API keys.

Validation: frontend build and project gates, UI regression tests, Rust unit tests, and a Windows x64 release build.
