use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
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
mod search_history;
mod search_result;
mod settings;
mod settings_search;
mod web_shortcuts;

use app_discovery::AppCatalog;
use app_launch::LaunchResult;
use file_actions::ValidatedPath;
use file_index::FileIndex;
use search_history::SearchHistory;
use search_result::{SearchResult, SearchSource};
use settings_search::PreparedSettingCommand;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

type FileIndexState = Arc<RwLock<FileIndex>>;
type SearchHistoryState = Arc<RwLock<SearchHistory>>;

const LAUNCHER_WINDOW_LABEL: &str = "main";
const LAUNCHER_SHOWN_EVENT: &str = "launcher:shown";
const TOGGLE_ARG: &str = "toggle";
const SEARCH_HISTORY_FILE_NAME: &str = "search-history.json";
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

fn validate_copy_text(text: &str) -> Result<&str, String> {
    if text.trim().is_empty() {
        return Err("Text is required".to_owned());
    }

    Ok(text)
}

fn history_file_path_from_data_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(SEARCH_HISTORY_FILE_NAME)
}

fn search_history_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|data_dir| history_file_path_from_data_dir(&data_dir))
        .map_err(|error| error.to_string())
}

fn fallback_search_history_path() -> PathBuf {
    std::env::temp_dir()
        .join("rat-search")
        .join(SEARCH_HISTORY_FILE_NAME)
}

fn load_search_history_state(app: &tauri::AppHandle) -> SearchHistoryState {
    let history_path = search_history_path(app).unwrap_or_else(|error| {
        eprintln!("failed to resolve search history path: {error}");
        fallback_search_history_path()
    });
    let history = SearchHistory::load(history_path);
    let entry_count = history.entries().len();

    dev_log(format!("loaded {entry_count} search history entries"));

    Arc::new(RwLock::new(history))
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn record_search_history_in_state(
    state: &SearchHistoryState,
    query: &str,
    now_ms: u64,
) -> Result<(), String> {
    if search_history::normalize_query(query).is_empty() {
        return Ok(());
    }

    let mut history = state
        .write()
        .map_err(|_| "Search history unavailable".to_owned())?;

    history.record_query_at(query, now_ms);
    history
        .save()
        .map_err(|_| "Could not save search history".to_owned())
}

fn prepare_setting_command(setting_id: &str) -> Result<PreparedSettingCommand, String> {
    settings_search::command_for_setting(setting_id).ok_or_else(|| "Could not open item".to_owned())
}

fn spawn_setting_command(command: &PreparedSettingCommand) -> Result<(), String> {
    Command::new(&command.program)
        .args(&command.args)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            eprintln!(
                "failed to open setting with '{} {}': {error}",
                command.program,
                command.args.join(" ")
            );
            "Could not open item".to_owned()
        })
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

#[tauri::command]
fn copy_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
    let text = validate_copy_text(&text)?;

    app.clipboard()
        .write_text(text.to_owned())
        .map_err(|error| {
            eprintln!("failed to copy text: {error}");
            "Could not complete action".to_owned()
        })?;

    hide_launcher_for_app(&app)
}

#[tauri::command]
fn open_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    if !web_shortcuts::is_allowed_url(&url) {
        return Err("Could not open item".to_owned());
    }

    app.opener().open_url(url, None::<&str>).map_err(|error| {
        eprintln!("failed to open url: {error}");
        "Could not open item".to_owned()
    })?;

    hide_launcher_for_app(&app)
}

#[tauri::command]
fn open_setting(app: tauri::AppHandle, setting_id: String) -> Result<(), String> {
    let command = prepare_setting_command(&setting_id)?;

    spawn_setting_command(&command)?;

    hide_launcher_for_app(&app)
}

