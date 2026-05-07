use std::{
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow};

mod app_discovery;
mod app_icons;
mod app_launch;
mod app_search;
mod calculator;
mod file_actions;
mod file_icons;
mod file_index;
mod file_search;
mod search_result;
mod settings;
mod settings_search;
mod web_shortcuts;

use app_discovery::AppCatalog;
use app_launch::LaunchResult;
use file_actions::ValidatedPath;
use file_index::FileIndex;
use search_result::{SearchResult, SearchSource};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

type FileIndexState = Arc<RwLock<FileIndex>>;

const LAUNCHER_WINDOW_LABEL: &str = "main";
const LAUNCHER_SHOWN_EVENT: &str = "launcher:shown";
const TOGGLE_ARG: &str = "toggle";
const DELAYED_CENTER_MS: u64 = 80;
const LAUNCHER_VERTICAL_PERCENT: u32 = 25;
const LAUNCHER_SEARCH_ROW_CENTER_OFFSET: i32 = 42;

#[cfg(debug_assertions)]
fn dev_log(message: impl AsRef<str>) {
    eprintln!("[rat-search] {}", message.as_ref());
}

#[cfg(not(debug_assertions))]
fn dev_log(_message: impl AsRef<str>) {}

#[cfg(target_os = "linux")]
fn initialize_linux_window_backend() {
    let session_type = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "<unset>".into());
    let current_desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "<unset>".into());
    let wayland_display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "<unset>".into());
    let display = std::env::var("DISPLAY").unwrap_or_else(|_| "<unset>".into());

    match std::env::var("GDK_BACKEND") {
        Ok(value) if !value.trim().is_empty() => {
            dev_log(format!("GDK_BACKEND already configured as '{value}'"));
        }
        _ => {
            std::env::set_var("GDK_BACKEND", "x11");
            dev_log("set GDK_BACKEND=x11 for reliable launcher positioning");
        }
    }

    let gdk_backend = std::env::var("GDK_BACKEND").unwrap_or_else(|_| "<unset>".into());
    dev_log(format!(
        "linux session: XDG_SESSION_TYPE={session_type}, XDG_CURRENT_DESKTOP={current_desktop}, WAYLAND_DISPLAY={wayland_display}, DISPLAY={display}, GDK_BACKEND={gdk_backend}"
    ));
}

#[cfg(not(target_os = "linux"))]
fn initialize_linux_window_backend() {}

fn position_launcher_with_reason(window: &WebviewWindow, reason: &str) -> tauri::Result<()> {
    let monitor = match window.primary_monitor()? {
        Some(monitor) => Some(monitor),
        None => window.current_monitor()?,
    };

    if let Some(monitor) = monitor {
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let window_size = window.outer_size()?;

        let x = monitor_position.x
            + ((monitor_size.width.saturating_sub(window_size.width)) / 2) as i32;
        let vertical_target = (monitor_size
            .height
            .saturating_mul(LAUNCHER_VERTICAL_PERCENT)
            / 100) as i32;
        let max_y = monitor_position.y
            + monitor_size
                .height
                .saturating_sub(window_size.height)
                .try_into()
                .unwrap_or(i32::MAX);
        let desired_y = monitor_position.y + vertical_target - LAUNCHER_SEARCH_ROW_CENTER_OFFSET;
        let y = desired_y.clamp(monitor_position.y, max_y.max(monitor_position.y));

        dev_log(format!(
            "{reason}: monitor=({}, {}) {}x{}, window={}x{}, vertical_target={LAUNCHER_VERTICAL_PERCENT}%, search_row_offset={}, target=({}, {})",
            monitor_position.x,
            monitor_position.y,
            monitor_size.width,
            monitor_size.height,
            window_size.width,
            window_size.height,
            LAUNCHER_SEARCH_ROW_CENTER_OFFSET,
            x,
            y
        ));

        window.set_position(Position::Physical(PhysicalPosition { x, y }))?;
        dev_log(format!("{reason}: position request succeeded"));
    } else {
        dev_log(format!("{reason}: no monitor available for positioning"));
    }

    Ok(())
}

fn position_launcher(window: &WebviewWindow) -> tauri::Result<()> {
    position_launcher_with_reason(window, "position")
}

fn set_launcher_size(window: &WebviewWindow, expanded: bool) -> tauri::Result<()> {
    let height = if expanded {
        settings::LAUNCHER_EXPANDED_HEIGHT
    } else {
        settings::LAUNCHER_COMPACT_HEIGHT
    };

    window.set_size(Size::Physical(PhysicalSize {
        width: settings::LAUNCHER_WINDOW_WIDTH,
        height,
    }))?;
    position_launcher_with_reason(window, if expanded { "expand" } else { "collapse" })
}

