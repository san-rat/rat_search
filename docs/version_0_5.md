# Rat Search Version 0.5

Version 0.5 improves default actions for files, folders, and calculator results
while preserving the Version 0.4 launcher, search, clipboard, and privacy
behavior.

## VS Code Open Intent

Use the `open` prefix to ask Rat Search to open matching code work in Visual
Studio Code:

```text
open pc_work
open work_done.md
```

- `open <folder>` opens matching folders in Visual Studio Code.
- `open <code-like-file>` opens matching code-like files in Visual Studio Code.
- `open <non-code-file>` opens non-code files with the system opener.
- Normal file/folder search without `open` keeps the existing default opener.
- Missing or failing VS Code falls back to the system opener.
- Folder opening in VS Code is supported.
- `Ctrl+Enter` still reveals file/folder results.
- `Ctrl+C` still copies file/folder paths when no input text is selected.

The VS Code command is resolved from `code`, `codium`, then `code-insiders`.
Rat Search validates selected paths against the existing file index before
opening them and does not use shell-string command execution.

## Calculator Action

Calculator search still evaluates local arithmetic expressions in Rat Search.
Pressing `Enter` on a calculator result now opens the desktop calculator app.

Calculator expression prefill depends on installed calculator support. When a
safe expression handoff is not available, Rat Search opens the calculator and
copies the existing calculator fallback text to the clipboard. This preserves
the previous calculator copy behavior as a fallback.

## Preserved Behavior

Version 0.5 keeps the completed Version 0.4 behavior stable:

- Applications.
- File and folder search.
- Calculator search and ranking.
- Google question search.
- GNOME Settings search.
- Search history.
- Clipboard history and privacy controls.
- Existing launcher keyboard navigation.
- Wayland shortcut caveat.

Clipboard history remains opt-in, local-only, and disabled by default.

## Exclusions

Version 0.5 does not include YouTube quick keys, GitHub quick keys, terminal
quick keys, `code <path>` quick keys, `calc <expr>` quick keys, note quick keys,
user-defined quick keys, user-defined commands, arbitrary shell execution, or
destructive actions.