#[tauri::command]
fn record_search_history(
    history: tauri::State<'_, SearchHistoryState>,
    query: String,
) -> Result<(), String> {
    record_search_history_in_state(&history, &query, current_time_ms())
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
    history: Option<&SearchHistory>,
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    let limit = settings::normalize_result_limit(limit);
    let mut results = app_search::search_apps(catalog, query, limit);

    if let Some(file_index) = file_index {
        results.extend(file_search::search_files(file_index, query, limit));
    }

    results.extend(calculator::search_calculator(query, limit));
    results.extend(web_shortcuts::search_web_shortcuts(query, limit));
    results.extend(settings_search::search_settings(query, limit));

    if let Some(history) = history {
        results.extend(search_history::search_history(history, query, limit));
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
    history: tauri::State<'_, SearchHistoryState>,
    query: String,
    limit: usize,
) -> Vec<SearchResult> {
    let file_index_guard = match file_index.read() {
        Ok(index) => Some(index),
        Err(error) => {
            eprintln!("failed to read file index for search: {error}");
            None
        }
    };
    let history_guard = match history.read() {
        Ok(history) => Some(history),
        Err(error) => {
            eprintln!("failed to read search history for search: {error}");
            None
        }
    };

    search_all(
        &catalog,
        file_index_guard.as_deref(),
        history_guard.as_deref(),
        &query,
        limit,
    )
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
            copy_text,
            hide_launcher,
            launch_app,
            open_path,
            open_setting,
            open_url,
            record_search_history,
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

            let search_history = load_search_history_state(app.handle());
            app.manage(search_history);

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
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{
        app_discovery::{AppCatalog, AppRecord},
        file_index::{FileIndex, FileRecord},
        search_history::SearchHistory,
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

    fn history(entries: &[(&str, u64, u32)]) -> SearchHistory {
        let mut history = SearchHistory::load(PathBuf::from("/tmp/history.json"));

        for (query, last_used_ms, use_count) in entries {
            for offset in 0..*use_count {
                history.record_query_at(query, last_used_ms + u64::from(offset));
            }
        }

        history
    }

    fn temporary_directory(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("rat-search-{name}-{}-{unique}", std::process::id()));

        fs::create_dir_all(&path).expect("temporary directory should be created");

        path
    }

    fn history_state(path: PathBuf) -> SearchHistoryState {
        Arc::new(RwLock::new(SearchHistory::load(path)))
    }

    #[test]
    fn history_file_path_from_data_dir_appends_history_file_name() {
        assert_eq!(
            history_file_path_from_data_dir(Path::new("/tmp/rat-search-data")),
            PathBuf::from("/tmp/rat-search-data").join(SEARCH_HISTORY_FILE_NAME)
        );
    }

    #[test]
    fn copy_text_validation_rejects_empty_text() {
        assert_eq!(
            validate_copy_text("   ").expect_err("empty copy text should be rejected"),
            "Text is required"
        );
        assert_eq!(
            validate_copy_text(" calculator result ").expect("text should validate"),
            " calculator result "
        );
    }

    #[test]
    fn prepare_setting_command_resolves_known_ids_and_rejects_unknown_ids() {
        assert_eq!(
            prepare_setting_command("wifi").expect("wifi setting should resolve"),
            PreparedSettingCommand {
                program: "gnome-control-center".to_owned(),
                args: vec!["wifi".to_owned()],
            }
        );
        assert_eq!(
            prepare_setting_command("definitely-not-a-setting")
                .expect_err("unknown setting should be rejected"),
            "Could not open item"
        );
    }

    #[test]
    fn record_search_history_in_state_ignores_empty_queries() {
        let root = temporary_directory("empty-history-record");
        let path = root.join("history.json");
        let state = history_state(path.clone());

        record_search_history_in_state(&state, "   ", 1_700)
            .expect("empty query should be ignored");

        assert!(
            !path.exists(),
            "empty history query should not create a history file"
        );
        assert!(state.read().expect("history lock").entries().is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn record_search_history_in_state_records_and_persists_query() {
        let root = temporary_directory("persist-history-record");
        let path = root.join("history.json");
        let state = history_state(path.clone());

        record_search_history_in_state(&state, " wifi ", 1_700).expect("history should record");
        let loaded = SearchHistory::load(path);

        assert_eq!(loaded.entries()[0].query, "wifi");
        assert_eq!(loaded.entries()[0].last_used_ms, 1_700);
        assert_eq!(loaded.entries()[0].use_count, 1);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn record_search_history_in_state_reports_save_failures() {
        let root = temporary_directory("history-save-failure");
        let path = root.join("history.json");
        fs::create_dir(&path).expect("directory should block file save");
        let state = history_state(path);

        let error = record_search_history_in_state(&state, "wifi", 1_700)
            .expect_err("history save should fail");

        assert_eq!(error, "Could not save search history");

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn mixed_search_keeps_strong_app_above_noisy_file_match() {
        let app_catalog = catalog(vec![app("report.desktop", "Report")]);
        let file_index = index(vec![file("/home/sanuk/Documents/Annual Report.pdf")]);

        let results = search_all(&app_catalog, Some(&file_index), None, "report", 8);

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

        let results = search_all(&app_catalog, Some(&file_index), None, "report", 1);

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

        let results = search_all(&app_catalog, Some(&file_index), None, "alpha", 8);

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

        let results = search_all(&app_catalog, None, None, "settings", 8);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SearchSource::Applications);
    }

    #[test]
    fn mixed_search_includes_calculator_above_weak_file_matches() {
        let app_catalog = AppCatalog::default();
        let file_index = index(vec![file("/home/sanuk/Documents/2 plus 2 notes.txt")]);

        let results = search_all(&app_catalog, Some(&file_index), None, "2+2", 8);

        assert_eq!(results[0].source, SearchSource::Calculator);
        assert_eq!(results[0].title, "4");
    }

    #[test]
    fn mixed_search_keeps_exact_app_above_web_and_history_results() {
        let app_catalog = catalog(vec![app("g-rust.desktop", "g rust")]);
        let history = history(&[("g rust", 1_700, 10)]);

        let results = search_all(&app_catalog, None, Some(&history), "g rust", 8);

        assert_eq!(results[0].source, SearchSource::Applications);
        assert_eq!(results[0].title, "g rust");
        assert!(results
            .iter()
            .any(|result| result.source == SearchSource::Web));
        assert!(results
            .iter()
            .any(|result| result.source == SearchSource::History));
    }

    #[test]
    fn mixed_search_includes_settings_exact_match() {
        let app_catalog = AppCatalog::default();

        let results = search_all(&app_catalog, None, None, "wifi", 8);

        assert_eq!(results[0].source, SearchSource::Settings);
        assert_eq!(results[0].title, "Wi-Fi");
    }

    #[test]
    fn mixed_search_includes_explicit_web_shortcuts() {
        let app_catalog = AppCatalog::default();

        let results = search_all(&app_catalog, None, None, "g rust tauri", 8);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SearchSource::Web);
        assert_eq!(results[0].title, "Search Google");
    }

    #[test]
    fn mixed_search_keeps_history_below_strong_live_results() {
        let app_catalog = AppCatalog::default();
        let history = history(&[("wifi troubleshooting", 1_700, 20)]);

        let results = search_all(&app_catalog, None, Some(&history), "wifi", 8);

        assert_eq!(results[0].source, SearchSource::Settings);
        assert_eq!(results[0].title, "Wi-Fi");
        assert!(
            results
                .iter()
                .position(|result| result.source == SearchSource::History)
                .expect("history result should exist")
                > 0
        );
    }

    #[test]
    fn mixed_search_applies_final_limit_after_all_v0_3_sources() {
        let app_catalog = catalog(vec![app("wifi.desktop", "WiFi Utility")]);
        let history = history(&[("wifi history", 1_700, 3)]);

        let results = search_all(&app_catalog, None, Some(&history), "wifi", 2);

        assert_eq!(results.len(), 2);
        assert_eq!(
            results
                .iter()
                .map(|result| result.source.clone())
                .collect::<Vec<_>>(),
            [SearchSource::Applications, SearchSource::Settings]
        );
    }

    #[test]
    fn mixed_search_falls_back_when_history_is_absent() {
        let app_catalog = AppCatalog::default();

        let results = search_all(&app_catalog, None, None, "wifi", 8);

        assert!(results
            .iter()
            .any(|result| result.source == SearchSource::Settings));
        assert!(!results
            .iter()
            .any(|result| result.source == SearchSource::History));
    }
}
