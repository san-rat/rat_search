# macOS Spotlight Feature Inventory  
## Reference document for building a Spotlight-like launcher for Ubuntu Linux

**Purpose:** This document lists the major, minor, and easily-missed features of Apple’s macOS Spotlight so they can be studied and recreated as a Linux/Ubuntu project.

**Scope:** This is a product/feature specification, not source code. It focuses on what Spotlight does, how it behaves, and what a Linux clone should consider implementing.

**Reference baseline:** macOS Tahoe 26 / recent macOS Spotlight behavior, based mainly on Apple’s Mac User Guide and Apple Support pages. Some features may depend on macOS version, region, device, language, apps installed, Apple Intelligence availability, or privacy settings.

---

# 1. What Spotlight is

Spotlight is macOS’s system-wide search and command interface. It is not only an app launcher. It combines:

- Application launching
- File search
- Folder search
- System setting search
- App-content search
- Web/internet suggestions
- Clipboard history search
- Calculator
- Unit/currency/time conversion
- Quick Look previews
- Finder integration
- Action execution
- Shortcuts execution
- Siri query handoff
- Privacy-controlled indexing

For your Ubuntu project, think of Spotlight as a **central command palette for the whole operating system**.

---

# 2. Core design idea

Spotlight follows a simple user experience:

1. User presses a shortcut.
2. A centered search window appears.
3. User types naturally.
4. Results appear immediately while typing.
5. The best match is prioritized.
6. User can open, preview, copy, filter, or act on the result.
7. User can run commands without leaving the keyboard.

The important design principle is:

> Search first, then action.

The user should not need to open a separate app, file manager, calculator, browser, or settings window for small tasks.

---

# 3. Entry points / ways to open Spotlight

macOS Spotlight can be opened in several ways:

## 3.1 Keyboard shortcut

- `Command + Space`
- Main default Spotlight shortcut.
- Opens and closes the Spotlight search window.

## 3.2 Menu bar icon

- Spotlight can be opened by clicking the Spotlight icon in the macOS menu bar.
- The icon may be shown or hidden depending on menu bar settings.

## 3.3 Keyboard search key

- Some Mac keyboards have a dedicated Search/Spotlight key in the function-key row.
- Pressing it opens Spotlight.

## 3.4 Finder search shortcut

- `Option + Command + Space`
- Opens a Finder window with the search field selected.
- This is separate from the small floating Spotlight window.

## Ubuntu clone requirement

Your version should support:

- A global hotkey, for example `Alt + Space`, `Ctrl + Space`, or `Super + Space`
- Optional tray/top-bar icon
- Optional command-line launch command
- Optional file-manager search mode
- Ability to close with the same shortcut or `Esc`

---

# 4. Spotlight window behavior

## 4.1 Floating search window

Spotlight appears as a floating search interface, not a normal full application window.

## 4.2 Can be moved

The Spotlight window can be dragged around the desktop.

## 4.3 Can be resized

The window can be resized to show more or fewer results.

## 4.4 Minimal UI

The interface is simple:

- Search field at top
- Results below
- Preview/extra information area depending on result
- Browse-mode buttons/categories near the search field in newer macOS versions

## 4.5 Keyboard-first behavior

Spotlight is designed to be controlled mostly from the keyboard:

- Open
- Type
- Navigate
- Preview
- Run
- Close

## Ubuntu clone requirement

Your UI should be:

- Centered
- Fast to open
- Keyboard-focused
- Minimal
- Light/dark theme aware
- Able to show icons, titles, subtitles, source/category, and preview information
- Fast enough to feel instant even on older hardware

---

# 5. Search-as-you-type behavior

## 5.1 Live results

Results appear while the user types.

## 5.2 Top matches first

Spotlight prioritizes the most likely result near the top.

## 5.3 Search suggestions

Spotlight may suggest variations of the user’s search query.

## 5.4 Search history

Pressing the Up Arrow can show previous Spotlight searches.

## 5.5 Natural search style

Users do not always need exact filenames. Spotlight can match:

- App names
- File names
- Folder names
- Document content
- Contacts
- Calendar items
- Mail messages
- Settings
- Web suggestions
- Actions
- Clipboard items

## Ubuntu clone requirement

You should implement:

- Instant search while typing
- Ranking system
- Fuzzy search
- Recent query memory
- Recent item boosting
- Exact match boosting
- App launch boosting
- Frequently used result boosting

---

# 6. Result categories

Spotlight results can come from multiple sources.

## 6.1 Applications

Examples:

- Safari
- Mail
- Notes
- Preview
- Terminal
- System Settings
- Any installed app

Expected behavior:

- Type app name
- Select result
- Press Return
- App opens

## 6.2 Files

Examples:

- PDFs
- Text files
- Word documents
- Images
- Videos
- Music/audio files
- Code files
- Presentations
- Downloads
- Desktop files
- iCloud Drive files

Expected behavior:

- Search by name
- Search by type
- Search by content where supported
- Show location
- Open file
- Preview file
- Reveal file in Finder

