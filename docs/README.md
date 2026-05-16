# Rat Search Documentation

This folder contains user-facing project documentation. Local planning and work
logs may exist in ignored subfolders, but the files listed here are the stable
docs to read first.

## Current Version

- [Version 0.5 Overview](version_0_5.md): VS Code `open` intent, calculator app
  default action, fallbacks, exclusions, and preserved launcher behavior.
- [Version 0.5 Testing Guide](version_0_5_testing.md): automated, static, and
  manual checks for Version 0.5 action defaults.
- [Local Run, Packaging, and Autostart](local_run_packaging_autostart.md): how to
  run the app locally, package it, and prepare startup behavior.

## Planning References

- [Version 0.5 Implementation Plan](guides/version_0_5_implementation_plan.md):
  completed step-by-step implementation source for the current release.
- [Version 0.4 Implementation Plan](guides/version_0_4_implementation_plan.md):
  clipboard history planning reference.
- [Version 0.3 Implementation Plan](guides/version_0_3_implementation_plan.md):
  calculator, Google question search, settings, and history planning reference.
- [Version 0.2 Implementation Plan](guides/version_0_2_implementation_plan.md):
  local file-and-folder launcher planning reference.

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
- Wayland users should bind their desktop shortcut to `rat-search foreground`
  because global shortcut registration is skipped by design.
