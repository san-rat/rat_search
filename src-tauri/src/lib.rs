use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow};

mod app_discovery;
mod app_icons;
mod app_launch;
mod app_search;
mod calculator;
mod clipboard_history;
mod clipboard_settings;
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
use clipboard_history::{ClipboardHistory, ClipboardRecordOutcome};
use clipboard_settings::ClipboardSettings;
use file_actions::{PreferredOpen, ValidatedPath};
use file_index::FileIndex;
use search_history::SearchHistory;
use search_result::{SearchAction, SearchResult, SearchSource};
use settings_search::PreparedSettingCommand;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

type FileIndexState = Arc<RwLock<FileIndex>>;
type ClipboardHistoryState = Arc<RwLock<ClipboardHistory>>;
type ClipboardSettingsState = Arc<RwLock<ClipboardSettings>>;
type SearchHistoryState = Arc<RwLock<SearchHistory>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ClipboardPrivacyStatus {
    enabled: bool,
    entry_count: usize,
    retention_days: u32,
    max_entries: u32,
    max_text_bytes: u32,
}

const LAUNCHER_WINDOW_LABEL: &str = "main";
const LAUNCHER_SHOWN_EVENT: &str = "launcher:shown";
const TOGGLE_ARG: &str = "toggle";
const FOREGROUND_ARG: &str = "foreground";
const CLIPBOARD_HISTORY_FILE_NAME: &str = "clipboard-history.json";
const CLIPBOARD_SETTINGS_FILE_NAME: &str = "clipboard-settings.json";
const CLIPBOARD_POLL_INTERVAL_MS: u64 = 1_000;
const SEARCH_HISTORY_FILE_NAME: &str = "search-history.json";
const LAUNCHER_FOCUS_RETRY_MS: [u64; 4] = [50, 150, 300, 500];
const LAUNCHER_VERTICAL_PERCENT: u32 = 25;
const LAUNCHER_SEARCH_ROW_CENTER_OFFSET: i32 = 42;
const STARTUP_ID_ARG: &str = "--startup-id";
const XDG_ACTIVATION_TOKEN_ARG: &str = "--xdg-activation-token";
const LEGACY_GNOME_HOTKEY_COMMAND: &str = "rat-search toggle";
const GNOME_HOTKEY_COMMAND: &str = r#"/bin/sh -c 'exec rat-search foreground --startup-id "$DESKTOP_STARTUP_ID" --xdg-activation-token "$XDG_ACTIVATION_TOKEN"'"#;
const GNOME_MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const GNOME_CUSTOM_KEYBINDING_SCHEMA: &str =
    "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const FOREGROUND_PID_FILE_NAME: &str = "rat-search-foreground.pid";

static FOREGROUND_LAUNCHER_MODE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LauncherActivation {
    startup_id: Option<String>,
    activation_time: Option<u32>,
    xdg_activation_token: Option<String>,
}

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

#[cfg(target_os = "linux")]
fn prepare_launcher_activation_native(window: &WebviewWindow, activation: &LauncherActivation) {
    let Some(startup_id) = activation
        .startup_id
        .clone()
        .or_else(|| activation.xdg_activation_token.clone())
    else {
        return;
    };

    if gtk::glib::MainContext::default().is_owner() {
        match window.gtk_window() {
            Ok(gtk_window) => {
                use gtk::prelude::*;

                gtk_window.set_startup_id(&startup_id);
            }
            Err(error) => {
                eprintln!("failed to access launcher GTK window for startup id: {error}");
            }
        }
        return;
    }

    let target = window.clone();
    let (sender, receiver) = std::sync::mpsc::channel();

    if let Err(error) = window.run_on_main_thread(move || match target.gtk_window() {
        Ok(gtk_window) => {
            use gtk::prelude::*;

            gtk_window.set_startup_id(&startup_id);
            let _ = sender.send(());
        }
        Err(error) => {
            eprintln!("failed to access launcher GTK window for startup id: {error}");
            let _ = sender.send(());
        }
    }) {
        eprintln!("failed to schedule launcher startup id: {error}");
        return;
    }

    if receiver.recv_timeout(Duration::from_millis(100)).is_err() {
        dev_log("timed out waiting for launcher startup id to apply before show");
    }
}

#[cfg(not(target_os = "linux"))]
fn prepare_launcher_activation_native(_window: &WebviewWindow, _activation: &LauncherActivation) {}

#[cfg(target_os = "linux")]
fn present_launcher_native(window: &WebviewWindow, activation: &LauncherActivation) {
    let target = window.clone();
    let activation = activation.clone();

    if let Err(error) = window.run_on_main_thread(move || match target.gtk_window() {
        Ok(gtk_window) => {
            use gtk::prelude::*;

            if let Some(startup_id) = activation.startup_id.as_deref() {
                gtk_window.set_startup_id(startup_id);
            } else if let Some(xdg_activation_token) = activation.xdg_activation_token.as_deref() {
                gtk_window.set_startup_id(xdg_activation_token);
            }

            gtk_window.set_accept_focus(true);
            gtk_window.set_focus_on_map(true);
            gtk_window.set_keep_above(true);
            gtk_window.present_with_time(
                activation
                    .activation_time
                    .unwrap_or_else(gtk::current_event_time),
            );
            gtk_window.grab_focus();
        }
        Err(error) => {
            eprintln!("failed to access launcher GTK window for focus: {error}");
        }
    }) {
        eprintln!("failed to schedule launcher GTK focus: {error}");
    }
}

#[cfg(not(target_os = "linux"))]
fn present_launcher_native(_window: &WebviewWindow, _activation: &LauncherActivation) {}

fn focus_launcher(window: &WebviewWindow, activation: &LauncherActivation) {
    present_launcher_native(window, activation);

    if let Err(error) = window.set_focus() {
        eprintln!("failed to focus launcher window: {error}");
    }

    if let Err(error) = window.emit(LAUNCHER_SHOWN_EVENT, ()) {
        eprintln!("failed to emit {LAUNCHER_SHOWN_EVENT}: {error}");
    }

    if let Err(error) = window.eval("window.__ratSearchFocusInput?.()") {
        eprintln!("failed to evaluate launcher input focus helper: {error}");
    }
}

fn show_launcher(window: &WebviewWindow, activation: LauncherActivation) -> tauri::Result<()> {
    window.set_focusable(true)?;
    window.unminimize()?;
    set_launcher_size(window, false)?;
    position_launcher_with_reason(window, "before show")?;
    prepare_launcher_activation_native(window, &activation);
    window.show()?;
    position_launcher_with_reason(window, "after show")?;
    focus_launcher(window, &activation);

    let delayed_window = window.clone();
    thread::spawn(move || {
        for delay_ms in LAUNCHER_FOCUS_RETRY_MS {
            thread::sleep(Duration::from_millis(delay_ms));

            if !matches!(delayed_window.is_visible(), Ok(true)) {
                dev_log("delayed after show: launcher is no longer visible; skipping center/focus");
                return;
            }

            if let Err(error) = position_launcher_with_reason(&delayed_window, "delayed after show")
            {
                eprintln!("failed to position launcher window after show: {error}");
            }

            focus_launcher(&delayed_window, &activation);
        }
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

    hide_launcher_window(&window).map_err(|error| error.to_string())?;

    if FOREGROUND_LAUNCHER_MODE.load(Ordering::Relaxed) {
        remove_foreground_pid_file();
        app.exit(0);
    }

    Ok(())
}

fn toggle_launcher(app: &tauri::AppHandle, activation: LauncherActivation) {
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
            if let Err(error) = show_launcher(&window, activation) {
                eprintln!("failed to show launcher window: {error}");
            }
        }
        Err(error) => eprintln!("failed to read launcher window visibility: {error}"),
    }
}

