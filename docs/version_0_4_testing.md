# Rat Search Version 0.4 Testing Guide

Use this guide to verify the v0.4 clipboard history feature and the existing
launcher baseline.

## Automated Gate

Run from the repository root:

```bash
npm run check
/home/sanuk/.cargo/bin/cargo check --manifest-path src-tauri/Cargo.toml
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
/home/sanuk/.cargo/bin/cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Targeted Rust filters:

```bash
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml search_result
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml clipboard_settings
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml clipboard_history
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml clipboard_privacy
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml clipboard_search
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml mixed_search
```

## Static Review

- Clipboard history is disabled by default.
- Clipboard text, previews, normalized text, and item IDs are not logged.
- Clipboard result metadata does not include full unbounded clipboard text.
- Clipboard commands validate item IDs.
- Clear history and disable tracking are separate actions.
- No image, file-list, rich text, or HTML clipboard formats are stored.
- Existing v0.3 commands remain registered.

## Manual Desktop Checks

Start the app:

```bash
npm run tauri dev
```

Baseline:

- Empty query stays compact.
- Typing expands the launcher.
- Backspacing to empty collapses.
- `Esc`, arrows, `Tab`, `Enter`, `Ctrl+Enter`, and `Ctrl+C` preserve v0.3
  behavior.
- App, file, folder, calculator, Google, settings, and history checks still
  pass.

Clipboard disabled:

- Confirm clipboard history starts disabled.
- Copy text in another app.
- Search for that text and confirm no clipboard result appears.

Clipboard enabled:

- Open clipboard privacy controls.
- Enable clipboard history and confirm the warning appears before enabling.
- Copy `rat search clipboard test one`.
- Wait for the polling interval.
- Search `clipboard test` and confirm a `Clip` result appears.
- Press `Enter`, then paste into a text editor and confirm the old text was
  copied.

Clipboard management:

- Copy the same text twice and confirm only one newest clipboard entry appears.
- Delete one clipboard entry and confirm it no longer appears.
- Clear clipboard history and confirm previous clipboard snippets no longer
  appear.
- Disable clipboard history, copy new text, and confirm new text is not
  recorded.

Privacy filters:

- Copy text containing `password=example` and confirm it is not recorded.
- Copy a fake private key block and confirm it is not recorded.
- Check terminal logs and confirm sensitive text is not printed.