## 6.3 Folders

Expected behavior:

- Search folder names
- Open folder in Finder
- Show path
- Support folder-type filtering

## 6.4 Contacts

Spotlight can search contacts.

Possible result data:

- Name
- Phone number
- Email address
- Organization
- Contact card
- Communication actions

## 6.5 Email / Mail messages

Spotlight can search email results, depending on mail configuration and indexing.

Possible search targets:

- Sender
- Recipient
- Subject
- Message content
- Attachments
- Date
- Mailbox

## 6.6 Calendar events

Spotlight can search calendar events.

Possible search targets:

- Event title
- Date/time
- Location
- Attendees
- Notes/description

## 6.7 Reminders

Spotlight can search reminders.

Possible search targets:

- Reminder title
- Notes
- Due date
- List name
- Completion state

## 6.8 Images

Spotlight can search image files.

Possible search targets:

- File name
- Metadata
- Location
- Date
- Image category where supported by system intelligence

## 6.9 Movies / videos

Spotlight can search video files.

Possible search targets:

- File name
- File type
- Metadata
- Folder/location

## 6.10 Music / audio

Spotlight can search audio and music files.

Possible search targets:

- Song name
- Artist
- Album
- File name
- Audio file type

## 6.11 PDFs

Spotlight can search PDFs.

Possible search targets:

- PDF file name
- PDF text content where indexed
- Metadata
- Location

## 6.12 System Settings

Spotlight can search system settings.

Examples:

- Wi-Fi
- Bluetooth
- Display
- Keyboard
- Trackpad
- Privacy
- Spotlight settings
- Sound
- Wallpaper

Expected behavior:

- Type setting name
- Press Return
- Relevant System Settings page opens

## 6.13 Bookmarks

Spotlight can search browser bookmarks, especially Safari bookmarks.

Possible search targets:

- Bookmark title
- URL
- Browser history/bookmark metadata

## 6.14 Fonts

Spotlight can search installed fonts.

Possible search targets:

- Font name
- Font family
- Font file

## 6.15 Presentations

Spotlight can search presentation files.

Possible file types:

- Keynote
- PowerPoint
- Other presentation files

## 6.16 Internet / web suggestions

Spotlight can show internet-based suggestions or related web content depending on settings.

Examples:

- General knowledge results
- Search suggestions
- Web results
- Related content

## 6.17 Clipboard history

Newer Spotlight versions can search clipboard history when enabled.

Possible clipboard results:

- Copied text
- Copied files
- Previously copied content

## 6.18 Actions

Newer Spotlight versions can search and run actions.

Examples:

- Send message
- Send email
- Create event
- Run shortcut
- Remove image background
- Translate text
- Change text case
- Recognize music

## Ubuntu clone requirement

Build a plugin-based source system:

```text
Search Source 1: Applications
Search Source 2: Files
Search Source 3: Folders
Search Source 4: Settings
Search Source 5: Web
Search Source 6: Calculator
Search Source 7: Unit Converter
Search Source 8: Clipboard
Search Source 9: Commands / Actions
Search Source 10: Browser Bookmarks
Search Source 11: Recent Files
Search Source 12: Terminal Commands
```

---

# 7. Browse modes

Recent macOS Spotlight includes focused browse modes.

## 7.1 Applications mode

Shortcut:

- `Command + 1`

Purpose:

- Search only applications.
- Makes Spotlight work like an app launcher.

## 7.2 Files mode

Shortcut:

- `Command + 2`

Purpose:

- Search only files.
- Useful when the user wants documents or downloads, not apps or web results.

## 7.3 Actions mode

Shortcut:

- `Command + 3`

Purpose:

- Search actions and shortcuts.
- Allows the user to run tasks directly from Spotlight.

## 7.4 Clipboard mode

Shortcut:

- `Command + 4`

Purpose:

- Search clipboard history.

## 7.5 Dynamic category chips

Spotlight can show dynamic category filters below the search field.

Examples:

- Screenshot
- System Settings
- Folders
- PDFs
- Files
- Apps

These can change as the user types.

## Ubuntu clone requirement

Your version should include tab/filter modes such as:

```text
All | Apps | Files | Folders | Settings | Web | Calculator | Clipboard | Commands
```

Keyboard shortcuts could be:

```text
Ctrl+1 = Apps
Ctrl+2 = Files
Ctrl+3 = Actions
Ctrl+4 = Clipboard
Ctrl+5 = Web
```

---

# 8. Filtering and narrowing search

Spotlight supports several ways to narrow results.

## 8.1 Search within an app

Pattern:

```text
AppName + Tab + query
```

Example:

```text
Mail [Tab] assignment
Notes [Tab] project idea
Preview [Tab] pdf name
```

Behavior:

- User types an app name.
- Presses Tab.
- Spotlight searches within that app/source.

## 8.2 Search by item kind using slash

Pattern:

```text
/type
```

Example:

```text
/PDF
/folder
/application
```