fn should_toggle_from_args(args: &[String]) -> bool {
    args.iter().any(|arg| arg == TOGGLE_ARG)
}

fn should_launch_foreground_from_args(args: &[String]) -> bool {
    args.iter().any(|arg| arg == FOREGROUND_ARG)
}

fn should_show_launcher_from_args(args: &[String]) -> bool {
    should_toggle_from_args(args) || should_launch_foreground_from_args(args)
}

fn foreground_pid_file_path() -> PathBuf {
    std::env::temp_dir().join(FOREGROUND_PID_FILE_NAME)
}

fn parse_pid(pid: &str) -> Option<u32> {
    pid.trim().parse::<u32>().ok().filter(|pid| *pid > 0)
}

#[cfg(target_os = "linux")]
fn process_is_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
}

#[cfg(not(target_os = "linux"))]
fn process_is_alive(_pid: u32) -> bool {
    false
}

fn terminate_process(pid: u32) -> bool {
    Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn remove_foreground_pid_file() {
    let _ = fs::remove_file(foreground_pid_file_path());
}

fn consume_foreground_toggle_request(args: &[String]) -> bool {
    if !should_launch_foreground_from_args(args) {
        return false;
    }

    let pid_file = foreground_pid_file_path();

    if let Ok(pid) = fs::read_to_string(&pid_file) {
        if let Some(pid) = parse_pid(&pid) {
            if pid != std::process::id() && process_is_alive(pid) {
                if terminate_process(pid) {
                    remove_foreground_pid_file();
                    return true;
                }
            }
        }
    }

    if let Err(error) = fs::write(&pid_file, std::process::id().to_string()) {
        eprintln!("failed to write foreground launcher pid file: {error}");
    }

    false
}

fn activation_time_from_startup_id(startup_id: &str) -> Option<u32> {
    let time_start = startup_id.rfind("_TIME")? + "_TIME".len();
    let timestamp = startup_id[time_start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();

    if timestamp.is_empty() {
        return None;
    }

    timestamp.parse::<u32>().ok()
}

fn activation_time_from_args(args: &[String]) -> Option<u32> {
    args.windows(2).find_map(|arg_pair| {
        if arg_pair[0] == STARTUP_ID_ARG {
            activation_time_from_startup_id(&arg_pair[1])
        } else {
            None
        }
    })
}

fn launcher_activation_from_args(args: &[String]) -> LauncherActivation {
    let startup_id = args.windows(2).find_map(|arg_pair| {
        if arg_pair[0] == STARTUP_ID_ARG && !arg_pair[1].trim().is_empty() {
            Some(arg_pair[1].clone())
        } else {
            None
        }
    });
    let xdg_activation_token = args.windows(2).find_map(|arg_pair| {
        if arg_pair[0] == XDG_ACTIVATION_TOKEN_ARG && !arg_pair[1].trim().is_empty() {
            Some(arg_pair[1].clone())
        } else {
            None
        }
    });
    let activation_time = startup_id
        .as_deref()
        .and_then(activation_time_from_startup_id)
        .or_else(|| activation_time_from_args(args));

    LauncherActivation {
        startup_id,
        activation_time,
        xdg_activation_token,
    }
}

fn handle_cli_args(app: &tauri::AppHandle, args: &[String]) {
    if should_toggle_from_args(args) {
        toggle_launcher(app, launcher_activation_from_args(args));
    }
}

fn is_wayland_session() -> bool {
    std::env::var("XDG_SESSION_TYPE")
        .map(|session_type| session_type.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn is_gnome_desktop() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|desktop| desktop.to_ascii_uppercase().contains("GNOME"))
        .unwrap_or(false)
}

fn parse_gsettings_path_array(output: &str) -> Vec<String> {
    output
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .filter_map(|path| {
            let path = path.trim().trim_matches('\'').trim_matches('"').trim();
            (!path.is_empty() && path != "@as").then(|| path.to_owned())
        })
        .collect()
}

fn unquote_gsettings_string(output: &str) -> String {
    output
        .trim()
        .trim_matches('\'')
        .trim_matches('"')
        .to_owned()
}

fn quote_gsettings_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn is_legacy_rat_search_hotkey_command(command: &str) -> bool {
    let command = command.trim();
    command == LEGACY_GNOME_HOTKEY_COMMAND
        || (command.contains(LEGACY_GNOME_HOTKEY_COMMAND)
            && !is_current_rat_search_hotkey_command(command))
}

fn is_current_rat_search_hotkey_command(command: &str) -> bool {
    command.trim() == GNOME_HOTKEY_COMMAND
}

fn is_rat_search_hotkey_binding(name: &str, command: &str) -> bool {
    name.trim() == "Rat Search"
        || name.trim() == LEGACY_GNOME_HOTKEY_COMMAND
        || command.contains(LEGACY_GNOME_HOTKEY_COMMAND)
        || command.contains("rat-search foreground")
}

#[cfg(target_os = "linux")]
fn gsettings_get(schema: &str, key: &str) -> Result<String, String> {
    Command::new("gsettings")
        .args(["get", schema, key])
        .output()
        .map_err(|error| error.to_string())
        .and_then(|output| {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).into_owned())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).into_owned())
            }
        })
}

#[cfg(target_os = "linux")]
fn gsettings_get_custom(path: &str, key: &str) -> Result<String, String> {
    let schema = format!("{GNOME_CUSTOM_KEYBINDING_SCHEMA}:{path}");

    gsettings_get(&schema, key)
}

#[cfg(target_os = "linux")]
fn gsettings_set_custom(path: &str, key: &str, value: &str) -> Result<(), String> {
    let schema = format!("{GNOME_CUSTOM_KEYBINDING_SCHEMA}:{path}");
    let output = Command::new("gsettings")
        .args(["set", &schema, key, value])
        .output()
        .map_err(|error| error.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

#[cfg(target_os = "linux")]
fn migrate_gnome_hotkey_command_if_needed() {
    if !is_wayland_session() || !is_gnome_desktop() {
        dev_log("GNOME hotkey migration skipped outside GNOME Wayland");
        return;
    }

    let paths = match gsettings_get(GNOME_MEDIA_KEYS_SCHEMA, "custom-keybindings") {
        Ok(output) => parse_gsettings_path_array(&output),
        Err(error) => {
            eprintln!("failed to read GNOME custom keybindings: {error}");
            return;
        }
    };

    for path in paths {
        let name = gsettings_get_custom(&path, "name")
            .map(|output| unquote_gsettings_string(&output))
            .unwrap_or_default();
        let command = gsettings_get_custom(&path, "command")
            .map(|output| unquote_gsettings_string(&output))
            .unwrap_or_default();

        if !is_rat_search_hotkey_binding(&name, &command) {
            continue;
        }

        if is_current_rat_search_hotkey_command(&command) {
            dev_log(format!(
                "GNOME hotkey migration skipped; {path} already uses startup activation"
            ));
            return;
        }

        if is_legacy_rat_search_hotkey_command(&command) {
            match gsettings_set_custom(
                &path,
                "command",
                &quote_gsettings_string(GNOME_HOTKEY_COMMAND),
            ) {
                Ok(()) => dev_log(format!(
                    "migrated GNOME hotkey {path} to startup activation command"
                )),
                Err(error) => eprintln!("failed to migrate GNOME hotkey {path}: {error}"),
            }
            return;
        }
    }

    dev_log("GNOME hotkey migration skipped; no Rat Search binding found");
}

#[cfg(not(target_os = "linux"))]
fn migrate_gnome_hotkey_command_if_needed() {}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
fn register_launcher_shortcut(app: &tauri::App) {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

    if cfg!(target_os = "linux") && is_wayland_session() {
        eprintln!(
            "global shortcut registration skipped on Wayland; bind your desktop shortcut to `{GNOME_HOTKEY_COMMAND}`"
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
                toggle_launcher(app, LauncherActivation::default());
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

fn spawn_prepared_command(command: &file_actions::PreparedCommand) -> Result<(), String> {
    Command::new(&command.program)
        .args(&command.args)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            eprintln!(
                "failed to spawn '{} {}': {error}",
                command.program,
                command.args.join(" ")
            );
            "Could not open item".to_owned()
        })
}

fn spawn_calculator_command(command: &calculator::PreparedCalculatorCommand) -> Result<(), String> {
    Command::new(&command.program)
        .args(&command.args)
        .spawn()
        .map(|_| ())
        .map_err(|error| {
            eprintln!(
                "failed to open calculator with '{} {}': {error}",
                command.program,
                command.args.join(" ")
            );
            "Could not open calculator".to_owned()
        })
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

fn clipboard_history_file_path_from_data_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(CLIPBOARD_HISTORY_FILE_NAME)
}

fn clipboard_settings_file_path_from_data_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(CLIPBOARD_SETTINGS_FILE_NAME)
}

fn search_history_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|data_dir| history_file_path_from_data_dir(&data_dir))
        .map_err(|error| error.to_string())
}

