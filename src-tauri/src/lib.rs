use std::{thread, time::Duration};

use tauri::{Emitter, Manager, PhysicalPosition, Position, WebviewWindow};

mod app_discovery;
mod app_launch;
mod app_search;

use app_discovery::AppCatalog;
use app_launch::LaunchResult;
use app_search::AppSearchResult;

const LAUNCHER_WINDOW_LABEL: &str = "main";
const LAUNCHER_SHOWN_EVENT: &str = "launcher:shown";
const TOGGLE_ARG: &str = "toggle";
const DELAYED_CENTER_MS: u64 = 80;
const LAUNCHER_VERTICAL_PERCENT: u32 = 25;

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
        let y = (monitor_position.y + vertical_target - (window_size.height / 2) as i32)
            .max(monitor_position.y);

        dev_log(format!(
            "{reason}: monitor=({}, {}) {}x{}, window={}x{}, vertical_target={LAUNCHER_VERTICAL_PERCENT}%, target=({}, {})",
            monitor_position.x,
            monitor_position.y,
            monitor_size.width,
            monitor_size.height,
            window_size.width,
            window_size.height,
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

fn focus_launcher(window: &WebviewWindow) {
    if let Err(error) = window.set_focus() {
        eprintln!("failed to focus launcher window: {error}");
    }

    if let Err(error) = window.emit(LAUNCHER_SHOWN_EVENT, ()) {
        eprintln!("failed to emit {LAUNCHER_SHOWN_EVENT}: {error}");
    }
}

fn show_launcher(window: &WebviewWindow) -> tauri::Result<()> {
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
    use tauri_plugin_global_shortcut::{
        Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
    };

    if cfg!(target_os = "linux") && is_wayland_session() {
        eprintln!(
            "global shortcut registration skipped on Wayland; bind your desktop shortcut to `rat-search toggle`"
        );
        return;
    }

    let launcher_shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Space);
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
        eprintln!("failed to register Alt+Space launcher shortcut: {error}");
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

#[tauri::command]
fn search_apps(
    catalog: tauri::State<'_, AppCatalog>,
    query: String,
    limit: usize,
) -> Vec<AppSearchResult> {
    app_search::search_apps(&catalog, &query, limit)
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
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            close_launcher,
            hide_launcher,
            launch_app,
            search_apps
        ])
        .setup(|app| {
            let launch_args = std::env::args().collect::<Vec<_>>();
            let app_catalog = AppCatalog::scan();
            dev_log(format!("discovered {} applications", app_catalog.len()));
            app.manage(app_catalog);

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
