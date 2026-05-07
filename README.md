# Rat Search

A Spotlight-inspired command palette for Ubuntu Linux, built with Tauri, Rust,
and Svelte.

Rat Search starts as a resident desktop utility and opens a compact launcher on
demand. Version 0.2 searches installed applications plus files and folders from
safe user folders.

## Features

- Search and launch installed Linux desktop applications.
- Search files and folders by name from `Desktop`, `Documents`, `Downloads`,
  and `Pictures`.
- Open file and folder results with `Enter`.
- Reveal file and folder results with `Ctrl+Enter`.
- Copy file and folder paths with `Ctrl+C` when no search text is selected.
- Render app icons from the local icon theme when available.
- Show source-aware app, file, and folder icons.
- Keep search responsive by scanning files once at startup and searching an
  in-memory index.

## Keyboard

| Shortcut | Behavior |
| --- | --- |
| `Alt+Space` | Toggle the launcher on X11 |
| `Up` / `Down` | Move selection |
| `Tab` | Cycle selection |
| `Enter` | Launch the app or open the selected file/folder |
| `Ctrl+Enter` | Reveal the selected file/folder |
| `Ctrl+C` | Copy the selected file/folder path if no input text is selected |
| `Esc` | Clear or close the launcher |

On Wayland, global shortcut registration is skipped by design. Bind your
desktop shortcut to `rat-search toggle` for the same launcher workflow.

## Local Development

Run the Tauri development app:

```bash
npm run tauri dev
```

If the Tauri CLI cannot find Cargo, add Cargo to `PATH` first:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

The app starts as a resident utility with the launcher window hidden. Use
`Alt+Space` to toggle the launcher.

When developing from the Snap-packaged VS Code app, native GTK/WebKit
environment variables can leak into child processes. If `tauri dev` fails with a
Snap library symbol error, retry from a normal terminal or a clean shell
environment.

## Packaging

Build a Linux desktop package through Tauri:

```bash
npm run tauri build
```

Linux build artifacts are written under:

```text
src-tauri/target/release/bundle/
```

After installing or launching a packaged build, Rat Search should stay resident
and wait for the launcher hotkey.

## Startup

Rat Search is intended to work well as a startup application, but v0.2 does not
auto-enable startup because there is no settings screen or consent flow yet. See
[Local Run, Packaging, and Autostart](docs/local_run_packaging_autostart.md)
for the prepared startup path and performance notes.

## Documentation

- [Documentation index](docs/README.md)
- [Version 0.2 overview](docs/version_0_2.md)
- [Local Run, Packaging, and Autostart](docs/local_run_packaging_autostart.md)