Behavior:

- The slash filter narrows the search to a type/category.

## 8.3 Search by kind keyword

Pattern:

```text
kind:type query
```

Examples:

```text
kind:images New York City
kind:pdf assignment
kind:folder project
```

## 8.4 Supported kind keywords

Spotlight supports item-kind filters including:

### Applications

```text
kind:application
kind:applications
kind:app
```

### Contacts

```text
kind:contact
kind:contacts
```

### Folders

```text
kind:folder
kind:folders
```

### Email

```text
kind:email
kind:emails
kind:mail message
kind:mail messages
```

### Calendar events

```text
kind:event
kind:events
```

### Reminders

```text
kind:reminder
kind:reminders
```

### Images

```text
kind:image
kind:images
```

### Movies

```text
kind:movie
kind:movies
```

### Music

```text
kind:music
```

### Audio

```text
kind:audio
```

### PDFs

```text
kind:pdf
kind:pdfs
```

### Settings

```text
kind:system settings
kind:settings
```

### Bookmarks

```text
kind:bookmark
kind:bookmarks
```

### Fonts

```text
kind:font
kind:fonts
```

### Presentations

```text
kind:presentation
kind:presentations
```

## 8.5 Other metadata keywords

Spotlight can also use metadata-like search terms such as:

```text
from:
to:
author:
with:
by:
tag:
title:
name:
keyword:
contains:
```

Examples:

```text
author:John
title:New York City
tag:university
name:report
contains:machine learning
```

## 8.6 Filter by storage location

Spotlight can narrow by storage location.

Example behavior:

```text
iCloud Drive [Tab] file name
```

This searches within the chosen storage source.

## Ubuntu clone requirement

Your project should support a simple query language:

```text
kind:pdf assignment
type:image logo
folder:Downloads report
app:Firefox tabs
title:project
contains:Nyquist
tag:important
```

Advanced version:

```text
source:documents kind:pdf contains:"machine learning"
modified:last-week
created:today
size:>10MB
```

---

# 9. Result actions

Spotlight is not only for finding things. It supports actions on results.

## 9.1 Open result

- Select result.
- Press `Return`.
- Or double-click the result.

## 9.2 Preview result

- Select result.
- Press `Space`.
- Opens Quick Look preview.

## 9.3 Show file path

- Select file result.
- Hold `Command`.
- Spotlight shows the file location/path.

## 9.4 Reveal in Finder

- `Command + R`
- Or `Command + double-click`
- Opens the file’s location in Finder.

## 9.5 Copy item

- Drag a file result to the desktop or Finder window.
- This copies/moves/places the item depending on Finder behavior.

## 9.6 Take action

- Select an action result.
- Fill required blanks if needed.
- Press `Return`.
- Spotlight runs the action.

## 9.7 Return from mistaken action

- Press `Esc` if an action was selected by mistake.
- Returns to normal search.

## Ubuntu clone requirement

Each result should support multiple actions:

```text
Enter       = open/run
Space       = preview
Ctrl+Enter  = reveal in file manager
Ctrl+C      = copy path or copy result
Alt+Enter   = show actions menu
Esc         = close or go back
```

---

# 10. Quick Look preview behavior

Spotlight integrates with macOS Quick Look.

## 10.1 Preview shortcut

- Select result.
- Press `Space`.

## 10.2 Previewable content

Likely previewable items include:

- Images
- PDFs
- Text files
- Documents
- Videos
- Audio files
- Presentations
- Some folders/files depending on system support

## 10.3 Purpose

Quick Look allows the user to check a file without fully opening it.

## Ubuntu clone requirement

Implement preview using:

- File thumbnails
- Text excerpt preview
- Image preview
- PDF preview
- Metadata preview
- “Open externally” fallback

On Ubuntu, this could be similar to GNOME Sushi.

---

# 11. Calculator features

Spotlight can act as a calculator.

## 11.1 Basic arithmetic

Examples:

```text
956*23.94
2020/15
10+25
45-12
8*7
```

## 11.2 Expected behavior

- User types expression.
- Result appears instantly.
- User can copy the answer.
- User does not need to open Calculator app.

## Ubuntu clone requirement

Support:

```text
+  -  *  /  %  ^  parentheses
sqrt()
sin()
cos()
tan()
log()
pi
e
```

Basic version can start with arithmetic only.

---

# 12. Conversion features

Spotlight supports direct conversions.

## 12.1 Currency conversion

Examples:

```text
£100
100 yen
300 krone in euros
100 USD to LKR
```

## 12.2 Temperature conversion

Examples:

```text
98.8F
32C
340K in F
```

## 12.3 Measurement conversion

Examples:

```text
25 lbs
54 yards
23 stone
32 ft to metres
```

Possible measurement groups:

- Length
- Weight/mass
- Area
- Volume
- Speed
- Energy
- Power
- Pressure
- Digital storage
- Temperature

## 12.4 World clock / time lookup

Examples:

