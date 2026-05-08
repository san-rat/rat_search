# Rat Search Documentation

This folder contains user-facing project documentation. Local planning and work
logs may exist in ignored subfolders, but the files listed here are the stable
docs to read first.

## Current Version

- [Version 0.3 Overview](version_0_3.md): features, behavior, architecture, and
  verification notes for calculator, web shortcut, settings, and history
  search.
- [Local Run, Packaging, and Autostart](local_run_packaging_autostart.md): how to
  run the app locally, package it, and prepare startup behavior.

## Prior Versions

- [Version 0.2 Overview](version_0_2.md): local file-and-folder launcher
  release notes.

## Quick Facts

- Rat Search is a resident Tauri desktop utility for Ubuntu Linux.
- The launcher searches applications, files, folders, calculator expressions,
  explicit web shortcuts, GNOME Settings panels, and recent query history.
- File indexing is conservative by default: `Desktop`, `Documents`, `Downloads`,
  and `Pictures`.
- Search uses in-memory catalogs and lightweight history state so typing does
  not rescan the filesystem.
- Wayland users should bind their desktop shortcut to `rat-search toggle`
  because global shortcut registration is skipped by design.
