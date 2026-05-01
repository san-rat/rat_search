# Version 0.1 App Launcher Implementation Plan

## 1. Purpose

Version 0.1 is the foundation for a Spotlight-inspired command palette for Ubuntu Linux. The first release focuses on one excellent workflow:

```text
Press hotkey -> type app name -> select result -> launch app
```

This version is intentionally small, but its architecture must support the long-term product direction: file search, calculator, clipboard history, system actions, settings search, web shortcuts, and plugin-based result sources.

The priority is not to clone every Spotlight feature immediately. The priority is to make the first interaction feel fast, calm, keyboard-first, and reliable.

---

## 2. Product Goals

Version 0.1 is successful when the application can:

- Start automatically or manually and stay resident in the background.
- Open instantly from a global hotkey.
- Show a centered floating launcher window.
- Focus the search field immediately when opened.
- Discover installed Ubuntu applications from `.desktop` files.
- Search applications while the user types.
- Rank likely matches above weak matches.
- Navigate results with the keyboard.
- Launch the selected application with `Enter`.
- Close cleanly with `Esc` or by pressing the hotkey again.
- Remain responsive on the target Ubuntu machine.

The user should feel that the launcher is always ready, not that a full app is starting each time.

---

## 3. Tech Stack

Use this stack for Version 0.1:

```text
Desktop shell: Tauri v2
Backend: Rust
Frontend: Svelte + TypeScript
Storage: SQLite, reserved for usage history and future indexing
Target OS: Ubuntu Linux
```

### 3.1 Why This Stack

Rust is the long-term foundation for application discovery, search, ranking, launching, indexing, and system integration. It is fast, memory-efficient, and suitable for a resident background desktop utility.

Tauri v2 provides the desktop shell, native window management, command bridge, packaging path, global shortcut support, and a lightweight alternative to Electron.

Svelte with TypeScript provides a polished and reactive UI layer without excessive runtime weight.

SQLite is the local persistence layer for later versions. In Version 0.1, the app records can live in memory after startup. SQLite should be introduced only when needed for usage history, search preferences, or future indexing.

---

## 4. Architecture

Version 0.1 should be designed as a resident application:

```text
App starts
  -> Rust scans installed applications
  -> App records are stored in memory
  -> Global hotkey is registered
  -> Launcher window remains hidden

User presses hotkey
  -> Tauri shows and focuses launcher window
  -> Svelte search input receives focus

User types query
  -> Svelte sends query to Rust command
  -> Rust ranks in-memory app records
  -> Results are returned to Svelte
  -> UI updates immediately

User presses Enter
  -> Svelte sends selected app id to Rust command
  -> Rust launches the application safely
  -> Launcher hides
```

### 4.1 Main Subsystems

The implementation should be organized around these responsibilities:

- **Launcher window:** floating, centered, hidden by default, shown by hotkey.
- **Global shortcut:** toggles the launcher window.
- **Application source:** reads `.desktop` files and creates normalized app records.
- **Search engine:** filters and ranks app records for a query.
- **Launch action:** launches a selected application using its parsed desktop entry.
- **Frontend state:** owns query text, result list, selected result index, loading state, and empty state.
- **Settings defaults:** define hotkey, max result count, app directories, and theme behavior.

---

## 5. Version 0.1 Feature Scope

### 5.1 Included

Version 0.1 must include:

- Global hotkey toggle.
- Centered floating search window.
- Search input autofocus.
- Installed app discovery from `.desktop` files.
- App icon display where available.
- App title and subtitle display.
- Search-as-you-type.
- Exact, prefix, and fuzzy matching.
- Keyboard result navigation.
- `Enter` to launch.
- `Esc` to close.
- Mouse click to launch.
- Clean no-result state.
- Hidden-by-default resident behavior.

### 5.2 Excluded

Version 0.1 must not include:

- File indexing.
- Clipboard history.
- Calculator.
- Unit conversion.
- Web search shortcuts.
- Destructive system actions.
- Shell command execution.
- Plugin marketplace.
- Heavy animation or expensive blur effects.
- AI assistant integration.

These features should be designed for, but not implemented yet.

---

## 6. Step-by-Step Implementation

### Step 1: Create the Tauri Project

Bootstrap a Tauri v2 app using the Svelte + TypeScript template.

Expected project shape:

```text
rat_search/
  src/
    App.svelte
    main.ts
    app.css
  src-tauri/
    Cargo.toml
    tauri.conf.json
    src/
      main.rs
      lib.rs
```

Use a product name that can remain stable. A working internal name can be `Rat Search` until final branding is chosen.

### Step 2: Configure the Tauri Window

Configure the main launcher window as:

- Hidden on startup.
- Centered when shown.
- Always on top.
- Not maximized.
- Not resizable for Version 0.1.
- Transparent only if it does not cause rendering or focus issues.
- Sized for a compact command palette.

Recommended initial dimensions:

```text
Width: 720px
Height: 460px
```

The app should avoid looking like a normal document window. It should feel like a lightweight desktop overlay.

### Step 3: Add Global Shortcut Support

Add the Tauri global shortcut plugin and register the default launcher shortcut.