```text
time in Paris
Japan local time
time in Tokyo
```

## Ubuntu clone requirement

Implement conversions in stages:

Stage 1:

```text
temperature
length
weight
time zones
```

Stage 2:

```text
currency using online API
area
volume
speed
digital storage
```

Stage 3:

```text
natural-language unit parsing
offline unit database
cached exchange rates
```

---

# 13. Web and internet features

Spotlight can show results or suggestions from the internet depending on settings.

## 13.1 Web suggestions

Possible behavior:

- Suggest web searches.
- Show related internet content.
- Provide general knowledge-style results.
- Offer search variations.

## 13.2 Related content setting

macOS has a setting to allow related content from Apple partners to appear when searching or looking up text, objects, and photos.

## 13.3 Browser/search handoff

Spotlight can pass the query to the web/browser when the answer is not local.

## Ubuntu clone requirement

Support:

```text
? query          → web search
g query          → Google search
w query          → Wikipedia search
yt query         → YouTube search
gh query         → GitHub search
maps query       → maps search
```

Also allow user-configurable search engines.

---

# 14. Clipboard history features

Recent Spotlight versions include clipboard history search.

## 14.1 Clipboard mode

Shortcut:

```text
Command + 4
```

## 14.2 Enable prompt

When using clipboard history for the first time, the system may ask the user to enable it.

## 14.3 Sensitive-information warning

macOS warns that sensitive information may appear in clipboard history.

## 14.4 Search clipboard history

The user can search for previously copied content.

## 14.5 Copy from clipboard history

The user can select an item and copy it again.

## 14.6 Paste elsewhere

After copying a clipboard item, the user can paste it into another app.

## 14.7 Clear clipboard history

Spotlight includes an option to clear clipboard history.

## 14.8 Clipboard Search setting

Clipboard search can be turned on or off in Spotlight settings.

## Ubuntu clone requirement

Implement carefully:

- Store text clipboard history
- Optionally store file clipboard history
- Do not store passwords by default if possible
- Allow user to clear history
- Allow user to disable clipboard indexing
- Add maximum item limit
- Add expiry time, such as 7 days or 30 days
- Add privacy warning
- Add “ignore copied content from password managers” if possible

---

# 15. Actions and Shortcuts features

Newer Spotlight versions can perform actions directly.

## 15.1 Action search

User can search actions like normal results.

## 15.2 Actions browse mode

Shortcut:

```text
Command + 3
```

## 15.3 Built-in/system actions

Examples from Apple’s documentation include:

- Add File to Note
- Change Case
- Start FaceTime Call
- Random Number
- Recognize Music
- Remove Image Background
- Send Email
- Translate Text

## 15.4 App actions

Spotlight can expose actions from supported apps.

Possible examples:

- Send a message
- Create an event
- Start a call
- Open a specific app feature
- Run an app command

## 15.5 Shortcuts integration

Spotlight can run Shortcuts.

Possible behavior:

- Search for a Shortcut
- Run it directly
- Ask for input if needed
- Return result

## 15.6 Fill-in-the-blanks workflow

Some actions require user inputs.

Example structure:

```text
Send Email → recipient → subject/body → run
```

## 15.7 Press Return to execute

After required inputs are filled, pressing `Return` runs the action.

## 15.8 Escape to cancel/go back

If the user selected an action by mistake, `Esc` returns to search.

## Ubuntu clone requirement

Implement an action/plugin system.

Possible Linux actions:

```text
Open app
Open folder
Open URL
Run shell command
Create text note
Create file
Search web
Send desktop notification
Lock screen
Restart/logout prompt
Change volume
Toggle Wi-Fi
Open terminal here
Calculate expression
Convert unit
Copy file path
Rename file
Move file
Compress file
Extract archive
```

Important safety rule:

- Ask confirmation before destructive actions like delete, shutdown, moving many files, or running privileged commands.

---

# 16. Quick Keys

Quick Keys are shortcuts inside Spotlight for actions.

## 16.1 Purpose

Quick Keys let users run frequent actions with a few characters.

Example:

```text
ft Ashley
```

This could run a FaceTime action with Ashley as the contact.

## 16.2 Assign Quick Key

User can assign a quick key to an action.

## 16.3 Edit Quick Key

User can edit an existing quick key.

## 16.4 Automatic suggestions

Spotlight may automatically suggest quick keys after the user runs an action once.

## 16.5 Reset Quick Keys

Spotlight settings include an option to reset quick keys to defaults.

## Ubuntu clone requirement

Implement alias commands:

```text
gg query      → Google search
yt query      → YouTube search
gh query      → GitHub search
term          → Open Terminal
code folder   → Open folder in VS Code
calc expr     → Calculator
note text     → Create quick note
```

Allow user-defined quick keys in a config file:

```json
{
  "yt": "https://www.youtube.com/results?search_query={query}",
  "gh": "https://github.com/search?q={query}",
  "docs": "open ~/Documents"
}
```

---

# 17. Siri integration

Spotlight can hand a typed request to Siri.