fn focus_launcher(window: &WebviewWindow) {
    if let Err(error) = window.set_focus() {
        eprintln!("failed to focus launcher window: {error}");
    }

    if let Err(error) = window.emit(LAUNCHER_SHOWN_EVENT, ()) {
        eprintln!("failed to emit {LAUNCHER_SHOWN_EVENT}: {error}");
    }
}

fn show_launcher(window: &WebviewWindow) -> tauri::Result<()> {
    set_launcher_size(window, false)?;
    position_launcher_with_reason(window, "before show")?;
    window.show()?;
    position_launcher_with_reason(window, "after show")?;

    let delayed_window = window.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(DELAYED_CENTER_MS));

        if !matches!(delayed_window.is_visible(), Ok(true)) {
            dev_log("delayed after show: launcher is no longer visible; skipping center/focus");
            return;
        }

        if let Err(error) = position_launcher_with_reason(&delayed_window, "delayed after show") {
            eprintln!("failed to position launcher window after show: {error}");
        }

        focus_launcher(&delayed_window);
    });

    Ok(())
}

fn hide_launcher_window(window: &WebviewWindow) -> tauri::Result<()> {
    window.hide()
}

fn hide_launcher_for_app(app: &tauri::AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window(LAUNCHER_WINDOW_LABEL) else {
        return Ok(());
    };

    hide_launcher_window(&window).map_err(|error| error.to_string())
}

fn toggle_launcher(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window(LAUNCHER_WINDOW_LABEL) else {
        eprintln!("launcher window '{LAUNCHER_WINDOW_LABEL}' was not found");
        return;
    };

    match window.is_visible() {
        Ok(true) => {
            if let Err(error) = hide_launcher_window(&window) {
                eprintln!("failed to hide launcher window: {error}");
            }
        }
        Ok(false) => {
            if let Err(error) = show_launcher(&window) {
                eprintln!("failed to show launcher window: {error}");
            }
        }
        Err(error) => eprintln!("failed to read launcher window visibility: {error}"),
    }
}

fn should_toggle_from_args(args: &[String]) -> bool {
    args.iter().any(|arg| arg == TOGGLE_ARG)
}

fn handle_cli_args(app: &tauri::AppHandle, args: &[String]) {
    if should_toggle_from_args(args) {
        toggle_launcher(app);
    }
}

fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|session_type| session_type.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn register_launcher_shortcut(app: &tauri::App) {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    if cfg!(target_os = "linux") && is_wayland_session() {
        eprintln!(
            "global shortcut registration skipped on Wayland; bind your desktop shortcut to `rat-search toggle`"
        );
        return;
    }

    let launcher_shortcut = Shortcut::new(
        Some(settings::DEFAULT_HOTKEY_MODIFIERS),
        settings::DEFAULT_HOTKEY_CODE,
    );
    let handler_shortcut = launcher_shortcut.clone();
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, shortcut, event| {
            if shortcut == &handler_shortcut && event.state() == ShortcutState::Pressed {
                toggle_launcher(app);
            }
        })
        .build();

    if let Err(error) = app.handle().plugin(plugin) {
        eprintln!("failed to initialize global shortcut plugin: {error}");
        return;
    }

    if let Err(error) = app.global_shortcut().register(launcher_shortcut) {
        eprintln!(
            "failed to register {} launcher shortcut: {error}",
            settings::DEFAULT_HOTKEY_LABEL
        );
    }
}

#[tauri::command]
fn close_launcher(app: tauri::AppHandle) -> Result<(), String> {
    hide_launcher_for_app(&app)
}

#[tauri::command]
fn hide_launcher(app: tauri::AppHandle) -> Result<(), String> {
    hide_launcher_for_app(&app)
}

fn validate_file_action_path(
    file_index: &FileIndexState,
    path: &str,
) -> Result<ValidatedPath, String> {
    let index = file_index
        .read()
        .map_err(|_| "File index unavailable".to_owned())?;

    file_actions::validate_indexed_path(&index, path)
}

