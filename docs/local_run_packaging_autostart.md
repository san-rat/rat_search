# Local Run, Packaging, and Autostart

This guide covers local development, Linux packaging, Version 1 startup setup,
and GNOME Wayland hotkey behavior for Rat Search.

## Local Development

Start the app in Tauri development mode:

```bash
npm run tauri dev
```

Expected behavior:

- The native app starts and stays resident.
- The launcher window starts hidden.
- `Alt+Space` toggles the launcher on X11.
- On Wayland, global shortcut registration is skipped; bind your desktop
  shortcut to the setup-script shell command that runs `rat-search foreground`.
- Search results use in-memory app and file/folder catalogs scanned at startup,
  plus lightweight search history and clipboard state loaded from app data.
- `open <folder>` and `open <code-like-file>` results open in Visual Studio
  Code when available, with system-opener fallback.
- Calculator results open the desktop calculator app, with clipboard fallback
  when expression prefill is unavailable.
- Clipboard history starts disabled and must be enabled from the launcher before
  text clipboard changes are recorded.

If the Tauri CLI cannot find `cargo`, make sure Cargo's bin directory is on
`PATH`:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

If you run from the Snap-packaged VS Code app and see a native library symbol
error, retry from a normal terminal or a clean shell environment. Snap GTK/WebKit
environment variables can leak into child processes.

## Linux Packaging

Build a Linux desktop package through Tauri:

```bash
npm run tauri build
```

Linux build artifacts are written under:

```text
src-tauri/target/release/bundle/
```

Version 1 builds Debian and RPM packages by default:

```text
src-tauri/target/release/bundle/deb/
src-tauri/target/release/bundle/rpm/
```

After installing or launching the packaged build, Rat Search should run like a
small desktop utility: resident in the background, hidden until invoked, and
ready for the hotkey.

## Version 1 Autostart And Hotkey

Autostart is useful for Rat Search because the resident process should already
be warm when the user presses the launcher hotkey. Version 1 enables this with
the current-user setup script after package install:

```bash
bash /home/sanuk/Desktop/Projects/rat_search/scripts/setup_ubuntu_user_startup.sh
```

The autostart entry launches the resident app:

```text
Exec=rat-search
```

The GNOME Wayland shortcut runs a foreground launcher:

```bash
/bin/sh -c 'exec rat-search foreground --startup-id "$DESKTOP_STARTUP_ID" --xdg-activation-token "$XDG_ACTIVATION_TOKEN"'
```

The foreground launcher exists because GNOME Wayland focus prevention is more
reliable when the visible window is created by the user-activated shortcut
process. The resident process remains responsible for warm app/file/history
state through local IPC.

To disable startup and Rat Search custom shortcuts, run:

```bash
bash /home/sanuk/Desktop/Projects/rat_search/scripts/disable_ubuntu_user_startup.sh
```

## Performance Notes

Autostart means Rat Search uses some RAM while idle. CPU use should be near zero
after initial startup work finishes.

The main resident startup costs in Version 1 are scanning installed `.desktop` application
entries, indexing conservative user folders once, loading lightweight search
history, and loading clipboard settings/history from app data. Clipboard polling
runs only as a lightweight text read loop, and storage remains inactive until
clipboard history is enabled.

Foreground mode skips warmup and asks the resident process for search results
and actions over local IPC. Tauri/WebKit startup cost can still affect how
quickly the foreground window appears.