## 17.1 Ask Siri from Spotlight

User types a request and can choose “Ask Siri” from results.

## 17.2 Purpose

This lets the user ask assistant-style queries from the Spotlight interface.

## Ubuntu clone requirement

For Ubuntu, possible equivalents:

- Local AI assistant integration
- Command-line assistant
- Web search answer
- Offline knowledge assistant
- Shell command helper
- Optional LLM integration if hardware allows

For your current PC, a lightweight version is better:

```text
ask query → opens browser/search engine
ai query  → optional local/online assistant later
```

---

# 18. System Settings integration

Spotlight can open macOS settings pages.

## 18.1 Search setting names

Examples:

```text
wifi
bluetooth
display
keyboard
privacy
sound
spotlight
wallpaper
```

## 18.2 Open directly

Selecting the result opens the matching settings page.

## Ubuntu clone requirement

Support GNOME settings shortcuts:

```text
wifi          → gnome-control-center wifi
bluetooth     → gnome-control-center bluetooth
display       → gnome-control-center display
keyboard      → gnome-control-center keyboard
sound         → gnome-control-center sound
privacy       → gnome-control-center privacy
appearance    → gnome-control-center appearance
```

---

# 19. File location and Finder integration

## 19.1 Show path

Holding `Command` shows the selected file’s path/location.

## 19.2 Reveal in Finder

`Command + R` reveals the selected file in Finder.

## 19.3 Drag/copy file result

The user can drag a file result to the desktop or Finder.

## 19.4 Finder search mode

`Option + Command + Space` opens Finder with the search field active.

## Ubuntu clone requirement

Implement file actions:

```text
Open
Open with...
Reveal in Files
Copy path
Copy file
Copy filename
Open terminal here
Show properties
```

---

# 20. Indexing behavior

Spotlight relies on indexing to make search fast.

## 20.1 Index local content

Spotlight indexes local searchable content so it can return results quickly.

## 20.2 App and system content

Spotlight can include content from apps and system categories if enabled.

## 20.3 Rebuild index

Apple provides a way to rebuild the Spotlight index when search results are wrong or missing.

## 20.4 Exclude locations

Users can exclude specific files, folders, or disks from Spotlight searches using Search Privacy.

## Ubuntu clone requirement

You need an indexing system:

### Minimum index

- Apps from `.desktop` files
- Files from selected folders
- Recent files
- Basic metadata

### Advanced index

- File content
- PDF text
- Document text
- Image metadata
- Browser bookmarks
- Clipboard history
- User-defined tags

### Indexing controls

- Include folders
- Exclude folders
- Rebuild index
- Pause indexing
- Index only when idle
- Limit index size
- Show indexing status

Important folders to exclude by default:

```text
node_modules
.git
.cache
venv
target
build
dist
```

This is especially important on your 120GB SSD.

---

# 21. Privacy and settings features

Spotlight has privacy-related controls.

## 21.1 Result category toggles

Users can turn categories on or off.

Main groups include:

- Results from Apps
- Results from System
- Clipboard Search

## 21.2 Search Privacy

Users can exclude specific files/folders/disks from Spotlight search.

## 21.3 Clipboard Search toggle

Users can disable clipboard content appearing in Spotlight.

## 21.4 Delete Spotlight Search History

Users can delete history associated with Spotlight search.

## 21.5 Help Apple Improve Search

There is a setting related to helping improve Search by allowing Apple to store certain Safari, Siri, Spotlight, Lookup, and image-search queries in a non-identifying way.

## 21.6 About Search & Privacy

Spotlight settings include privacy information explaining how search data may be processed and controlled.

## Ubuntu clone requirement

Privacy controls should be clear:

```text
Enable/disable each source
Clear search history
Clear clipboard history
Clear index
Exclude folders
Disable web suggestions
Disable telemetry completely by default
Show exactly what is stored
Allow full uninstall cleanup
```

For a portfolio project, privacy-by-design would be a strong selling point.

---

# 22. Screen Time / restriction behavior

macOS Spotlight respects Screen Time restrictions.

## 22.1 Dimmed app icons

If an app is blocked by downtime or time limits, its icon can appear dimmed.

## 22.2 Hourglass indicator

Spotlight can show an hourglass-style indicator when access is restricted.

## Ubuntu clone requirement

Linux equivalent could be:

- Show unavailable/blocked apps differently
- Respect parental-control or user-defined restrictions
- Respect disabled plugins
- Respect missing permissions

This is optional for your first version.

---

# 23. Keyboard shortcuts summary

## 23.1 General Spotlight shortcuts

| Action | macOS Shortcut |
|---|---|
| Open/close Spotlight | `Command + Space` |
| Open result | `Return` |
| Quick Look preview | `Space` |
| Move to next result | `Down Arrow` |
| Move to previous result | `Up Arrow` |
| Show path of selected file | Hold `Command` |
| Reveal file in Finder | `Command + R` or `Command + Double-click` |
| Open Finder search | `Option + Command + Space` |