fn path_for_opener(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

#[tauri::command]
fn open_path(
    app: tauri::AppHandle,
    file_index: tauri::State<'_, FileIndexState>,
    path: String,
) -> Result<(), String> {
    let validated_path = validate_file_action_path(&file_index, &path)?;

    app.opener()
        .open_path(path_for_opener(&validated_path.path), None::<&str>)
        .map_err(|error| {
            eprintln!(
                "failed to open path '{}': {error}",
                validated_path.path.display()
            );
            "Could not open item".to_owned()
        })?;

    hide_launcher_for_app(&app)
}

#[tauri::command]
fn reveal_path(
    app: tauri::AppHandle,
    file_index: tauri::State<'_, FileIndexState>,
    path: String,
) -> Result<(), String> {
    let validated_path = validate_file_action_path(&file_index, &path)?;
    let reveal_target = file_actions::reveal_target(&validated_path)?;

    app.opener()
        .open_path(path_for_opener(&reveal_target), None::<&str>)
        .map_err(|error| {
            eprintln!(
                "failed to reveal path '{}': {error}",
                reveal_target.display()
            );
            "Could not open item".to_owned()
        })?;

    hide_launcher_for_app(&app)
}

#[tauri::command]
fn copy_path(
    app: tauri::AppHandle,
    file_index: tauri::State<'_, FileIndexState>,
    path: String,
) -> Result<(), String> {
    let validated_path = validate_file_action_path(&file_index, &path)?;

    app.clipboard()
        .write_text(path_for_opener(&validated_path.path))
        .map_err(|error| {
            eprintln!(
                "failed to copy path '{}': {error}",
                validated_path.path.display()
            );
            "Could not complete action".to_owned()
        })?;

    hide_launcher_for_app(&app)
}

fn start_file_index_scan(file_index: FileIndexState) {
    thread::spawn(move || {
        let roots = file_index::default_index_roots();
        dev_log(format!(
            "file index scan: discovered {} default roots",
            roots.len()
        ));

        let scanned_index = file_index::scan_roots(&roots);
        let scanned_count = scanned_index.len();

        match file_index.write() {
            Ok(mut index) => {
                *index = scanned_index;
                dev_log(format!(
                    "file index scan: stored {scanned_count} file/folder records"
                ));
            }
            Err(error) => {
                eprintln!("failed to store file index scan results: {error}");
            }
        }
    });
}

fn search_all(
    catalog: &AppCatalog,
    file_index: Option<&FileIndex>,
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    let limit = settings::normalize_result_limit(limit);
    let mut results = app_search::search_apps(catalog, query, limit);

    if let Some(file_index) = file_index {
        results.extend(file_search::search_files(file_index, query, limit));
    }

    results.sort_by(compare_search_results);
    results.truncate(limit);
    results
}

fn compare_search_results(left: &SearchResult, right: &SearchResult) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| source_priority(&left.source).cmp(&source_priority(&right.source)))
        .then_with(|| normalize_search_text(&left.title).cmp(&normalize_search_text(&right.title)))
        .then_with(|| left.id.cmp(&right.id))
}

fn source_priority(source: &SearchSource) -> u8 {
    match source {
        SearchSource::Applications => 0,
        SearchSource::Calculator => 1,
        SearchSource::Settings => 2,
        SearchSource::Folders => 3,
        SearchSource::Files => 4,
        SearchSource::Web => 5,
        SearchSource::History => 6,
    }
}

fn normalize_search_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[tauri::command]
fn search(
    catalog: tauri::State<'_, AppCatalog>,
    file_index: tauri::State<'_, FileIndexState>,
    query: String,
    limit: usize,
) -> Vec<SearchResult> {
    match file_index.read() {
        Ok(index) => search_all(&catalog, Some(&index), &query, limit),
        Err(error) => {
            eprintln!("failed to read file index for search: {error}");
            search_all(&catalog, None, &query, limit)
        }
    }
}

#[tauri::command]
fn set_launcher_expanded(app: tauri::AppHandle, expanded: bool) -> Result<(), String> {
    let Some(window) = app.get_webview_window(LAUNCHER_WINDOW_LABEL) else {
        return Ok(());
    };

    set_launcher_size(&window, expanded).map_err(|error| {
        eprintln!("failed to set launcher expanded={expanded}: {error}");
        error.to_string()
    })
}