Recommended default:

```text
Alt + Space
```

Behavior:

- If the launcher is hidden, show it, center it, and focus it.
- If the launcher is visible, hide it.
- If shortcut registration fails, log the failure and allow the app to run without crashing.

Wayland and desktop environment differences may affect global shortcut support. Version 0.1 should use the Tauri plugin first. A later fallback can expose a `rat-search toggle` command so the user can bind it through Ubuntu keyboard settings.

### Step 4: Implement Application Discovery in Rust

Read `.desktop` entries from standard Linux application directories:

```text
/usr/share/applications
/usr/local/share/applications
~/.local/share/applications
```

For each valid app, parse:

- Desktop file id.
- Name.
- Generic name, if available.
- Comment, if available.
- Exec command.
- Icon name or path.
- Categories.
- Keywords.
- NoDisplay.
- Hidden.
- Terminal.

Ignore entries when:

- `NoDisplay=true`
- `Hidden=true`
- `Name` is missing
- `Exec` is missing

Normalize the app record into a Rust structure similar to:

```rust
struct AppRecord {
    id: String,
    name: String,
    generic_name: Option<String>,
    comment: Option<String>,
    exec: String,
    icon: Option<String>,
    categories: Vec<String>,
    keywords: Vec<String>,
    desktop_file_path: String,
    terminal: bool,
}
```

The Rust backend should scan app records at startup and keep them in memory.

### Step 5: Implement Safe Launching

Implement a Rust command that launches an app by id.

Desktop `Exec` values may contain field codes such as:

```text
%f %F %u %U %i %c %k
```

Version 0.1 should strip unsupported field codes before launch. It should not execute arbitrary user input.

If `Terminal=true`, launch the command in the user's default terminal if practical. If terminal launching is not reliable in Version 0.1, show the app in results but return a clear launch error.

After a successful launch, hide the launcher window.

### Step 6: Expose Rust Commands to the Frontend

Expose Tauri commands for:

```text
search_apps(query: string, limit: number) -> AppSearchResult[]
launch_app(app_id: string) -> LaunchResult
hide_launcher() -> void
```

The frontend should not know how `.desktop` files are parsed or how apps are launched. It should ask the backend for ranked results and request launch by id.

### Step 7: Implement Search and Ranking

Version 0.1 should rank applications using a simple deterministic score.

Suggested scoring:

```text
Exact name match: highest
Name starts with query: very high
Keyword/category exact match: high
Name contains query: medium
Generic name/comment contains query: medium-low
Fuzzy abbreviation match: medium-low
Weak fuzzy match: low
```

Examples:

```text
term -> Terminal
fire -> Firefox
code -> Visual Studio Code
files -> Files
sett -> Settings
```

The first result should usually be the expected app. This is more important than showing many results.

Recommended Version 0.1 result limit:

```text
8 results
```

SQLite-backed frequency and recency boosting should be reserved for a later improvement.

### Step 8: Build the Svelte UI

The UI should contain:

- One focused search input.
- A vertical results list.
- Result rows with icon, title, and subtitle.
- A selected result state.
- Empty state for no results.

The UI should not include explanatory text, onboarding copy, or visible keyboard shortcut instructions in the main launcher. The interface should feel obvious by behavior.

Recommended layout:

```text
+------------------------------------------------+
| Search input                                   |
+------------------------------------------------+
| App icon | App name                | source     |
|          | Comment or category                 |
|------------------------------------------------|
| App icon | App name                            |
|          | Comment or category                 |
+------------------------------------------------+
```

### Step 9: Implement Keyboard Behavior

Keyboard behavior:

```text
Esc          Close launcher
Enter        Launch selected result
ArrowDown    Move selection down
ArrowUp      Move selection up
Tab          Optional: move selection down
Shift+Tab    Optional: move selection up
```

Selection rules:

- When results change, select the first result.
- Arrow navigation should wrap from bottom to top and top to bottom.
- `Enter` should do nothing if there is no selected result.
- Typing should never lose input focus.

### Step 10: Add UI Polish

The launcher should feel premium but lightweight.

Requirements:

- Light and dark theme support.
- Clear selected row highlight.
- Consistent row height.
- App icons aligned cleanly.
- Text truncates gracefully.
- No layout shift while typing.
- No heavy blur or GPU-expensive effects in Version 0.1.
- Window opens centered and ready every time.

The design should prioritize speed, clarity, and keyboard confidence.

### Step 11: Add Basic Settings Defaults

Define defaults in Rust or a small config module:

```text
Default hotkey: Alt+Space
Max results: 8
Window width: 720
Window height: 460
Theme: system
Search source: applications only
```

Do not build a settings screen in Version 0.1. Keep the defaults centralized so a settings UI can be added later.

### Step 12: Prepare Local Run and Packaging

The first development target is a local dev run:

```text
npm run tauri dev
```

The first package target should be a Linux desktop build through Tauri:

```text
npm run tauri build
```

Packaging is not complete until the app can be started normally and the launcher can be triggered without using the terminal.

---

## 7. UI/UX Requirements

The user experience must follow these principles:

