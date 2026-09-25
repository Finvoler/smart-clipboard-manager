# Smart Clipboard v0.3.0

This release fixes history rows that overlapped or left large gaps, and replaces paid image OCR with the Windows OCR engine already present on supported machines.

- Text previews stay within 220 px. **Expand all** loads the full record on demand; collapsing, resizing, scrolling, and new records keep the virtual list aligned.
- Image OCR runs locally when clicked, preserves line breaks, and saves the result on the image record. There is no OCR API key, model download, installer, certificate, or MSIX identity package. The legacy OCR model database column remains for rollback compatibility but is no longer used.
- Search now finds text beyond the short preview and ignores stale responses from earlier queries.
- Image previews load through Tauri's restricted local asset protocol instead of moving whole images through Base64 IPC; new clipboard events send previews rather than full text.
- The toolbar and cards have a lighter glass finish and short pointer feedback. Layout size is unchanged; reduced-motion and forced-colors preferences are supported.
- Release Rust optimization reduced the exe from roughly 16.1 MB to 11.7 MB. Background clipboard capture remains event-driven.
- Updated build dependencies with a clean `npm audit`; AI search and archive remain available and separate from OCR.

**Windows requirement:** Windows 10/11 x64 with WebView2 and the desired Windows OCR language capability installed in Settings > Time & language > Language options. Chinese screenshots need a Chinese OCR pack. Recognition is offline, but installing an optional language component may need internet access. A plain unpackaged exe successfully called `Windows.Media.Ocr` without a certificate on the development PC; other Windows configurations may differ.

**Validation:** 21 Rust tests including a real Chinese/English OCR fixture, frontend layout/race tests, and an isolated native end-to-end run covering clipboard capture, image preview, OCR persistence/search, editing, favorites, folders, paste clipboard formats, disabled capture, and OpenAI/Anthropic flows against a local test server. Automated input cannot prove Windows foreground restoration after a physical `Win+V`; that interaction should be checked manually on each machine.

Extract the zip before running `SmartClipboard.exe`. Settings and history stay in your configured data directory. Full setup and rollback notes are in README.md.
