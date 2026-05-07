# Version 0.2 Overview

Version 0.2 expands Rat Search from an application launcher into a local
file-and-folder launcher while preserving the v0.1 app workflow.

The core workflow is:

```text
Press hotkey -> search app/file/folder -> launch, open, reveal, or copy path
```

## User-Facing Behavior

- Empty query shows the compact search-only launcher.
- Typing a query expands the launcher and searches applications, files, and
  folders together.
- Application results launch with `Enter`.
- File and folder results open with `Enter`.
- File and folder results reveal in the file manager with `Ctrl+Enter`.
- File and folder paths copy with `Ctrl+C` when no search text is selected.
- `Esc` clears the current query or closes the launcher.
- Long names and paths truncate inside fixed-height rows.
- Result rows show source labels for `App`, `File`, and `Folder`.
- App icons resolve from Linux icon themes when possible.
- File and folder results use source-aware symbolic icons.

## Indexed Folders

Version 0.2 indexes only conservative user folders by default:

```text
Desktop
Documents
Downloads
Pictures
```

Missing folders are skipped quietly. Rat Search does not index the whole home
directory, hidden folders, system directories, or generated/heavy directories
such as `node_modules`, `.git`, `target`, `build`, `dist`, `.cache`, `tmp`, and
`temp`.

The index is in memory for v0.2. It is populated once at startup, and search
queries use the existing index rather than scanning the filesystem per
keystroke.

## Architecture

The frontend calls one unified Tauri command:

```text
search(query, limit) -> Vec<SearchResult>
```

Each result has a shared shape with an `id`, display text, icon, source, action,
optional path, score, and source-specific metadata. Application discovery,
file/folder indexing, search ranking, and path actions remain implemented in
Rust.

Supported result sources:

```text
applications
files
folders
```

Supported actions:

```text
launch_app
open_path
reveal_path
copy_path
```

File actions validate the requested path against the managed in-memory index
before opening, revealing, or copying it.

## Platform Notes

Rat Search targets Ubuntu/Linux first.

On X11, `Alt+Space` toggles the launcher through the app's global shortcut
registration. On Wayland, global shortcut registration is skipped by design;
bind your desktop shortcut to:

```bash
rat-search toggle
```

This keeps the same launcher workflow while respecting Wayland shortcut
constraints.

## Verification Status

Version 0.2 completed the automated release gate:

```bash
npm run check
/home/sanuk/.cargo/bin/cargo check --manifest-path src-tauri/Cargo.toml
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
/home/sanuk/.cargo/bin/cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

The Rust suite covers file index inclusion/exclusion, hidden/generated folder
skipping, file/folder ranking, mixed app/file/folder ordering, path action
validation, app icon resolution, result serialization, and app launch behavior.

A bounded startup smoke test confirmed that the app starts, discovers
applications, scans default file roots, and logs the expected Wayland global
shortcut caveat. Full hands-on UI verification should still be done in the
active desktop session before treating a build as release-ready.

## Not Included In Version 0.2

- Full document content indexing.
- Preview panels.
- OCR.
- Cloud-drive integration.
- Destructive file actions.
- File move/delete/rename actions.
- Settings UI.
- Plugin marketplace.
- AI or semantic search.
- Persistent SQLite indexing.