- The launcher opens quickly enough to feel instant.
- The search input is focused before the user starts typing.
- The best match appears at the top.
- The selected result is visually obvious.
- The interface is useful without the mouse.
- The UI is calm, minimal, and free of clutter.
- The app feels like an operating-system utility, not a website inside a window.

### 7.1 Visual Direction

Use a restrained visual style:

- Neutral surface color.
- High contrast text.
- Subtle border or shadow.
- 8px or smaller radius for result rows.
- Compact spacing.
- No decorative gradients.
- No landing page.
- No marketing-style hero screen.

### 7.2 Result Row Requirements

Each result row should show:

- App icon.
- App name.
- Short subtitle from comment, generic name, category, or source.
- Selected state.

Rows must have stable height so the interface does not jump while searching.

---

## 8. Linux Integration Details

### 8.1 `.desktop` Discovery

The backend must discover apps from common application directories and parse desktop entries directly. This avoids depending on a separate launcher tool and gives the project its own search foundation.

### 8.2 Icon Resolution

Version 0.1 should support:

- Absolute icon file paths.
- Icon names from the current icon theme where practical.
- A fallback generic app icon when no icon can be resolved.

The UI should never break because an icon is missing.

### 8.3 Hotkey Fallback Strategy

Global hotkeys can behave differently across X11, Wayland, GNOME, and other Linux desktop environments.

Version 0.1 should:

- Try Tauri global shortcut support first.
- Log or surface a clear error if registration fails.
- Keep a future path for a command-line toggle fallback.

Future fallback:

```text
rat-search toggle
```

This would let users bind the launcher manually through Ubuntu keyboard shortcut settings.

### 8.4 Autostart Plan

Autostart is desirable for the Spotlight experience because the app should already be warm when the user presses the hotkey.

Version 0.1 should document or prepare a `.desktop` autostart entry, but it does not need a full settings UI for enabling/disabling autostart.

---

## 9. Future-Proofing for Later Versions

Version 0.1 should avoid decisions that block future features.

Use a source-like internal model even though only apps exist at first:

```text
SearchSource: Applications
SearchResult: shared result shape
Action: launch app
```

Later versions can add:

- Files source.
- Calculator source.
- Web source.
- Clipboard source.
- Settings source.
- Actions source.

The frontend should render generic search results rather than app-only UI wherever possible. App-specific behavior should live in the backend.

Recommended shared result shape:

```rust
struct SearchResult {
    id: String,
    title: String,
    subtitle: Option<String>,
    icon: Option<String>,
    source: String,
    action: String,
    score: f64,
}
```

In Version 0.1, every result can have:

```text
source = "applications"
action = "launch_app"
```

---

## 10. Testing and Acceptance Criteria

### 10.1 Manual Acceptance Tests

Run these checks before considering Version 0.1 complete:

- Pressing the configured hotkey opens the centered launcher.
- Pressing the hotkey again closes the launcher.
- Pressing `Esc` closes the launcher.
- Search input is focused immediately every time the launcher opens.
- Typing `term` finds Terminal quickly.
- Typing `fire` finds Firefox if installed.
- Typing `files` finds the system file manager.
- Arrow keys move through results without mouse interaction.
- Result selection wraps at the top and bottom.
- `Enter` launches the selected application.
- Clicking a result launches it.
- Missing icons use a fallback icon.
- Invalid or hidden `.desktop` entries do not appear.
- Empty query shows useful default app results or an empty calm state.
- No-result query shows a clean no-result state.
- The app remains responsive while typing.

### 10.2 Automated Test Targets

Add Rust tests for:

- Desktop entry parsing.
- Hidden app filtering.
- NoDisplay app filtering.
- Exec field-code cleanup.
- Exact match ranking.
- Prefix match ranking.
- Fuzzy match ranking.
- Result limit enforcement.

Add frontend tests later if the UI grows complex. For Version 0.1, manual UI verification is acceptable, but keyboard behavior should be checked carefully.

### 10.3 Performance Acceptance

The target behavior:

- App discovery runs once at startup.
- Search uses in-memory app records.
- Typing does not block the UI.
- Hotkey show/hide feels instant after the app is resident.
- The app remains comfortable on an Ubuntu machine with modest CPU and 8GB RAM.

---

## 11. Completion Definition

Version 0.1 is complete when:

- The project can be launched locally with Tauri.
- The app stays resident after startup.
- The launcher window is hidden until invoked.
- The hotkey toggles the launcher.
- Installed apps are searchable.
- Results are ranked well enough for common app names.
- Apps can be launched from the keyboard.
- The UI feels polished, minimal, and fast.
- The implementation leaves a clear path for Version 0.2 file search.

---

## 12. Assumptions

- The target operating system is Ubuntu Linux.
- The first implementation starts from an empty project structure.
- Version 0.1 is application launcher only.
- The app should be designed as a long-running resident utility.
- The default hotkey is `Alt+Space` unless it conflicts with the user's desktop environment.
- SQLite is part of the long-term stack, but not mandatory for the first in-memory search loop.
- File search, clipboard history, calculator, and actions are intentionally deferred.
- The UI should prioritize Spotlight-like speed and clarity over visual effects.