## 23.2 Browse mode shortcuts

| Action | macOS Shortcut |
|---|---|
| Search Applications | `Command + 1` |
| Search Files | `Command + 2` |
| Search Actions | `Command + 3` |
| Search Clipboard | `Command + 4` |
| Search within a specific app | Type app name, then press `Tab` |

## Ubuntu clone suggested shortcuts

| Action | Suggested Linux Shortcut |
|---|---|
| Open/close launcher | `Alt + Space` |
| Open result | `Enter` |
| Preview | `Space` |
| Next result | `Down` |
| Previous result | `Up` |
| Reveal in file manager | `Ctrl + R` |
| Copy path/result | `Ctrl + C` |
| Open actions menu | `Alt + Enter` |
| Applications mode | `Ctrl + 1` |
| Files mode | `Ctrl + 2` |
| Actions mode | `Ctrl + 3` |
| Clipboard mode | `Ctrl + 4` |
| Close/back | `Esc` |

---

# 24. Result ranking behavior

Apple does not expose all ranking internals, but observable Spotlight behavior suggests that good ranking should consider:

## 24.1 Exact match

Exact app/file name match should rank high.

## 24.2 Prefix match

Typing the beginning of an app/file should rank high.

## 24.3 Fuzzy match

Small typing mistakes should still find results.

## 24.4 Recency

Recently opened files/apps should rank higher.

## 24.5 Frequency

Frequently used apps/actions should rank higher.

## 24.6 Category priority

Apps often appear high when query matches an app name.

## 24.7 Context

If user is in a mode like Files, file results should rank above app results.

## 24.8 User selection learning

If the user repeatedly selects the second result for a query, the system can learn to promote it.

## Ubuntu clone requirement

Suggested scoring formula:

```text
score =
  exact_match_score
+ prefix_match_score
+ fuzzy_match_score
+ recency_score
+ frequency_score
+ category_weight
+ user_preference_score
- penalty_for_hidden_or_excluded_items
```

---

# 25. Result display details

A good Spotlight-like result item should show:

- Icon
- Title
- Subtitle
- Category/source
- Path or short description
- Preview snippet
- Keyboard hint
- Action hint
- Matched text highlight
- File size/date for files
- App name for app-owned content
- Status indicator if unavailable

Examples:

```text
[PDF icon] IE3054 Assignment.pdf
Documents/University/DSP · PDF · Modified yesterday
```

```text
[Settings icon] Bluetooth
System Settings
```

```text
[Calculator icon] 25 * 4
= 100
```

---

# 26. Advanced result preview panel

Spotlight can preview selected items. For a Linux clone, preview panel can show:

## 26.1 Files

- Thumbnail
- File type
- Path
- Size
- Modified date
- Created date
- Owner/permissions
- Open/reveal actions

## 26.2 Documents

- Text snippet
- Page count if available
- Matching terms
- PDF thumbnail if possible

## 26.3 Images

- Image preview
- Dimensions
- File size
- Format

## 26.4 Apps

- App icon
- Description
- Command
- Category
- Last opened time

## 26.5 Calculator

- Expression
- Result
- Copy result action

## 26.6 Web search

- Search engine
- Query
- Open in browser action

## 26.7 Clipboard

- Clipboard text preview
- Copied time
- Copy again action
- Delete item action

---

# 27. Features to clone for minimum version

For your first Ubuntu version, implement:

## MVP 1: Launcher

- Global shortcut
- Centered window
- Search installed apps
- Open apps
- Keyboard navigation
- Enter to launch
- Esc to close
- App icons

## MVP 2: File search

- Search files in selected folders
- Open files
- Reveal in file manager
- Copy path
- Exclude folders
- Rebuild index

## MVP 3: Calculator and web

- Basic arithmetic
- Unit conversion basics
- Web search shortcuts
- Browser opening

## MVP 4: Clipboard

- Clipboard history
- Search clipboard
- Copy old item
- Clear history
- Disable clipboard tracking

## MVP 5: Actions

- Open settings pages
- Run safe shell commands
- User-defined quick keys
- Plugin architecture

---

# 28. Feature priority table for Ubuntu project

| Priority | Feature | Why it matters |
|---|---|---|
| High | Global shortcut | Core Spotlight feeling |
| High | App search | Most common use case |
| High | File search | Makes it useful beyond launcher |
| High | Fast ranking | Determines quality of experience |
| High | Keyboard navigation | Spotlight is keyboard-first |
| Medium | Calculator | Useful and easy to demo |
| Medium | Web search shortcuts | Useful daily feature |
| Medium | Preview panel | Makes it feel premium |
| Medium | Clipboard history | Powerful but privacy-sensitive |
| Medium | Settings search | OS integration |
| Low | Deep document content indexing | More complex |
| Low | Actions/automation | Advanced portfolio feature |
| Low | AI assistant integration | Optional and hardware-sensitive |
| Low | Fancy UI blur | Nice but not core |

---

