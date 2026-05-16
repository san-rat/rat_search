# Changelog

## v1.0.0 - 2026-05-16

Rat Search Version 1 is the first installed desktop release for Ubuntu GNOME.

### Highlights

- Installs as `Rat Search` with version `1.0.0`.
- Starts a hidden resident `rat-search` process at login through the current-user
  setup script.
- Uses `Ctrl+Alt+Space` as the GNOME Wayland launcher shortcut.
- Runs `rat-search foreground` from the GNOME shortcut so the launcher receives
  focus reliably on Wayland.
- Uses resident IPC so foreground launchers get warm app, file, history, and
  optional clipboard state without rescanning.
- Keeps foreground fallback behavior when the resident process is unavailable.
- Preserves app, file, folder, calculator, settings, web, history, and opt-in
  clipboard search/action flows.
- Keeps clipboard history local-only and disabled by default.
- Rebuilds Linux `.deb` and `.rpm` package artifacts.

### Install

Install the Debian package:

```bash
sudo apt install -y "/home/sanuk/Desktop/Projects/rat_search/src-tauri/target/release/bundle/deb/Rat Search_1.0.0_amd64.deb"
```

Configure current-user startup and the GNOME shortcut:

```bash
bash /home/sanuk/Desktop/Projects/rat_search/scripts/setup_ubuntu_user_startup.sh
```

Restart the resident process after reinstalling:

```bash
pkill -f rat-search
rat-search
```

### Artifacts

```text
b0b4948b3abcc0ccc7a8c3afb1f50e3a68c0537104d8face374aafef029f2807  Rat Search_1.0.0_amd64.deb
3582656a993faeccc1e95a3418764e127c4b8f70edb57aad47441dabded2c817  Rat Search-1.0.0-1.x86_64.rpm
```

### Verification

- `npm run check`
- `npm run build`
- `/home/sanuk/.cargo/bin/cargo check --manifest-path src-tauri/Cargo.toml`
- `/home/sanuk/.cargo/bin/cargo test --manifest-path src-tauri/Cargo.toml`
- `git diff --check`
- `/home/sanuk/.cargo/bin/cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- `PATH=/home/sanuk/.cargo/bin:$PATH npm run tauri build`
- Targeted Rust filters: `search_result`, `file_search`, `file_actions`,
  `calculator`, `mixed_search`, and `ipc`

### Known Limitations

- No tray or top-bar indicator yet.
- No settings UI or in-app hotkey editor yet.
- No crash restart service yet.
- Tauri/WebKit startup cost can still affect foreground window appearance time,
  even though resident IPC keeps search/action state warm.
- Primary target is Ubuntu GNOME Wayland.
