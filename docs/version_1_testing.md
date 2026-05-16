# Version 1 Testing

This guide verifies the installed Version 1 release path: package install,
resident startup, GNOME Wayland foreground hotkey, resident IPC, fallback
behavior, and normal search/actions.

## Automated Checks

Run the full validation set from the repository root:

```bash
npm run check
npm run build
/home/sanuk/.cargo/bin/cargo check --manifest-path src-tauri/Cargo.toml
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
/home/sanuk/.cargo/bin/cargo fmt --manifest-path src-tauri/Cargo.toml --check
PATH=/home/sanuk/.cargo/bin:$PATH npm run tauri build
```

Run targeted backend tests:

```bash
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml search_result
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml file_search
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml file_actions
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml calculator
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml mixed_search
/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml ipc
```

## Package Reinstall

Install the generated package:

```bash
sudo apt install -y "/home/sanuk/Desktop/Projects/rat_search/src-tauri/target/release/bundle/deb/Rat Search_1.0.0_amd64.deb"
```

Run current-user setup:

```bash
bash /home/sanuk/Desktop/Projects/rat_search/scripts/setup_ubuntu_user_startup.sh
```

Restart the resident process:

```bash
pkill -f rat-search
rat-search
```

## Static Checks

Confirm the autostart desktop entry starts the resident process:

```bash
rg -n "^Exec=rat-search$" ~/.config/autostart/rat-search.desktop
```

Confirm the GNOME shortcut command uses the focused foreground launcher:

```bash
gsettings get org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/ command
```

The command should contain:

```text
rat-search foreground
```

Confirm startup entries do not use development commands or foreground mode:

```bash
rg -n "npm|vite|tauri dev|foreground" ~/.config/autostart/rat-search.desktop
```

That command should return no matches.

Confirm the generated package desktop entry enables startup notification:

```bash
mkdir -p /tmp/rat-search-deb-check
dpkg-deb -x "/home/sanuk/Desktop/Projects/rat_search/src-tauri/target/release/bundle/deb/Rat Search_1.0.0_amd64.deb" /tmp/rat-search-deb-check
rg -n "StartupNotify=true" "/tmp/rat-search-deb-check/usr/share/applications/Rat Search.desktop"
```

## Focus And Startup

After setup, log out and log in. Then press `Ctrl+Alt+Space`.

Expected result:

- The launcher appears centered.
- The launcher is focused immediately.
- Typing goes into Rat Search without a mouse click.
- No GNOME "Rat Search is ready" focus-stealing notification appears.
- Pressing `Ctrl+Alt+Space` again while visible closes the launcher.

Repeat the same check after a reboot.

## Resident IPC

Confirm the resident process is running:

```bash
pgrep -af '^rat-search'
```

Confirm the IPC socket exists:

```bash
ls -l "${XDG_RUNTIME_DIR:-/tmp}/rat-search.sock"
```

Trigger the launcher and type a query that should use warm state, such as an
installed app name or a file indexed from the default folders. Search results
should appear without foreground scanning.

When collecting timing details, start the resident with logs redirected:

```bash
pkill -f rat-search
rat-search >/tmp/rat-search-resident.log 2>&1 &
```

Then press `Ctrl+Alt+Space` and inspect:

```bash
cat /tmp/rat-search-resident.log
```

Use the startup, frontend ready, first search, and IPC response logs to separate
foreground Tauri/WebKit boot latency from resident data latency.

## Resident-Missing Fallback

Stop the resident process and remove foreground state:

```bash
pkill -f rat-search
rm -f /tmp/rat-search-foreground.pid
```

Open a foreground launcher:

```bash
rat-search foreground
```

Expected result:

- The launcher still opens.
- Lightweight local fallback results work.
- Resident-only warm state may be unavailable until `rat-search` is started
  again.

Restart the resident afterward:

```bash
rat-search
```

## Normal Search And Actions

Verify these flows from the installed foreground launcher:

- App search and launch.
- File and folder search.
- `open <folder>` and `open <code-like-file>` Visual Studio Code actions.
- Non-code file open with the system opener.
- `Ctrl+Enter` reveal for file/folder results.
- `Ctrl+C` path copy when no input text is selected.
- Calculator expression result and calculator app action.
- GNOME Settings panel result.
- Google question-style query result.
- Recent search history result.
- Clipboard result behavior only when clipboard history is explicitly enabled.
