# Rat Search Version 0.4

Version 0.4 adds privacy-first local clipboard history to the existing
application, file, folder, calculator, Google question, GNOME Settings, and
search history launcher.

## Clipboard History

- Clipboard history is opt-in and disabled by default.
- Clipboard history is stored locally in app data.
- Only text clipboard history is supported.
- Image, file-list, rich text, and HTML clipboard formats are not stored.
- Users can disable future tracking without clearing existing history.
- Users can clear all clipboard history or delete a single clipboard item.
- Clipboard results can be copied back to the system clipboard from search.

## Privacy Limits

Sensitive-content filtering is best effort, not a password-manager-grade
detector. Rat Search skips common private key blocks and obvious secret labels
such as passwords, tokens, and API keys before text enters local history.

The app avoids logging clipboard text, previews, normalized text, item IDs, or
stored file contents. Clipboard result metadata exposes bounded preview data for
display, not full unbounded clipboard text.

## Controls

The launcher includes a compact clipboard privacy panel. It shows enabled state,
entry count, retention days, maximum entries, and maximum text bytes. Enabling
clipboard history requires an explicit second click after a local-history
warning.

## Storage And Search

Clipboard settings and history are loaded at startup. Expired clipboard entries
are pruned during startup load. Clipboard monitoring records text changes only
when history is enabled. Clipboard search is merged into unified search after
strong live sources and before search history.

## Wayland Note

The existing Wayland global-shortcut limitation remains unchanged. On Wayland,
bind the desktop shortcut to `rat-search toggle`; on X11, `Alt+Space` toggles
the launcher.