#[tauri::command]
fn launch_app(
    app: tauri::AppHandle,
    catalog: tauri::State<'_, AppCatalog>,
    app_id: String,
) -> Result<LaunchResult, String> {
    let result = app_launch::launch_app(&catalog, &app_id)?;

    hide_launcher_for_app(&app)?;

    Ok(result)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    initialize_linux_window_backend();

    let mut builder = tauri::Builder::default();

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            handle_cli_args(app, &args);
        }));
    }

    builder
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            close_launcher,
            copy_path,
            hide_launcher,
            launch_app,
            open_path,
            reveal_path,
            search,
            set_launcher_expanded
        ])
        .setup(|app| {
            let launch_args = std::env::args().collect::<Vec<_>>();
            let app_catalog = AppCatalog::scan();
            dev_log(format!(
                "defaults: hotkey={}, max_results={}, compact_window={}x{}, expanded_window={}x{}, theme={}, search_source={}",
                settings::DEFAULT_HOTKEY_LABEL,
                settings::DEFAULT_MAX_RESULTS,
                settings::LAUNCHER_WINDOW_WIDTH,
                settings::LAUNCHER_COMPACT_HEIGHT,
                settings::LAUNCHER_WINDOW_WIDTH,
                settings::LAUNCHER_EXPANDED_HEIGHT,
                settings::DEFAULT_THEME,
                settings::DEFAULT_SEARCH_SOURCE
            ));
            dev_log(format!("discovered {} applications", app_catalog.len()));
            app.manage(app_catalog);

            let file_index = Arc::new(RwLock::new(FileIndex::new()));
            app.manage(file_index.clone());
            start_file_index_scan(file_index);

            if let Some(window) = app.get_webview_window(LAUNCHER_WINDOW_LABEL) {
                position_launcher(&window)?;

                if should_toggle_from_args(&launch_args) {
                    show_launcher(&window)?;
                }
            }

            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            register_launcher_shortcut(app);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        app_discovery::{AppCatalog, AppRecord},
        file_index::{FileIndex, FileRecord},
        search_result::SearchSource,
    };

    use super::*;

    fn app(id: &str, name: &str) -> AppRecord {
        AppRecord {
            id: id.to_owned(),
            name: name.to_owned(),
            generic_name: None,
            comment: None,
            exec: name.to_lowercase(),
            icon: None,
            categories: Vec::new(),
            keywords: Vec::new(),
            desktop_file_path: format!("/tmp/{id}"),
            terminal: false,
        }
    }

    fn catalog(apps: Vec<AppRecord>) -> AppCatalog {
        AppCatalog { apps }
    }

    fn file(path: &str) -> FileRecord {
        FileRecord::new(PathBuf::from(path), false, None, Some(10))
    }

    fn folder(path: &str) -> FileRecord {
        FileRecord::new(PathBuf::from(path), true, None, None)
    }

    fn index(records: Vec<FileRecord>) -> FileIndex {
        FileIndex::from_records(records)
    }

    #[test]
    fn mixed_search_keeps_strong_app_above_noisy_file_match() {
        let app_catalog = catalog(vec![app("report.desktop", "Report")]);
        let file_index = index(vec![file("/home/sanuk/Documents/Annual Report.pdf")]);

        let results = search_all(&app_catalog, Some(&file_index), "report", 8);

        assert_eq!(results[0].source, SearchSource::Applications);
        assert_eq!(results[0].title, "Report");
        assert_eq!(results[1].source, SearchSource::Files);
    }

    #[test]
    fn mixed_search_applies_final_limit_after_merging_sources() {
        let app_catalog = catalog(vec![app("calendar.desktop", "Calendar")]);
        let file_index = index(vec![
            file("/home/sanuk/Documents/Report.pdf"),
            folder("/home/sanuk/Documents/Reports"),
        ]);

        let results = search_all(&app_catalog, Some(&file_index), "report", 1);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Reports");
        assert_eq!(results[0].source, SearchSource::Folders);
    }

    #[test]
    fn mixed_search_ties_sort_by_source_then_title_then_id() {
        let app_catalog = AppCatalog::default();
        let first_file = file("/home/sanuk/Documents/Alpha");
        let folder = folder("/home/sanuk/Documents/Alpha");
        let second_file = file("/home/sanuk/Documents/Beta");
        let file_index = index(vec![second_file, first_file, folder]);

        let results = search_all(&app_catalog, Some(&file_index), "alpha", 8);

        assert_eq!(
            results
                .iter()
                .map(|result| (result.source.clone(), result.title.as_str()))
                .collect::<Vec<_>>(),
            [
                (SearchSource::Folders, "Alpha"),
                (SearchSource::Files, "Alpha")
            ]
        );
    }

    #[test]
    fn mixed_search_falls_back_to_apps_without_file_index() {
        let app_catalog = catalog(vec![app("settings.desktop", "Settings")]);

        let results = search_all(&app_catalog, None, "settings", 8);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SearchSource::Applications);
    }
}
