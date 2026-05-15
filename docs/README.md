# Rat Search Documentation

This folder contains user-facing project documentation. Local planning and work
logs may exist in ignored subfolders, but the files listed here are the stable
docs to read first.

## Current Version

- [Version 0.5 Overview](version_0_5.md): VS Code `open` intent, calculator app
  default action, fallbacks, exclusions, and preserved v0.4 behavior.
- [Version 0.5 Testing Guide](version_0_5_testing.md): automated, static, and
  manual checks for Version 0.5 action defaults.
- [Local Run, Packaging, and Autostart](local_run_packaging_autostart.md): how to
  run the app locally, package it, and prepare startup behavior.

## Prior Versions

- [Version 0.4 Overview](version_0_4.md): clipboard history behavior, privacy
  limits, frontend controls, and backend storage/search notes.
- [Version 0.4 Testing Guide](version_0_4_testing.md): automated, static, and
  manual checks for clipboard history verification.
- [Version 0.3 Overview](version_0_3.md): features, behavior, architecture, and
  verification notes for calculator, Google question search, settings, and
  history search.
- [Version 0.3 Testing Guide](version_0_3_testing.md): automated and manual
  checks for local v0.3 verification.
- [Version 0.2 Overview](version_0_2.md): local file-and-folder launcher
  release notes.

## Quick Facts

- Rat Search is a resident Tauri desktop utility for Ubuntu Linux.
- The launcher searches applications, files, folders, calculator expressions,
  Google question searches, GNOME Settings panels, recent query history, and
  opt-in local text clipboard history.
- `open <folder>` opens matching folders in Visual Studio Code.
- `open <code-like-file>` opens matching code-like files in Visual Studio Code.
- `open <non-code-file>` keeps the system opener for non-code files.
- Normal file/folder search without `open` keeps the existing default opener.
- Missing or failing VS Code falls back to the system opener.
- `Ctrl+Enter` still reveals file/folder results and `Ctrl+C` still copies
  file/folder paths.
- Calculator results open the desktop calculator with `Enter`; expression
  prefill depends on installed calculator support.
- Clipboard history is disabled by default, local-only, text-only, and can be
  disabled or cleared from the launcher.
- YouTube, GitHub, terminal, note, and user-defined quick keys are not included.
- File indexing is conservative by default: `Desktop`, `Documents`, `Downloads`,
  and `Pictures`.
- Search uses in-memory catalogs and lightweight history state so typing does
  not rescan the filesystem.
- Wayland users should bind their desktop shortcut to `rat-search toggle`
  because global shortcut registration is skipped by design.