# 29. Performance considerations for your Ubuntu PC

Your Linux machine specs:

```text
CPU: Intel Core i3-3240
RAM: 8GB
Storage: 120GB SSD
OS: Ubuntu
```

Recommended design:

## 29.1 Keep the UI lightweight

Avoid heavy animations and large background blur in the first version.

## 29.2 Index selected folders only

Start with:

```text
Desktop
Documents
Downloads
Pictures
Projects
```

Avoid indexing the full disk.

## 29.3 Exclude heavy developer folders

Exclude:

```text
node_modules
.git
.cache
venv
build
dist
target
vendor
```

## 29.4 Use background indexing carefully

Index when:

- System is idle
- User requests rebuild
- File changes are detected

## 29.5 Use SQLite for index storage

SQLite is a good first database for a local search index.

## 29.6 Use async search

The UI should not freeze while searching.

---

# 30. Possible Linux technology stack

## 30.1 UI options

### Python

- PyQt6
- PySide6
- GTK4 with Python

### JavaScript/TypeScript

- Electron
- Tauri with frontend framework

### Rust

- Tauri
- GTK-rs

### C/C++

- GTK
- Qt

## 30.2 Recommended for your portfolio

Best balance:

```text
Python + PyQt6 + SQLite
```

or

```text
Tauri + React + Rust + SQLite
```

For learning and fast development:

```text
Python + PyQt6
```

For stronger portfolio impact:

```text
Tauri + React + Rust
```

---

# 31. Suggested architecture for your Ubuntu Spotlight clone

```text
spotlight-clone/
│
├── app/
│   ├── main.py
│   ├── ui/
│   │   ├── launcher_window.py
│   │   ├── result_item.py
│   │   └── preview_panel.py
│   │
│   ├── search/
│   │   ├── search_engine.py
│   │   ├── ranking.py
│   │   ├── query_parser.py
│   │   └── indexer.py
│   │
│   ├── sources/
│   │   ├── apps_source.py
│   │   ├── files_source.py
│   │   ├── settings_source.py
│   │   ├── calculator_source.py
│   │   ├── web_source.py
│   │   ├── clipboard_source.py
│   │   └── actions_source.py
│   │
│   ├── actions/
│   │   ├── open_action.py
│   │   ├── reveal_action.py
│   │   ├── copy_action.py
│   │   └── command_action.py
│   │
│   ├── database/
│   │   ├── schema.sql
│   │   └── db.py
│   │
│   └── config/
│       ├── settings.json
│       └── quick_keys.json
│
├── docs/
│   ├── feature_inventory.md
│   ├── architecture.md
│   └── roadmap.md
│
└── README.md
```

---

# 32. Suggested feature checklist

## Core launcher

- [ ] Global shortcut
- [ ] Floating centered window
- [ ] Search field autofocus
- [ ] Results update while typing
- [ ] Keyboard navigation
- [ ] Enter opens result
- [ ] Esc closes
- [ ] Result icons
- [ ] Result subtitles
- [ ] Search history

## App search

- [ ] Read `.desktop` files
- [ ] Extract app names
- [ ] Extract icons
- [ ] Extract launch commands
- [ ] Launch apps
- [ ] Rank frequent apps higher

## File search

- [ ] Index selected folders
- [ ] Search filenames
- [ ] Search folders
- [ ] Open files
- [ ] Reveal in file manager
- [ ] Copy path
- [ ] Exclude folders
- [ ] Rebuild index

## Calculator/converter

- [ ] Basic arithmetic
- [ ] Copy answer
- [ ] Temperature conversion
- [ ] Length conversion
- [ ] Weight conversion
- [ ] Time zone lookup
- [ ] Currency conversion later

## Web

- [ ] Default web search
- [ ] Custom web shortcuts
- [ ] User-configurable search engines
- [ ] Open result in browser

## Clipboard

- [ ] Track text clipboard
- [ ] Search clipboard history
- [ ] Copy item again
- [ ] Clear clipboard history
- [ ] Disable clipboard search
- [ ] Ignore sensitive content where possible

## Actions

- [ ] Open settings pages
- [ ] Run user-defined quick keys
- [ ] Safe command actions
- [ ] Confirmation for risky actions
- [ ] Plugin system

## Settings

- [ ] Enable/disable result sources
- [ ] Configure shortcut
- [ ] Configure indexed folders
- [ ] Configure excluded folders
- [ ] Clear history
- [ ] Clear index
- [ ] Theme settings
- [ ] Privacy settings

---

# 33. What makes Spotlight feel “Spotlight-like”

To make your Ubuntu version feel close to macOS Spotlight, focus on these details:

1. It opens instantly.
2. The search field is already focused.
3. Results appear while typing.
4. Keyboard control feels natural.
5. The best result is usually correct.
6. The UI is minimal and clean.
7. It can open apps quickly.
8. It can find files quickly.
9. It can preview files without fully opening them.
10. It can calculate and convert units.
11. It can hand off to web search.
12. It remembers recent usage.
13. It lets the user act, not just search.
14. It respects privacy.
15. It lets the user configure what appears in results.