fn clipboard_history_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|data_dir| clipboard_history_file_path_from_data_dir(&data_dir))
        .map_err(|error| error.to_string())
}

fn clipboard_settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|data_dir| clipboard_settings_file_path_from_data_dir(&data_dir))
        .map_err(|error| error.to_string())
}

fn fallback_search_history_path() -> PathBuf {
    std::env::temp_dir()
        .join("rat-search")
        .join(SEARCH_HISTORY_FILE_NAME)
}

fn fallback_clipboard_history_path() -> PathBuf {
    std::env::temp_dir()
        .join("rat-search")
        .join(CLIPBOARD_HISTORY_FILE_NAME)
}

fn fallback_clipboard_settings_path() -> PathBuf {
    std::env::temp_dir()
        .join("rat-search")
        .join(CLIPBOARD_SETTINGS_FILE_NAME)
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

fn load_clipboard_states(
    app: &tauri::AppHandle,
) -> (ClipboardSettingsState, ClipboardHistoryState) {
    let settings_path = clipboard_settings_path(app).unwrap_or_else(|error| {
        eprintln!("failed to resolve clipboard settings path: {error}");
        fallback_clipboard_settings_path()
    });
    let history_path = clipboard_history_path(app).unwrap_or_else(|error| {
        eprintln!("failed to resolve clipboard history path: {error}");
        fallback_clipboard_history_path()
    });

    load_clipboard_states_from_paths(settings_path, history_path, current_time_ms())
}

fn load_clipboard_states_from_paths(
    settings_path: PathBuf,
    history_path: PathBuf,
    now_ms: u64,
) -> (ClipboardSettingsState, ClipboardHistoryState) {
    let settings = ClipboardSettings::load(settings_path);
    let mut history = ClipboardHistory::load(history_path);
    let before_prune_count = history.entries().len();

    history.prune_expired_at(now_ms, settings.retention_days());

    if history.entries().len() != before_prune_count {
        if let Err(error) = history.save() {
            eprintln!("failed to save pruned clipboard history: {error}");
        }
    }

    let entry_count = history.entries().len();
    let status = if settings.is_enabled() {
        "enabled"
    } else {
        "disabled"
    };

    dev_log(format!(
        "loaded {entry_count} clipboard history entries; clipboard history is {status}"
    ));

    (
        Arc::new(RwLock::new(settings)),
        Arc::new(RwLock::new(history)),
    )
}

fn start_clipboard_monitor(
    app: tauri::AppHandle,
    settings: ClipboardSettingsState,
    history: ClipboardHistoryState,
) {
    thread::spawn(move || loop {
        let enabled = match settings.read() {
            Ok(settings) => settings.is_enabled(),
            Err(error) => {
                eprintln!("failed to read clipboard settings for monitor: {error}");
                false
            }
        };

        if enabled {
            match app.clipboard().read_text() {
                Ok(text) => {
                    if let Err(error) = record_clipboard_text_in_state(
                        &settings,
                        &history,
                        &text,
                        current_time_ms(),
                    ) {
                        eprintln!("failed to record clipboard text: {error}");
                    }
                }
                Err(error) => {
                    eprintln!("failed to read clipboard text: {error}");
                }
            }
        }

        thread::sleep(Duration::from_millis(CLIPBOARD_POLL_INTERVAL_MS));
    });
}

fn record_clipboard_text_in_state(
    settings: &ClipboardSettingsState,
    history: &ClipboardHistoryState,
    text: &str,
    now_ms: u64,
) -> Result<ClipboardRecordOutcome, String> {
    let settings = settings
        .read()
        .map_err(|_| "Clipboard settings unavailable".to_owned())?;

    if !settings.is_enabled() {
        return Ok(ClipboardRecordOutcome::IgnoredDisabled);
    }

    let mut history = history
        .write()
        .map_err(|_| "Clipboard history unavailable".to_owned())?;
    let outcome = history.record_text_at(text, now_ms, &settings);

    if matches!(
        outcome,
        ClipboardRecordOutcome::Recorded | ClipboardRecordOutcome::UpdatedExisting
    ) {
        history
            .save()
            .map_err(|_| "Could not save clipboard history".to_owned())?;
    }

    Ok(outcome)
}

fn set_clipboard_history_enabled_in_state(
    settings: &ClipboardSettingsState,
    enabled: bool,
    now_ms: u64,
) -> Result<(), String> {
    let mut settings = settings
        .write()
        .map_err(|_| "Clipboard settings unavailable".to_owned())?;

    settings.set_enabled(enabled, now_ms);
    settings
        .save()
        .map_err(|_| "Could not save clipboard settings".to_owned())
}

fn clear_clipboard_history_in_state(history: &ClipboardHistoryState) -> Result<(), String> {
    let mut history = history
        .write()
        .map_err(|_| "Clipboard history unavailable".to_owned())?;

    history.clear();
    history
        .save()
        .map_err(|_| "Could not save clipboard history".to_owned())
}

fn delete_clipboard_item_in_state(
    history: &ClipboardHistoryState,
    item_id: &str,
) -> Result<(), String> {
    let mut history = history
        .write()
        .map_err(|_| "Clipboard history unavailable".to_owned())?;

    if !history.delete_item(item_id) {
        return Err("Could not complete action".to_owned());
    }

    history
        .save()
        .map_err(|_| "Could not save clipboard history".to_owned())
}

fn clipboard_item_text_from_state(
    history: &ClipboardHistoryState,
    item_id: &str,
) -> Result<String, String> {
    let history = history
        .read()
        .map_err(|_| "Clipboard history unavailable".to_owned())?;

    history
        .item_text(item_id)
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Could not complete action".to_owned())
}

fn mark_clipboard_item_copied_in_state(
    history: &ClipboardHistoryState,
    item_id: &str,
    now_ms: u64,
) -> Result<(), String> {
    let mut history = history
        .write()
        .map_err(|_| "Clipboard history unavailable".to_owned())?;

    if !history.mark_item_used_at(item_id, now_ms) {
        return Err("Could not complete action".to_owned());
    }

    history
        .save()
        .map_err(|_| "Could not save clipboard history".to_owned())
}

fn clipboard_privacy_status_from_state(
    settings: &ClipboardSettingsState,
    history: &ClipboardHistoryState,
) -> Result<ClipboardPrivacyStatus, String> {
    let settings = settings
        .read()
        .map_err(|_| "Clipboard settings unavailable".to_owned())?;
    let history = history
        .read()
        .map_err(|_| "Clipboard history unavailable".to_owned())?;

    Ok(ClipboardPrivacyStatus {
        enabled: settings.is_enabled(),
        entry_count: history.entries().len(),
        retention_days: settings.retention_days(),
        max_entries: settings.max_entries(),
        max_text_bytes: settings.max_text_bytes(),
    })
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
fn open_in_code(
    app: tauri::AppHandle,
    file_index: tauri::State<'_, FileIndexState>,
    path: String,
) -> Result<(), String> {
    let validated_path = validate_file_action_path(&file_index, &path)?;
    let preferred_open = file_actions::prepare_open_in_code(&validated_path);

    match preferred_open {
        PreferredOpen::Code(command) => {
            if spawn_prepared_command(&command).is_ok() {
                return hide_launcher_for_app(&app);
            }
        }
        PreferredOpen::System => {}
    }

    app.opener()
        .open_path(path_for_opener(&validated_path.path), None::<&str>)
        .map_err(|error| {
            eprintln!(
                "failed to open fallback path '{}': {error}",
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
fn open_calculator_app(
    app: tauri::AppHandle,
    expression: String,
    result: String,
    copy_text: String,
) -> Result<(), String> {
    let command = calculator::prepare_calculator_app_command(&expression, &result, &copy_text)?;

    spawn_calculator_command(&command)?;

    if command.copy_fallback {
        app.clipboard().write_text(copy_text).map_err(|error| {
            eprintln!("failed to copy calculator fallback text: {error}");
            "Could not open calculator".to_owned()
        })?;
    }

    hide_launcher_for_app(&app)
}

#[tauri::command]
fn enable_clipboard_history(
    settings: tauri::State<'_, ClipboardSettingsState>,
) -> Result<(), String> {
    set_clipboard_history_enabled_in_state(&settings, true, current_time_ms())
}

#[tauri::command]
fn disable_clipboard_history(
    settings: tauri::State<'_, ClipboardSettingsState>,
) -> Result<(), String> {
    set_clipboard_history_enabled_in_state(&settings, false, current_time_ms())
}

#[tauri::command]
fn clear_clipboard_history(history: tauri::State<'_, ClipboardHistoryState>) -> Result<(), String> {
    clear_clipboard_history_in_state(&history)
}

#[tauri::command]
fn delete_clipboard_item(
    history: tauri::State<'_, ClipboardHistoryState>,
    item_id: String,
) -> Result<(), String> {
    delete_clipboard_item_in_state(&history, &item_id)
}

#[tauri::command]
fn copy_clipboard_item(
    app: tauri::AppHandle,
    history: tauri::State<'_, ClipboardHistoryState>,
    item_id: String,
) -> Result<(), String> {
    let text = clipboard_item_text_from_state(&history, &item_id)?;

    app.clipboard().write_text(text).map_err(|error| {
        eprintln!("failed to copy clipboard history item: {error}");
        "Could not complete action".to_owned()
    })?;

    mark_clipboard_item_copied_in_state(&history, &item_id, current_time_ms())?;
    hide_launcher_for_app(&app)
}

#[tauri::command]
fn get_clipboard_privacy_status(
    settings: tauri::State<'_, ClipboardSettingsState>,
    history: tauri::State<'_, ClipboardHistoryState>,
) -> Result<ClipboardPrivacyStatus, String> {
    clipboard_privacy_status_from_state(&settings, &history)
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
    clipboard_settings: Option<&ClipboardSettings>,
    clipboard_history: Option<&ClipboardHistory>,
    history: Option<&SearchHistory>,
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    let limit = settings::normalize_result_limit(limit);
    if let Some(open_query) = open_intent_query(query) {
        return file_index
            .map(|file_index| {
                file_search::search_files_with_action(
                    file_index,
                    open_query,
                    limit,
                    SearchAction::OpenInCode,
                )
            })
            .unwrap_or_default();
    }

    let mut results = app_search::search_apps(catalog, query, limit);

    if let Some(file_index) = file_index {
        results.extend(file_search::search_files(file_index, query, limit));
    }

    results.extend(calculator::search_calculator(query, limit));
    results.extend(web_shortcuts::search_web_shortcuts(query, limit));
    results.extend(settings_search::search_settings(query, limit));

    if clipboard_settings.is_some_and(ClipboardSettings::is_enabled) {
        if let Some(clipboard_history) = clipboard_history {
            results.extend(clipboard_history::search_clipboard(
                clipboard_history,
                query,
                limit,
            ));
        }
    }

    if let Some(history) = history {
        results.extend(search_history::search_history(history, query, limit));
    }

    results.sort_by(compare_search_results);
    results.truncate(limit);
    results
}

fn open_intent_query(query: &str) -> Option<&str> {
    let trimmed = query.trim_start();
    let (first, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));

    first
        .eq_ignore_ascii_case("open")
        .then_some(rest.trim_start())
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
        SearchSource::Clipboard => 6,
        SearchSource::History => 7,
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
    clipboard_settings: tauri::State<'_, ClipboardSettingsState>,
    clipboard_history: tauri::State<'_, ClipboardHistoryState>,
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
    let clipboard_settings_guard = match clipboard_settings.read() {
        Ok(settings) => Some(settings),
        Err(error) => {
            eprintln!("failed to read clipboard settings for search: {error}");
            None
        }
    };
    let clipboard_history_guard = match clipboard_history.read() {
        Ok(history) => Some(history),
        Err(error) => {
            eprintln!("failed to read clipboard history for search: {error}");
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
        clipboard_settings_guard.as_deref(),
        clipboard_history_guard.as_deref(),
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

    let launch_args = std::env::args().collect::<Vec<_>>();
    let foreground_mode = should_launch_foreground_from_args(&launch_args);

    if consume_foreground_toggle_request(&launch_args) {
        return;
    }

    FOREGROUND_LAUNCHER_MODE.store(foreground_mode, Ordering::Relaxed);

    let mut builder = tauri::Builder::default();

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    {
        if !foreground_mode {
            builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
                handle_cli_args(app, &args);
            }));
        }
    }

    let setup_launch_args = launch_args.clone();

    builder
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            close_launcher,
            clear_clipboard_history,
            copy_clipboard_item,
            copy_path,
            copy_text,
            delete_clipboard_item,
            disable_clipboard_history,
            enable_clipboard_history,
            get_clipboard_privacy_status,
            hide_launcher,
            launch_app,
            open_in_code,
            open_calculator_app,
            open_path,
            open_setting,
            open_url,
            record_search_history,
            reveal_path,
            search,
            set_launcher_expanded
        ])
        .setup(move |app| {
            let launch_args = setup_launch_args.clone();
            migrate_gnome_hotkey_command_if_needed();

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

            let (clipboard_settings, clipboard_history) = load_clipboard_states(app.handle());
            app.manage(clipboard_settings);
            app.manage(clipboard_history);

            if !foreground_mode {
                let clipboard_settings = app.state::<ClipboardSettingsState>().inner().clone();
                let clipboard_history = app.state::<ClipboardHistoryState>().inner().clone();
                start_clipboard_monitor(
                    app.handle().clone(),
                    clipboard_settings,
                    clipboard_history,
                );
            }

            let file_index = Arc::new(RwLock::new(FileIndex::new()));
            app.manage(file_index.clone());
            start_file_index_scan(file_index);

            if let Some(window) = app.get_webview_window(LAUNCHER_WINDOW_LABEL) {
                position_launcher(&window)?;

                if should_show_launcher_from_args(&launch_args) {
                    show_launcher(&window, launcher_activation_from_args(&launch_args))?;
                }
            }

            #[cfg(any(target_os = "linux", target_os = "macos", windows))]
            if !foreground_mode {
                register_launcher_shortcut(app);
            }

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

    use serde_json::json;

    use crate::{
        app_discovery::{AppCatalog, AppRecord},
        file_index::{FileIndex, FileRecord},
        search_history::SearchHistory,
        search_result::{SearchAction, SearchSource},
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

    #[test]
    fn activation_time_parses_from_gnome_startup_id() {
        assert_eq!(
            activation_time_from_startup_id("rat-search-1234_TIME987654321"),
            Some(987_654_321)
        );
    }

    #[test]
    fn activation_time_ignores_missing_or_invalid_startup_id() {
        assert_eq!(activation_time_from_startup_id("rat-search-1234"), None);
        assert_eq!(activation_time_from_startup_id("rat-search_TIME"), None);
    }

    #[test]
    fn activation_time_parses_from_cli_args() {
        let args = vec![
            "rat-search".to_owned(),
            "toggle".to_owned(),
            STARTUP_ID_ARG.to_owned(),
            "gnome-shell_TIME42".to_owned(),
        ];

        assert_eq!(activation_time_from_args(&args), Some(42));
    }

    #[test]
    fn launcher_activation_parses_startup_id_time_and_xdg_token_from_cli_args() {
        let args = vec![
            "rat-search".to_owned(),
            "toggle".to_owned(),
            STARTUP_ID_ARG.to_owned(),
            "gnome-shell_TIME42".to_owned(),
            XDG_ACTIVATION_TOKEN_ARG.to_owned(),
            "token-123".to_owned(),
        ];

        assert_eq!(
            launcher_activation_from_args(&args),
            LauncherActivation {
                startup_id: Some("gnome-shell_TIME42".to_owned()),
                activation_time: Some(42),
                xdg_activation_token: Some("token-123".to_owned()),
            }
        );
    }

    #[test]
    fn launcher_activation_ignores_empty_cli_values() {
        let args = vec![
            "rat-search".to_owned(),
            "toggle".to_owned(),
            STARTUP_ID_ARG.to_owned(),
            "".to_owned(),
            XDG_ACTIVATION_TOKEN_ARG.to_owned(),
            " ".to_owned(),
        ];

        assert_eq!(
            launcher_activation_from_args(&args),
            LauncherActivation::default()
        );
    }

    #[test]
    fn foreground_arg_requests_visible_launcher_without_toggle_arg() {
        let args = vec!["rat-search".to_owned(), FOREGROUND_ARG.to_owned()];

        assert!(should_launch_foreground_from_args(&args));
        assert!(should_show_launcher_from_args(&args));
        assert!(!should_toggle_from_args(&args));
    }

    #[test]
    fn pid_parser_rejects_invalid_or_zero_values() {
        assert_eq!(parse_pid("123"), Some(123));
        assert_eq!(parse_pid("0"), None);
        assert_eq!(parse_pid("abc"), None);
    }

    #[test]
    fn rat_search_hotkey_command_detection_handles_old_and_new_commands() {
        assert!(is_legacy_rat_search_hotkey_command("rat-search toggle"));
        assert!(is_legacy_rat_search_hotkey_command(
            "/usr/bin/rat-search toggle"
        ));
        assert!(is_legacy_rat_search_hotkey_command(
            r#"/bin/sh -c 'rat-search toggle --startup-id "$DESKTOP_STARTUP_ID"'"#
        ));
        assert!(!is_legacy_rat_search_hotkey_command(GNOME_HOTKEY_COMMAND));
        assert!(is_current_rat_search_hotkey_command(GNOME_HOTKEY_COMMAND));
        assert!(is_rat_search_hotkey_binding(
            "Rat Search",
            GNOME_HOTKEY_COMMAND
        ));
        assert!(GNOME_HOTKEY_COMMAND.contains("rat-search foreground"));
    }

    #[test]
    fn gsettings_helpers_parse_and_quote_values() {
        assert_eq!(
            parse_gsettings_path_array(
                "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/']"
            ),
            vec!["/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/"]
        );
        assert_eq!(
            unquote_gsettings_string("'rat-search toggle'"),
            "rat-search toggle"
        );
        assert_eq!(
            quote_gsettings_string(r#"rat-search "toggle""#),
            r#""rat-search \"toggle\"""#
        );
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

    fn clipboard_settings(enabled: bool) -> (PathBuf, ClipboardSettings) {
        let root = temporary_directory(if enabled {
            "enabled-clipboard-settings"
        } else {
            "disabled-clipboard-settings"
        });
        let path = root.join("clipboard-settings.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({
                "version": 1,
                "enabled": enabled,
                "max_entries": 100,
                "max_text_bytes": 10000,
                "retention_days": 7,
                "updated_at_ms": 1_700
            }))
            .expect("settings should serialize"),
        )
        .expect("clipboard settings should be written");

        (root, ClipboardSettings::load(path))
    }

    fn clipboard_history(entries: &[(&str, u64)]) -> (PathBuf, ClipboardHistory) {
        let root = temporary_directory("clipboard-history");
        let path = root.join("clipboard-history.json");
        let (settings_root, settings) = clipboard_settings(true);
        let mut history = ClipboardHistory::load(path);

        for (text, copied_at_ms) in entries {
            history.record_text_at(text, *copied_at_ms, &settings);
        }

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");

        (root, history)
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

    fn clipboard_settings_state(path: PathBuf, enabled: bool) -> ClipboardSettingsState {
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({
                "version": 1,
                "enabled": enabled,
                "max_entries": 100,
                "max_text_bytes": 10000,
                "retention_days": 7,
                "updated_at_ms": 1_700
            }))
            .expect("settings should serialize"),
        )
        .expect("clipboard settings should be written");

        Arc::new(RwLock::new(ClipboardSettings::load(path)))
    }

    fn clipboard_history_state(path: PathBuf) -> ClipboardHistoryState {
        Arc::new(RwLock::new(ClipboardHistory::load(path)))
    }

    #[test]
    fn history_file_path_from_data_dir_appends_history_file_name() {
        assert_eq!(
            history_file_path_from_data_dir(Path::new("/tmp/rat-search-data")),
            PathBuf::from("/tmp/rat-search-data").join(SEARCH_HISTORY_FILE_NAME)
        );
    }

    #[test]
    fn clipboard_path_helpers_append_expected_file_names() {
        let data_dir = Path::new("/tmp/rat-search-data");

        assert_eq!(
            clipboard_history_file_path_from_data_dir(data_dir),
            PathBuf::from("/tmp/rat-search-data").join(CLIPBOARD_HISTORY_FILE_NAME)
        );
        assert_eq!(
            clipboard_settings_file_path_from_data_dir(data_dir),
            PathBuf::from("/tmp/rat-search-data").join(CLIPBOARD_SETTINGS_FILE_NAME)
        );
    }

    #[test]
    fn clipboard_fallback_paths_use_temp_rat_search_directory() {
        assert_eq!(
            fallback_clipboard_history_path(),
            std::env::temp_dir()
                .join("rat-search")
                .join(CLIPBOARD_HISTORY_FILE_NAME)
        );
        assert_eq!(
            fallback_clipboard_settings_path(),
            std::env::temp_dir()
                .join("rat-search")
                .join(CLIPBOARD_SETTINGS_FILE_NAME)
        );
    }

    #[test]
    fn default_capabilities_allow_clipboard_text_reads() {
        let capability_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("capabilities")
            .join("default.json");
        let contents =
            fs::read_to_string(capability_path).expect("default capability should be readable");

        assert!(contents.contains("\"clipboard-manager:allow-read-text\""));
    }

    #[test]
    fn clipboard_history_enable_and_disable_helpers_persist_settings() {
        let root = temporary_directory("clipboard-enable-disable");
        let path = root.join("clipboard-settings.json");
        let settings = clipboard_settings_state(path.clone(), false);

        set_clipboard_history_enabled_in_state(&settings, true, 2_000)
            .expect("settings should be enabled");
        let enabled_file: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("settings should be readable"))
                .expect("settings json should parse");
        assert_eq!(enabled_file["enabled"], true);
        assert_eq!(enabled_file["updated_at_ms"], 2_000);

        set_clipboard_history_enabled_in_state(&settings, false, 3_000)
            .expect("settings should be disabled");
        let disabled_file: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("settings should be readable"))
                .expect("settings json should parse");
        assert_eq!(disabled_file["enabled"], false);
        assert_eq!(disabled_file["updated_at_ms"], 3_000);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn clipboard_privacy_status_returns_counts_and_limits_without_text() {
        let settings_root = temporary_directory("clipboard-status-settings");
        let history_root = temporary_directory("clipboard-status-history");
        let settings =
            clipboard_settings_state(settings_root.join("clipboard-settings.json"), true);
        let history_path = history_root.join("clipboard-history.json");
        let history = clipboard_history_state(history_path.clone());
        record_clipboard_text_in_state(&settings, &history, "ordinary clipboard text", 1_700)
            .expect("clipboard text should be recorded");

        let status = clipboard_privacy_status_from_state(&settings, &history)
            .expect("status should be returned");
        let status_json = serde_json::to_value(&status).expect("status should serialize");

        assert_eq!(status.enabled, true);
        assert_eq!(status.entry_count, 1);
        assert_eq!(status.retention_days, 7);
        assert_eq!(status.max_entries, 100);
        assert_eq!(status.max_text_bytes, 10_000);
        assert_eq!(status_json.get("text"), None);
        assert_eq!(status_json.get("preview"), None);
        assert_eq!(status_json.get("item_id"), None);

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn clear_clipboard_history_helper_removes_entries_and_persists() {
        let settings_root = temporary_directory("clipboard-clear-settings");
        let history_root = temporary_directory("clipboard-clear-history");
        let settings =
            clipboard_settings_state(settings_root.join("clipboard-settings.json"), true);
        let history_path = history_root.join("clipboard-history.json");
        let history = clipboard_history_state(history_path.clone());
        record_clipboard_text_in_state(&settings, &history, "first copied value", 1_700)
            .expect("first clipboard text should be recorded");
        record_clipboard_text_in_state(&settings, &history, "second copied value", 1_800)
            .expect("second clipboard text should be recorded");

        clear_clipboard_history_in_state(&history).expect("history should be cleared");

        assert!(history.read().expect("history lock").entries().is_empty());
        assert!(ClipboardHistory::load(history_path).entries().is_empty());

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn delete_clipboard_item_helper_removes_one_item_and_persists() {
        let settings_root = temporary_directory("clipboard-delete-settings");
        let history_root = temporary_directory("clipboard-delete-history");
        let settings =
            clipboard_settings_state(settings_root.join("clipboard-settings.json"), true);
        let history_path = history_root.join("clipboard-history.json");
        let history = clipboard_history_state(history_path.clone());
        record_clipboard_text_in_state(&settings, &history, "first copied value", 1_700)
            .expect("first clipboard text should be recorded");
        record_clipboard_text_in_state(&settings, &history, "second copied value", 1_800)
            .expect("second clipboard text should be recorded");
        let item_id = history.read().expect("history lock").entries()[1]
            .id
            .clone();

        delete_clipboard_item_in_state(&history, &item_id).expect("item should be deleted");

        let loaded = ClipboardHistory::load(history_path);
        assert_eq!(loaded.entries().len(), 1);
        assert_ne!(loaded.entries()[0].id, item_id);

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn delete_clipboard_item_helper_rejects_unknown_ids() {
        let history_root = temporary_directory("clipboard-delete-unknown");
        let history = clipboard_history_state(history_root.join("clipboard-history.json"));

        let error = delete_clipboard_item_in_state(&history, "clip:missing")
            .expect_err("unknown item should be rejected");

        assert_eq!(error, "Could not complete action");

        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn copy_clipboard_item_helpers_return_known_text_and_update_usage() {
        let settings_root = temporary_directory("clipboard-copy-settings");
        let history_root = temporary_directory("clipboard-copy-history");
        let settings =
            clipboard_settings_state(settings_root.join("clipboard-settings.json"), true);
        let history_path = history_root.join("clipboard-history.json");
        let history = clipboard_history_state(history_path.clone());
        record_clipboard_text_in_state(&settings, &history, "known copied value", 1_700)
            .expect("clipboard text should be recorded");
        let item_id = history.read().expect("history lock").entries()[0]
            .id
            .clone();

        let text = clipboard_item_text_from_state(&history, &item_id)
            .expect("known item text should be returned");
        mark_clipboard_item_copied_in_state(&history, &item_id, 2_000)
            .expect("known item should be marked copied");

        let loaded = ClipboardHistory::load(history_path);
        assert_eq!(text, "known copied value");
        assert_eq!(loaded.entries()[0].id, item_id);
        assert_eq!(loaded.entries()[0].copied_at_ms, 2_000);
        assert_eq!(loaded.entries()[0].last_used_ms, Some(2_000));
        assert_eq!(loaded.entries()[0].use_count, 1);

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn copy_clipboard_item_helpers_reject_unknown_ids() {
        let history_root = temporary_directory("clipboard-copy-unknown");
        let history = clipboard_history_state(history_root.join("clipboard-history.json"));

        let text_error = clipboard_item_text_from_state(&history, "clip:missing")
            .expect_err("unknown item text should be rejected");
        let mark_error = mark_clipboard_item_copied_in_state(&history, "clip:missing", 2_000)
            .expect_err("unknown item mark should be rejected");

        assert_eq!(text_error, "Could not complete action");
        assert_eq!(mark_error, "Could not complete action");

        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn clipboard_history_save_failure_returns_short_error_without_text() {
        let history_root = temporary_directory("clipboard-save-failure");
        let history_path = history_root.join("clipboard-history.json");
        fs::create_dir_all(&history_path).expect("directory should block file persistence");
        let history = clipboard_history_state(history_path);

        let error = clear_clipboard_history_in_state(&history)
            .expect_err("save failure should be reported");

        assert_eq!(error, "Could not save clipboard history");

        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn load_clipboard_states_from_paths_tolerates_missing_files() {
        let root = temporary_directory("missing-clipboard-state");
        let settings_path = root.join("clipboard-settings.json");
        let history_path = root.join("clipboard-history.json");

        let (settings, history) =
            load_clipboard_states_from_paths(settings_path, history_path, 1_700);

        assert!(!settings.read().expect("settings lock").is_enabled());
        assert!(history.read().expect("history lock").entries().is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn load_clipboard_states_from_paths_prunes_expired_entries() {
        let root = temporary_directory("pruned-clipboard-state");
        let settings_path = root.join("clipboard-settings.json");
        let history_path = root.join("clipboard-history.json");
        fs::write(
            &settings_path,
            serde_json::to_string_pretty(&json!({
                "version": 1,
                "enabled": true,
                "max_entries": 100,
                "max_text_bytes": 10000,
                "retention_days": 1,
                "updated_at_ms": 1_700
            }))
            .expect("settings should serialize"),
        )
        .expect("settings should be written");
        fs::write(
            &history_path,
            serde_json::to_string_pretty(&json!({
                "version": 1,
                "entries": [
                    {
                        "id": "clip:old",
                        "text": "old",
                        "normalized_text": "old",
                        "preview": "old",
                        "copied_at_ms": 1_000_u64,
                        "last_used_ms": null,
                        "use_count": 0,
                        "text_len": 3
                    },
                    {
                        "id": "clip:fresh",
                        "text": "fresh",
                        "normalized_text": "fresh",
                        "preview": "fresh",
                        "copied_at_ms": 86_401_000_u64,
                        "last_used_ms": null,
                        "use_count": 0,
                        "text_len": 5
                    }
                ]
            }))
            .expect("history should serialize"),
        )
        .expect("history should be written");

        let (_, history) =
            load_clipboard_states_from_paths(settings_path, history_path.clone(), 172_801_000);
        let history = history.read().expect("history lock");

        assert_eq!(history.entries().len(), 1);
        assert_eq!(history.entries()[0].id, "clip:fresh");
        drop(history);

        let persisted = clipboard_history::ClipboardHistory::load(history_path);
        assert_eq!(persisted.entries().len(), 1);
        assert_eq!(persisted.entries()[0].id, "clip:fresh");

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn load_clipboard_states_from_paths_preserves_non_expired_entries() {
        let root = temporary_directory("fresh-clipboard-state");
        let settings_path = root.join("clipboard-settings.json");
        let history_path = root.join("clipboard-history.json");
        fs::write(
            &settings_path,
            serde_json::to_string_pretty(&json!({
                "version": 1,
                "enabled": false,
                "max_entries": 100,
                "max_text_bytes": 10000,
                "retention_days": 7,
                "updated_at_ms": 1_700
            }))
            .expect("settings should serialize"),
        )
        .expect("settings should be written");
        fs::write(
            &history_path,
            serde_json::to_string_pretty(&json!({
                "version": 1,
                "entries": [
                    {
                        "id": "clip:fresh",
                        "text": "fresh",
                        "normalized_text": "fresh",
                        "preview": "fresh",
                        "copied_at_ms": 1_000_u64,
                        "last_used_ms": null,
                        "use_count": 0,
                        "text_len": 5
                    }
                ]
            }))
            .expect("history should serialize"),
        )
        .expect("history should be written");

        let (_, history) = load_clipboard_states_from_paths(settings_path, history_path, 2_000);
        let history = history.read().expect("history lock");

        assert_eq!(history.entries().len(), 1);
        assert_eq!(history.entries()[0].id, "clip:fresh");

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn record_clipboard_text_in_state_skips_when_disabled() {
        let root = temporary_directory("disabled-clipboard-record");
        let settings_path = root.join("clipboard-settings.json");
        let history_path = root.join("clipboard-history.json");
        let settings = clipboard_settings_state(settings_path, false);
        let history = clipboard_history_state(history_path.clone());

        let outcome = record_clipboard_text_in_state(&settings, &history, "ordinary text", 1_700)
            .expect("disabled record should not fail");

        assert_eq!(outcome, ClipboardRecordOutcome::IgnoredDisabled);
        assert!(history.read().expect("history lock").entries().is_empty());
        assert!(!history_path.exists());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn record_clipboard_text_in_state_records_and_persists_when_enabled() {
        let root = temporary_directory("enabled-clipboard-record");
        let settings_path = root.join("clipboard-settings.json");
        let history_path = root.join("clipboard-history.json");
        let settings = clipboard_settings_state(settings_path, true);
        let history = clipboard_history_state(history_path.clone());

        let outcome = record_clipboard_text_in_state(&settings, &history, "ordinary text", 1_700)
            .expect("enabled record should succeed");

        assert_eq!(outcome, ClipboardRecordOutcome::Recorded);
        let loaded = ClipboardHistory::load(history_path);
        assert_eq!(loaded.entries().len(), 1);
        assert_eq!(loaded.entries()[0].normalized_text, "ordinary text");

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn record_clipboard_text_in_state_does_not_persist_ignored_duplicate() {
        let root = temporary_directory("duplicate-clipboard-record");
        let settings_path = root.join("clipboard-settings.json");
        let history_path = root.join("clipboard-history.json");
        let settings = clipboard_settings_state(settings_path, true);
        let history = clipboard_history_state(history_path.clone());

        record_clipboard_text_in_state(&settings, &history, "ordinary text", 1_700)
            .expect("initial record should succeed");
        let outcome =
            record_clipboard_text_in_state(&settings, &history, " ordinary  text ", 1_800)
                .expect("duplicate record should not fail");

        assert_eq!(outcome, ClipboardRecordOutcome::IgnoredDuplicate);
        let loaded = ClipboardHistory::load(history_path);
        assert_eq!(loaded.entries().len(), 1);
        assert_eq!(loaded.entries()[0].copied_at_ms, 1_700);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn record_clipboard_text_in_state_does_not_persist_sensitive_text() {
        let root = temporary_directory("sensitive-clipboard-record");
        let settings_path = root.join("clipboard-settings.json");
        let history_path = root.join("clipboard-history.json");
        let settings = clipboard_settings_state(settings_path, true);
        let history = clipboard_history_state(history_path.clone());

        let outcome =
            record_clipboard_text_in_state(&settings, &history, "password=example", 1_700)
                .expect("sensitive record should not fail");

        assert_eq!(outcome, ClipboardRecordOutcome::IgnoredSensitive);
        assert!(history.read().expect("history lock").entries().is_empty());
        assert!(!history_path.exists());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn record_clipboard_text_in_state_reports_save_failure_without_text() {
        let root = temporary_directory("clipboard-save-failure");
        let settings_path = root.join("clipboard-settings.json");
        let history_path = root.join("clipboard-history.json");
        fs::create_dir(&history_path).expect("directory should block file save");
        let settings = clipboard_settings_state(settings_path, true);
        let history = clipboard_history_state(history_path);

        let error = record_clipboard_text_in_state(&settings, &history, "ordinary text", 1_700)
            .expect_err("save should fail");

        assert_eq!(error, "Could not save clipboard history");
        assert!(!error.contains("ordinary text"));

        fs::remove_dir_all(root).expect("temporary directory should be removed");
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
    fn calculator_spawn_failure_maps_to_short_error() {
        let command = calculator::PreparedCalculatorCommand {
            program: "/tmp/rat-search-missing-calculator".to_owned(),
            args: Vec::new(),
            copy_fallback: true,
        };

        assert_eq!(
            spawn_calculator_command(&command).expect_err("missing executable should fail"),
            "Could not open calculator"
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
    fn mixed_search_includes_clipboard_results_when_enabled() {
        let app_catalog = AppCatalog::default();
        let (settings_root, settings) = clipboard_settings(true);
        let (history_root, clipboard_history) = clipboard_history(&[("alpha copied text", 1_700)]);

        let results = search_all(
            &app_catalog,
            None,
            Some(&settings),
            Some(&clipboard_history),
            None,
            "alpha",
            8,
        );

        assert!(results
            .iter()
            .any(|result| result.source == SearchSource::Clipboard));

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn mixed_search_excludes_clipboard_results_when_disabled() {
        let app_catalog = AppCatalog::default();
        let (settings_root, settings) = clipboard_settings(false);
        let (history_root, clipboard_history) = clipboard_history(&[("alpha copied text", 1_700)]);

        let results = search_all(
            &app_catalog,
            None,
            Some(&settings),
            Some(&clipboard_history),
            None,
            "alpha",
            8,
        );

        assert!(!results
            .iter()
            .any(|result| result.source == SearchSource::Clipboard));

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn mixed_search_excludes_clipboard_results_for_empty_queries() {
        let app_catalog = AppCatalog::default();
        let (settings_root, settings) = clipboard_settings(true);
        let (history_root, clipboard_history) = clipboard_history(&[("alpha copied text", 1_700)]);

        let results = search_all(
            &app_catalog,
            None,
            Some(&settings),
            Some(&clipboard_history),
            None,
            "   ",
            8,
        );

        assert!(!results
            .iter()
            .any(|result| result.source == SearchSource::Clipboard));

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn mixed_search_keeps_strong_settings_above_clipboard() {
        let app_catalog = AppCatalog::default();
        let (settings_root, settings) = clipboard_settings(true);
        let (history_root, clipboard_history) = clipboard_history(&[("wifi copied text", 1_700)]);

        let results = search_all(
            &app_catalog,
            None,
            Some(&settings),
            Some(&clipboard_history),
            None,
            "wifi",
            8,
        );

        assert_eq!(results[0].source, SearchSource::Settings);
        assert!(results
            .iter()
            .any(|result| result.source == SearchSource::Clipboard));

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn mixed_search_clipboard_can_rank_above_weak_history() {
        let app_catalog = AppCatalog::default();
        let (settings_root, settings) = clipboard_settings(true);
        let (history_root, clipboard_history) = clipboard_history(&[("alpha copied text", 1_700)]);
        let history = history(&[("my alpha notes", 1_000, 1)]);

        let results = search_all(
            &app_catalog,
            None,
            Some(&settings),
            Some(&clipboard_history),
            Some(&history),
            "alpha",
            8,
        );
        let clipboard_position = results
            .iter()
            .position(|result| result.source == SearchSource::Clipboard)
            .expect("clipboard result should exist");
        let history_position = results
            .iter()
            .position(|result| result.source == SearchSource::History)
            .expect("history result should exist");

        assert!(clipboard_position < history_position);

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn mixed_search_applies_final_limit_after_clipboard_merge() {
        let app_catalog = AppCatalog::default();
        let (settings_root, settings) = clipboard_settings(true);
        let (history_root, clipboard_history) = clipboard_history(&[("alpha copied text", 1_700)]);
        let history = history(&[("my alpha notes", 1_000, 1)]);

        let results = search_all(
            &app_catalog,
            None,
            Some(&settings),
            Some(&clipboard_history),
            Some(&history),
            "alpha",
            1,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SearchSource::Clipboard);

        fs::remove_dir_all(settings_root).expect("temporary directory should be removed");
        fs::remove_dir_all(history_root).expect("temporary directory should be removed");
    }

    #[test]
    fn mixed_search_falls_back_when_clipboard_state_is_absent() {
        let app_catalog = AppCatalog::default();
        let history = history(&[("alpha history", 1_700, 1)]);

        let results = search_all(&app_catalog, None, None, None, Some(&history), "alpha", 8);

        assert!(results
            .iter()
            .any(|result| result.source == SearchSource::History));
        assert!(!results
            .iter()
            .any(|result| result.source == SearchSource::Clipboard));
    }

    #[test]
    fn mixed_search_keeps_strong_app_above_noisy_file_match() {
        let app_catalog = catalog(vec![app("report.desktop", "Report")]);
        let file_index = index(vec![file("/home/sanuk/Documents/Annual Report.pdf")]);

        let results = search_all(
            &app_catalog,
            Some(&file_index),
            None,
            None,
            None,
            "report",
            8,
        );

        assert_eq!(results[0].source, SearchSource::Applications);
        assert_eq!(results[0].title, "Report");
        assert_eq!(results[1].source, SearchSource::Files);
    }

    #[test]
    fn open_intent_strips_prefix_and_returns_file_suggestions() {
        let app_catalog = catalog(vec![app("code.desktop", "Code")]);
        let file_index = index(vec![
            file("/home/sanuk/Documents/work_done.md"),
            file("/home/sanuk/Documents/notes.txt"),
        ]);

        let results = search_all(
            &app_catalog,
            Some(&file_index),
            None,
            None,
            None,
            "open work_done.md",
            8,
        );

        assert_eq!(results.first().expect("file result").title, "work_done.md");
        assert!(results
            .iter()
            .all(|result| result.action == SearchAction::OpenInCode));
        assert!(results
            .iter()
            .all(|result| result.source == SearchSource::Files));
    }

    #[test]
    fn open_intent_is_case_insensitive_and_returns_folder_suggestions() {
        let app_catalog = catalog(Vec::new());
        let file_index = index(vec![
            folder("/home/sanuk/Desktop/Projects/pc_work"),
            folder("/home/sanuk/Desktop/Projects/other"),
        ]);

        let results = search_all(
            &app_catalog,
            Some(&file_index),
            None,
            None,
            None,
            "OPEN pc_work",
            8,
        );

        let result = results.first().expect("folder result");
        assert_eq!(result.title, "pc_work");
        assert_eq!(result.action, SearchAction::OpenInCode);
        assert_eq!(result.source, SearchSource::Folders);
        assert!(matches!(
            result.metadata,
            Some(search_result::SearchMetadata::Folder)
        ));
    }

    #[test]
    fn normal_file_search_keeps_default_open_action() {
        let app_catalog = catalog(Vec::new());
        let file_index = index(vec![file("/home/sanuk/Documents/work_done.md")]);

        let results = search_all(
            &app_catalog,
            Some(&file_index),
            None,
            None,
            None,
            "work_done.md",
            8,
        );

        assert_eq!(
            results.first().expect("file result").action,
            SearchAction::OpenPath
        );
    }

    #[test]
    fn mixed_search_applies_final_limit_after_merging_sources() {
        let app_catalog = catalog(vec![app("calendar.desktop", "Calendar")]);
        let file_index = index(vec![
            file("/home/sanuk/Documents/Report.pdf"),
            folder("/home/sanuk/Documents/Reports"),
        ]);

        let results = search_all(
            &app_catalog,
            Some(&file_index),
            None,
            None,
            None,
            "report",
            1,
        );

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

        let results = search_all(
            &app_catalog,
            Some(&file_index),
            None,
            None,
            None,
            "alpha",
            8,
        );

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
    fn source_priority_places_clipboard_between_web_and_history() {
        assert!(source_priority(&SearchSource::Web) < source_priority(&SearchSource::Clipboard));
        assert!(
            source_priority(&SearchSource::Clipboard) < source_priority(&SearchSource::History)
        );
    }

    #[test]
    fn mixed_search_falls_back_to_apps_without_file_index() {
        let app_catalog = catalog(vec![app("settings.desktop", "Settings")]);

        let results = search_all(&app_catalog, None, None, None, None, "settings", 8);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SearchSource::Applications);
    }

    #[test]
    fn mixed_search_includes_calculator_above_weak_file_matches() {
        let app_catalog = AppCatalog::default();
        let file_index = index(vec![file("/home/sanuk/Documents/2 plus 2 notes.txt")]);

        let results = search_all(&app_catalog, Some(&file_index), None, None, None, "2+2", 8);

        assert_eq!(results[0].source, SearchSource::Calculator);
        assert_eq!(results[0].title, "4");
    }

    #[test]
    fn mixed_search_keeps_exact_app_above_web_and_history_results() {
        let app_catalog = catalog(vec![app("what-is-rust.desktop", "what is rust")]);
        let history = history(&[("what is rust", 1_700, 10)]);

        let results = search_all(
            &app_catalog,
            None,
            None,
            None,
            Some(&history),
            "what is rust",
            8,
        );

        assert_eq!(results[0].source, SearchSource::Applications);
        assert_eq!(results[0].title, "what is rust");
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

        let results = search_all(&app_catalog, None, None, None, None, "wifi", 8);

        assert_eq!(results[0].source, SearchSource::Settings);
        assert_eq!(results[0].title, "Wi-Fi");
    }

    #[test]
    fn mixed_search_includes_google_question_search() {
        let app_catalog = AppCatalog::default();

        let results = search_all(
            &app_catalog,
            None,
            None,
            None,
            None,
            "what is rust tauri",
            8,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SearchSource::Web);
        assert_eq!(results[0].title, "Search Google");
    }

    #[test]
    fn mixed_search_keeps_history_below_strong_live_results() {
        let app_catalog = AppCatalog::default();
        let history = history(&[("wifi troubleshooting", 1_700, 20)]);

        let results = search_all(&app_catalog, None, None, None, Some(&history), "wifi", 8);

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

        let results = search_all(&app_catalog, None, None, None, Some(&history), "wifi", 2);

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

        let results = search_all(&app_catalog, None, None, None, None, "wifi", 8);

        assert!(results
            .iter()
            .any(|result| result.source == SearchSource::Settings));
        assert!(!results
            .iter()
            .any(|result| result.source == SearchSource::History));
    }
}
