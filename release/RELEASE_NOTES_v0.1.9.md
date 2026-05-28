# Smart Clipboard v0.1.9

## Windows Download

Asset: SmartClipboard-v0.1.9-windows-x64.zip

SHA256: `7431F53C1337C9C572A3615758F3985FB907E9F74986C5DE87D9A8B3728D2A98`

Size: 6,349,394 bytes

## Highlights

- Fixes the false “history truncation” behavior in the UI by loading the full month of retained records instead of only the newest 120 entries.
- Restores older starred records and foldered records in the sidebar when they still exist inside the SQLite database.
- Removes the sidebar’s extra record slicing so starred items and folder contents no longer disappear just because newer history entries were captured.
- Keeps Xiaomi default API endpoints on the normal production domain: `https://api.xiaomimimo.com/v1` and `https://api.xiaomimimo.com/anthropic`.
- Keeps the actual retention rule unchanged: non-starred records still expire after 30 days, and quick pool expiration behavior is unchanged.

## Verified

- Existing local database still contains older records beyond the newest 120 entries, including starred and foldered rows, so the data can become visible again after this fix.
- Verified on the current live database that the old 120-row UI window would show `0` starred and `0` foldered records, while the new full-month window shows `1` starred and `4` foldered records.
- `cargo test --manifest-path src-tauri/Cargo.toml` passed, including a new regression test for keeping older starred and foldered records visible when history exceeds 120 rows.
- `npm run build` passed.

## Install

1. Download and extract SmartClipboard-v0.1.9-windows-x64.zip.
2. Replace the existing SmartClipboard.exe in your stable app folder, for example `H:\Clipboard`.
3. Start the app normally. Older starred and foldered records that still exist in the local database will reappear automatically.