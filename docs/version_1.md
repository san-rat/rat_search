# Version 1 Overview

Version 1 makes Rat Search an installed Ubuntu GNOME utility instead of a
development-run launcher.

The intended daily flow is:

```text
Log in -> resident rat-search starts hidden -> press Ctrl+Alt+Space -> type
```

## Installed Usage

Press `Ctrl+Alt+Space` to open the focused launcher. Press it again while the
launcher is visible to close it.

The GNOME Wayland shortcut runs:

```bash
/bin/sh -c 'exec rat-search foreground --startup-id "$DESKTOP_STARTUP_ID" --xdg-activation-token "$XDG_ACTIVATION_TOKEN"'
```

`rat-search foreground` starts a short-lived focused launcher process. This is
intentional on GNOME Wayland because Mutter is strict about focus stealing for
already-running background windows.

At login, the autostart entry runs the resident process:

```bash
rat-search
```

The resident process stays hidden in the background and keeps the app catalog,
file index, search history, and optional clipboard state warm.

## Resident IPC

Foreground launchers use a local Unix socket to ask the resident process for
warm search results and action execution. The socket path is:

```text
${XDG_RUNTIME_DIR}/rat-search.sock
```

If `XDG_RUNTIME_DIR` is unavailable, Rat Search falls back to:

```text
/tmp/rat-search-${UID}.sock
```

IPC messages are command-shaped and local-user-only. Rat Search does not expose
arbitrary shell execution over IPC.

If the resident process is missing, `rat-search foreground` still opens and uses
its lightweight local fallback behavior.

## Install And Setup

Build the package:

```bash
PATH=/home/sanuk/.cargo/bin:$PATH npm run tauri build
```

Install or reinstall the generated Debian package:

```bash
sudo apt install -y "/home/sanuk/Desktop/Projects/rat_search/src-tauri/target/release/bundle/deb/Rat Search_1.0.0_amd64.deb"
```

Configure current-user startup, icon assets, and the GNOME shortcut:

```bash
bash /home/sanuk/Desktop/Projects/rat_search/scripts/setup_ubuntu_user_startup.sh
```

Restart the resident process after reinstalling:

```bash
pkill -f rat-search
rat-search
```

## Disable

Run the disable script to remove the current-user autostart entry and Rat Search
GNOME custom shortcut bindings that point at the old or current Rat Search
commands:

```bash
bash /home/sanuk/Desktop/Projects/rat_search/scripts/disable_ubuntu_user_startup.sh
```

The script does not uninstall the package.

## Clipboard History

Clipboard history remains disabled by default. When enabled, it is local-only,
text-only, and filtered with best-effort sensitive text checks.

## Known Limitations

- There is no tray or top-bar indicator.
- There is no settings UI or in-app hotkey editor.
- There is no crash restart service.
- Tauri/WebKit startup cost can still affect how quickly the foreground window
  appears, even though resident IPC keeps search/action state warm.
- Version 1 is focused on Ubuntu GNOME Wayland for this machine.