---

# 34. Features not necessary for first version

Avoid these at the start:

- Full AI assistant
- Full email indexing
- Full browser history indexing
- Heavy animated UI
- Full document OCR
- Full cloud-drive integration
- Complex plugin marketplace
- Destructive file actions
- Root/admin actions
- Deep semantic search

These are good for advanced versions, not the MVP.

---

# 35. Suggested project roadmap

## Version 0.1 — App launcher

Goal:

```text
Press shortcut → search app → open app
```

Features:

- Global shortcut
- App search
- Keyboard navigation
- Launch apps

## Version 0.2 — File search

Goal:

```text
Search local files and open/reveal them
```

Features:

- Index selected folders
- File search
- Folder search
- Reveal in Files
- Copy path

## Version 0.3 — Productivity tools

Goal:

```text
Use the launcher as a daily utility
```

Features:

- Calculator
- Web shortcuts
- Settings search
- Search history

## Version 0.4 — Clipboard and privacy

Goal:

```text
Clipboard history like modern Spotlight
```

Features:

- Clipboard tracking
- Clipboard search
- Clear history
- Privacy controls

## Version 0.5 — Actions and quick keys

Goal:

```text
Run useful tasks directly
```

Features:

- User-defined commands
- Quick keys
- Safe actions
- Plugin-like source structure

## Version 1.0 — Portfolio-ready release

Goal:

```text
A clean Linux Spotlight alternative for Ubuntu
```

Features:

- Polished UI
- README
- Screenshots
- Demo video
- Install script
- System tray/autostart
- Config screen
- Basic documentation

---

# 36. Naming ideas for your project

Possible project names:

- LunaSearch
- Linlight
- Ubuntu Spotlight
- NovaSearch
- QuickBeam
- Sanuk Search
- Penguinlight
- FocusFind
- DashLight
- OpenSpot

For portfolio, a polished name helps.

Example:

```text
LunaSearch — A Spotlight-inspired command palette for Ubuntu Linux
```

---

# 37. README positioning for GitHub

Use this kind of description:

```text
LunaSearch is a Spotlight-inspired desktop search and command launcher for Ubuntu Linux.
It allows users to launch apps, search files, run quick calculations, perform web searches,
browse clipboard history, and execute safe system actions from a single keyboard-first interface.
```

Strong portfolio points:

- Linux desktop integration
- Search indexing
- Ranking algorithm
- Plugin architecture
- UI/UX design
- Privacy-first clipboard handling
- SQLite storage
- Keyboard shortcuts
- Performance tuning for low-resource machines

---

# 38. Source notes

This document was prepared from public Apple documentation and support pages about Spotlight. Main sources used:

1. Apple Mac User Guide — Search for anything with Spotlight on Mac  
   https://support.apple.com/guide/mac-help/search-with-spotlight-mchlp1008/mac

2. Apple Mac User Guide — Narrow your search results in Spotlight on Mac  
   https://support.apple.com/guide/mac-help/narrow-your-search-results-in-spotlight-mchl4d69efd3/mac

3. Apple Mac User Guide — Take actions and shortcuts in Spotlight on Mac  
   https://support.apple.com/guide/mac-help/take-actions-and-shortcuts-in-spotlight-mchl4953dfeb/mac

4. Apple Mac User Guide — Search your Clipboard history in Spotlight on Mac  
   https://support.apple.com/guide/mac-help/search-your-clipboard-history-mchl40d5b86b/mac

5. Apple Mac User Guide — Spotlight keyboard shortcuts on Mac  
   https://support.apple.com/guide/mac-help/spotlight-keyboard-shortcuts-mh26783/mac

6. Apple Mac User Guide — Spotlight settings on Mac  
   https://support.apple.com/guide/mac-help/spotlight-settings-on-mac-mchl54d95e8a/mac

7. Apple Mac User Guide — Get calculations and conversions in Spotlight on Mac  
   https://support.apple.com/guide/mac-help/get-calculations-and-conversions-in-spotlight-mchldd6ba066/mac

8. Apple Support — What’s new in macOS Tahoe 26  
   https://support.apple.com/122868

9. Apple Support — Rebuild the Spotlight index on your Mac  
   https://support.apple.com/102321

---

# 39. Final product vision for your Ubuntu project

The goal is not to copy macOS visually only. The real goal is to recreate the **workflow power** of Spotlight:

```text
One shortcut.
One search box.
Everything searchable.
Everything actionable.
Fast enough to become a daily habit.
```

For your Ubuntu machine, this project can become a very strong portfolio project because it touches:

- Operating-system integration
- UI/UX
- File indexing
- Search algorithms
- Ranking systems
- Keyboard shortcuts
- Databases
- Privacy
- Linux desktop behavior
- Automation
- Real-world usability

This is much stronger than a normal CRUD app because it shows that you can build tools close to the operating-system level.