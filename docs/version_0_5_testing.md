# Rat Search Version 0.5 Testing Guide

Use this guide to verify the Version 0.5 action defaults and the existing
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
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml file_search
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml file_actions
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml calculator
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml mixed_search
```

## Static Review

- Normal file/folder default open behavior is unchanged.
- `open <query>` strips the prefix and uses existing file/folder matching.
- `open <folder>` opens folders in Visual Studio Code.
- `open <code-like-file>` opens code-like files in Visual Studio Code.
- `open <non-code-file>` keeps the system opener.
- Missing or failing VS Code falls back to the system opener.
- `Ctrl+Enter` still reveals file/folder results.
- `Ctrl+C` still copies file/folder paths.
- Calculator default action opens the desktop calculator.
- Calculator expression prefill depends on installed calculator support.
- No YouTube, GitHub, terminal, `code`, `calc`, note, or user-defined quick keys
  were added.
- No arbitrary shell execution or destructive commands were added.
- Clipboard history remains opt-in.

## Manual Desktop Checks

Start the app:

```bash
npm run tauri dev
```

Baseline:

- Empty query stays compact.
- Typing expands the launcher.
- Backspacing to empty collapses.
- `Esc`, arrows, `Tab`, `Enter`, `Ctrl+Enter`, and `Ctrl+C` preserve Version
  0.4 behavior except for the intentional new default actions.
- App, file, folder, calculator, Google, settings, history, and clipboard checks
  still pass.

Open intent actions:

- Search normally for a file or folder without `open`; press `Enter` and confirm
  the existing default opener behavior remains unchanged.
- Search `open work_done.md`; confirm similar file suggestions appear.
- Select a code-like file from `open work_done.md`; press `Enter` and confirm it
  opens in Visual Studio Code.
- Search `open pc_work`; confirm similar folder suggestions appear.
- Select a folder from `open pc_work`; press `Enter` and confirm it opens in
  Visual Studio Code.
- Search for a PDF, image, video, audio, or archive file with `open`; press
  `Enter` and confirm it uses the system opener rather than VS Code.
- Select a file or folder and press `Ctrl+Enter`; confirm it reveals in the file
  manager.
- Select a file or folder and press `Ctrl+C`; confirm the path is copied.
- Test without VS Code on `PATH` and confirm `open <folder>` falls back to the
  system opener.

Calculator actions:

- Search `2+2`; confirm Rat Search still returns `4`.
- Press `Enter`; confirm the desktop calculator opens.
- If the installed calculator supports expression handoff, confirm the
  expression is passed into the app.
- If expression handoff is not supported, confirm the calculator opens and the
  expression or result fallback is copied.

Regression checks:

- Google question search still works.
- Clipboard enable, search, copy, delete, and clear still work.
- GNOME Settings search still works.
- Search history still records successful normal actions.
- On Wayland, global shortcut skip remains expected.
- On X11, `Alt+Space` still toggles the launcher.
