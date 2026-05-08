# Version 0.3 Testing Guide

This guide explains how to test Rat Search v0.3 locally. It covers automated
checks that can run in any shell and manual checks that require an active Ubuntu
desktop session.

## Prerequisites

- Run from the repository root.
- Install Node dependencies before testing:

```bash
npm install
```

- Make sure Cargo is available. If needed:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

- Use an Ubuntu desktop session for manual launcher checks.
- Make sure `gnome-control-center` is installed before testing settings panels:

```bash
command -v gnome-control-center
```

## Automated Checks

Run the full gate:

```bash
npm run check
/home/sanuk/.cargo/bin/cargo check --manifest-path src-tauri/Cargo.toml
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
/home/sanuk/.cargo/bin/cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

Expected result:

- `npm run check` reports 0 errors and 0 warnings.
- `cargo check` finishes successfully.
- `cargo test` reports all Rust tests passing.
- `git diff --check` prints no whitespace errors.
- `cargo fmt --check` exits successfully.

Optional targeted Rust checks:

```bash
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml search_result
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml calculator
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml web_shortcuts
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml settings_search
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml search_history
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml mixed_search
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml record_search_history
```

## Start the App

Start Rat Search in development mode:

```bash
npm run tauri dev
```

Expected startup behavior:

- The native app starts and stays resident.
- The launcher window starts hidden.
- On X11, `Alt+Space` toggles the launcher.
- On Wayland, global shortcut registration is skipped by design. Bind your
  desktop shortcut to `rat-search toggle`, or launch/toggle the app through the
  available desktop workflow.
- The startup log should mention discovered applications, file index scanning,
  loaded search history count, and the Wayland shortcut caveat when applicable.

## Manual Launcher Checks

### Baseline

- Open the launcher.
- Confirm empty query stays compact.
- Type any query and confirm the launcher expands.
- Backspace to an empty query and confirm it collapses smoothly.
- Press `Esc` and confirm the query clears or the launcher closes.
- Use `Up`, `Down`, and `Tab` and confirm selection moves.
- Confirm source labels stay inside their column.
- Confirm long titles and subtitles truncate cleanly.

### Existing App, File, and Folder Behavior

- Search for an installed application and confirm it appears.
- Press `Enter` on an application result and confirm it launches.
- Search for a known file or folder after startup indexing finishes.
- Press `Enter` on a file or folder and confirm it opens.
- Press `Ctrl+Enter` on a file or folder and confirm its location is revealed.
- Press `Ctrl+C` on a file or folder when no input text is selected and confirm
  the path is copied.
- Select input text and press `Ctrl+C`; confirm normal text copy behavior is not
  replaced by path copying.

### Calculator

- Search `2+2`; confirm calculator result `4`.
- Press `Enter`; confirm `4` is copied and the launcher hides.
- Search `2 * (3 + 4)`; confirm result `14`.
- Search `2^8`; confirm result `256`.
- Search invalid syntax such as `2+*`; confirm no calculator result appears.

### Google Question Search

These checks open browser tabs or windows.

- Search `what is rust`; press `Enter`; confirm Google opens.
- Search `How does Tauri work`; press `Enter`; confirm Google opens.
- Search `rust tauri?`; press `Enter`; confirm Google opens.
- Search `g rust`, `yt lofi`, `gh tauri`, `maps colombo`, and `w rust`;
  confirm these old fixed prefixes no longer produce web results by themselves.
- Search `history notes`, `doing tasks`, and ordinary app/file terms; confirm
  web results do not appear unexpectedly.

### GNOME Settings

These checks open GNOME Settings panels.

- Search `wifi`; confirm Wi-Fi appears.
- Search `bluetooth`; confirm Bluetooth appears.
- Search `display`; confirm Displays appears.
- Search `keyboard`; confirm Keyboard appears.
- Search `sound`; confirm Sound appears.
- Press `Enter` on a settings result and confirm the expected settings panel
  opens.
- Search an unknown settings term and confirm no unrelated settings result is
  produced.

### Search History

History tests write normalized query text to the app data history file.

- Run a successful non-history action such as launching an app, opening a web
  question search, copying a calculator result, or opening a settings panel.
- Search part of that same query later and confirm a `Hist` result can appear
  below stronger live results.
- Press `Enter` on a history result and confirm the old query is restored in the
  input.
- Confirm selecting a history result does not immediately launch, open, or copy
  anything.

## Failure Notes

When a check fails, record:

- The exact query used.
- The selected result source label.
- The expected behavior and actual behavior.
- Whether the session is X11 or Wayland:

```bash
echo "$XDG_SESSION_TYPE"
```

- Relevant terminal logs from `npm run tauri dev`.

For browser/settings failures, include whether the external app opened at all.
For clipboard failures, include what was expected on the clipboard and what was
actually pasted.
