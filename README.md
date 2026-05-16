# Rat Search

A Spotlight-inspired command palette for Ubuntu Linux, built with Tauri, Rust,
and Svelte.

Rat Search installs as a resident desktop utility and opens a compact focused
launcher on demand. Version 1 searches installed applications, files, folders,
local calculator expressions, Google question searches, GNOME Settings panels,
recent search history, and opt-in local text clipboard history.

## Features

- Search and launch installed Linux desktop applications.
- Search files and folders by name from `Desktop`, `Documents`, `Downloads`,
  and `Pictures`.
- Evaluate local arithmetic expressions and open the desktop calculator with
  `Enter`.
- Search `open <folder>` to open matching folders in Visual Studio Code.
- Search `open <code-like-file>` to open matching code-like files in Visual
  Studio Code.
- Search `open <non-code-file>` to open matching PDFs, images, videos, audio,
  archives, and other non-code files with the system opener.
- Keep normal file/folder searches without `open` on the existing default
  opener.
- Fall back to the system opener when VS Code is missing or cannot open the
  selected item.
- Open Google searches for question-style queries, including sentences ending
  in `?` or containing whole words such as `what`, `how`, or `should`.
- Find whitelisted GNOME Settings panels such as Wi-Fi, Bluetooth, Displays,
  Keyboard, and Sound.
- Reuse recent successful search queries from local history.
- Opt in to local text clipboard history, search older copied text, copy a
  stored item back to the system clipboard, delete one item, or clear history.
- Keep clipboard history local, disabled by default, and filtered with
  best-effort sensitive text checks.
- Run the selected result's default action with `Enter`.
- Reveal file and folder results with `Ctrl+Enter`.
- Copy file and folder paths with `Ctrl+C` when no search text is selected.
- Render app icons from the local icon theme when available.
- Show source-aware app, file, folder, calculator, web, settings, and history
  icons, including clipboard results.
- Keep search responsive by scanning files once at startup and searching
  in-memory app/file/folder catalogs plus lightweight history state.

## Keyboard

| Shortcut | Behavior |
| --- | --- |
| `Alt+Space` | Toggle the launcher on X11 |
| `Ctrl+Alt+Space` | Open or close the launcher through the GNOME shortcut on Wayland |
| `Up` / `Down` | Move selection |
| `Tab` | Cycle selection |
| `Enter` | Run the selected result's default action |
| `Ctrl+Enter` | Reveal the selected file/folder |
| `Ctrl+C` | Copy the selected file/folder path if no input text is selected |
| `Esc` | Clear or close the launcher |

On Wayland, global shortcut registration is skipped by design. The Version 1
setup script binds `Ctrl+Alt+Space` to a shell command that runs
`rat-search foreground` so GNOME treats the launcher as a fresh user-activated
window. The login startup entry still runs plain `rat-search` as the hidden
resident process.

## Local Development

Run the Tauri development app:

```bash
npm run tauri dev
```

If the Tauri CLI cannot find Cargo, add Cargo to `PATH` first:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

The development app starts as a resident utility with the launcher window
hidden. Use `Alt+Space` to toggle the launcher on X11. On GNOME Wayland, test
the installed shortcut path with the setup script and `Ctrl+Alt+Space`.

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

After installing and running the setup script, Rat Search starts at login as a
hidden resident process and the GNOME shortcut opens focused foreground
launchers that use resident IPC for warm search/action state.

## Startup

Rat Search Version 1 uses a current-user setup script for startup and hotkey
configuration after package install:

```bash
bash /home/sanuk/Desktop/Projects/rat_search/scripts/setup_ubuntu_user_startup.sh
```

The setup installs an autostart entry with `Exec=rat-search` and configures the
GNOME Wayland hotkey to run `rat-search foreground`.

## Version 1 Notes

The `open` prefix is the only VS Code intent in Version 1. Rat Search does not add
YouTube, GitHub, terminal, note, `code <path>`, `calc <expr>`, user-defined
quick keys, user-defined commands, arbitrary shell execution, or destructive
actions.

Calculator expression prefill depends on installed calculator support. When a
safe prefill form is not available, Rat Search opens the calculator and copies
the existing calculator fallback text.

Version 1 preserves opt-in local clipboard history. Clipboard history remains
local-only and disabled by default.

There is no tray, settings UI, or crash restart service yet. Tauri/WebKit
startup cost can still affect how quickly the foreground launcher appears,
though resident IPC keeps search/action state warm.

## Documentation

- [Documentation index](docs/README.md)
- [Version 1 overview](docs/version_1.md)
- [Version 1 testing](docs/version_1_testing.md)
- [Version 0.5 overview](docs/version_0_5.md)
- [Version 0.5 testing](docs/version_0_5_testing.md)
- [Local Run, Packaging, and Autostart](docs/local_run_packaging_autostart.md)
