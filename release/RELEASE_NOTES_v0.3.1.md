# Smart Clipboard v0.3.1

This update refines the history preview and control appearance while keeping the v0.3.0 layout and Windows OCR behavior.

- A short text record no longer shows an unnecessary expand control. Long records show a soft gray fade with a centered ellipsis button; expanded content can be collapsed below the record.
- OCR text uses the same treatment only when its four-line preview actually overflows. Line breaks remain visible in the preview.
- Controls now have a translucent material with a brighter edge, reflected highlight following the pointer, and short hover/press feedback. The toolbar and sidebar use shared glass surfaces; record text stays clear and readable.
- Reduced-motion, reduced-transparency, and forced-colors preferences continue to work.

Windows OCR still uses installed system language packs and needs no OCR API key, certificate, or bundled model. See README.md for requirements.

Validation: frontend build and UI tests covering short/long text and OCR, expand/collapse, window resize, virtual scrolling, search races, pointer highlight, and reduced-motion mode. The unchanged Rust backend is included in the same self-contained exe.
