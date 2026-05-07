# Local Run, Packaging, and Autostart

This guide covers the v0.2 local run, Linux packaging path, and startup
preparation for Rat Search.

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
  shortcut to `rat-search toggle`.
- Search results use in-memory app and file/folder catalogs scanned at startup.

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

Version 0.2 builds Debian and RPM packages by default:

```text
src-tauri/target/release/bundle/deb/
src-tauri/target/release/bundle/rpm/
```

After installing or launching the packaged build, Rat Search should run like a
small desktop utility: resident in the background, hidden until invoked, and
ready for the hotkey.

## Autostart Preparation

Autostart is useful for Rat Search because the app should already be warm when
the user presses the launcher hotkey. Version 0.2 prepares for that workflow but
does not enable it automatically.

Startup is not auto-enabled in this step because there is no settings screen or
explicit consent flow yet. A later settings step can use the official Tauri v2
autostart plugin:

```text
https://v2.tauri.app/plugin/autostart/
```

That future implementation should add the plugin dependency, initialize it in
Rust, and grant only the required autostart permissions.

Until then, the manual Linux fallback is to install/package Rat Search normally
and add the installed app to Ubuntu's Startup Applications list. The startup
entry should launch Rat Search itself, not `rat-search toggle`; the app should
remain resident with the window hidden after startup.

## Performance Notes

Autostart means Rat Search uses some RAM while idle. CPU use should be near zero
after initial startup work finishes.

The main startup costs in v0.2 are scanning installed `.desktop` application
entries and indexing conservative user folders once. Search uses those in-memory
catalogs, so typing should stay responsive and should not trigger repeated
filesystem scans.

The v0.2 acceptance target is that Rat Search remains comfortable on an Ubuntu
machine with 8GB RAM.
