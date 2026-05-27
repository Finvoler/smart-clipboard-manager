# Smart Clipboard v0.1.8

## Windows Download

Asset: SmartClipboard-v0.1.8-windows-x64.zip

SHA256: `A511F79C513EEF44AD12B92DE26AD91FECC51F0BAC04FFBD6E12AC34108150E1`

Size: 6,348,267 bytes

## Highlights

- Fixes Xiaomi Token Plan API authentication for both OpenAI-compatible and Anthropic-compatible modes.
- Changes the default Xiaomi Base URL to `https://token-plan-cn.xiaomimimo.com/v1`.
- Changes the default Xiaomi Anthropic Base URL to `https://token-plan-cn.xiaomimimo.com/anthropic`.
- Migrates previously saved legacy Xiaomi URLs (`https://api.xiaomimimo.com/...`) to the new Token Plan endpoints on startup.

## Verified

- Local saved Xiaomi key succeeds against `https://token-plan-cn.xiaomimimo.com/v1/models` and fails against the legacy `https://api.xiaomimimo.com/v1/models`, confirming the root cause was the wrong Base URL rather than the key itself.
- Local saved Xiaomi key succeeds against `https://token-plan-cn.xiaomimimo.com/anthropic/v1/messages` and fails against the legacy `https://api.xiaomimimo.com/anthropic/v1/messages`.
- `npm run build` passed.
- `cargo test --manifest-path src-tauri/Cargo.toml` passed.
- `npm run tauri -- build` passed.

## Install

1. Download and extract SmartClipboard-v0.1.8-windows-x64.zip.
2. Run SmartClipboard.exe from a stable folder, for example `H:\Clipboard`.
3. If older Xiaomi URLs were previously saved, this version will migrate them automatically on startup.