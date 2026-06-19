# Smart Clipboard v0.2.0

## Performance Optimization Release

This release brings significant performance improvements for users with large clipboard histories (hundreds to thousands of records).

### What's New

- **Virtual scrolling**: History list now uses virtual scrolling (TanStack Virtual), rendering only visible items instead of all records. Smooth 60fps scrolling even with thousands of items.
- **Light history queries**: List view loads without full content fields, reducing JSON serialization overhead and memory usage.
- **Clipboard monitoring debounce**: 200ms debounce on clipboard change events, preventing rapid-fire database writes and UI updates.
- **Search debounce**: 250ms debounce on search input, reducing unnecessary IPC calls.
- **Paste latency optimization**: Replaced fixed 140ms sleep with active window focus polling (typically completes in ~50ms).
- **SQLite optimizations**: Added `synchronous=NORMAL`, `cache_size=20MB`, `temp_store=MEMORY`, `mmap_size=256MB` for faster database operations.
- **Image preview fallback**: Images load via Tauri asset protocol first, with automatic fallback to base64 IPC if needed.
- **Improved JSON parsing**: AI response parsing now uses multi-strategy extraction with retry logic for malformed responses.
- **Thinking model support**: Better handling of AI models that use reasoning tokens (e.g. mimo-v2.5-pro).

### Bug Fixes

- Fixed AI search showing truncated content when results come from light queries.
- Fixed new clipboard items leaking into AI search results.
- Fixed image display regression from v0.1.9.
- Added `get_items_by_ids` batch command for efficient AI result fetching.
- Added `get_item` command for on-demand content loading.
- Improved `filteredItems` to include content in client-side filtering when available.

### API Configuration

Default AI model changed to `mimo-v2-flash` (non-reasoning, faster responses). Search model `max_completion_tokens` increased from 1200 to 8000.

### Breaking Changes

None. Database schema is backward-compatible with v0.1.9.
