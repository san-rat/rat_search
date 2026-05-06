# Rat Search

A Spotlight-inspired command palette and app launcher for Ubuntu Linux, built with
Tauri, Rust, and Svelte.

## Local Development

Run the Tauri development app:

```bash
npm run tauri dev
```

The app starts as a resident utility with the launcher window hidden. Use
`Alt+Space` to toggle the launcher.

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

Rat Search is intended to work well as a startup application, but v0.1 does not
auto-enable startup because there is no settings screen or consent flow yet. See
[Local Run, Packaging, and Autostart](docs/guides/local_run_packaging_autostart.md)
for the prepared startup path and performance notes.
