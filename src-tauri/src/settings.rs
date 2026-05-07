pub(crate) const DEFAULT_HOTKEY_LABEL: &str = "Alt+Space";
pub(crate) const DEFAULT_MAX_RESULTS: usize = 8;
pub(crate) const RESULT_LIMIT_CAP: usize = 20;
pub(crate) const LAUNCHER_WINDOW_WIDTH: u32 = 704;
pub(crate) const LAUNCHER_COMPACT_HEIGHT: u32 = 76;
pub(crate) const LAUNCHER_EXPANDED_HEIGHT: u32 = 460;
pub(crate) const DEFAULT_THEME: &str = "system";
pub(crate) const DEFAULT_SEARCH_SOURCE: &str = "applications only";
pub(crate) const DEFAULT_INDEX_ROOT_NAMES: [&str; 4] =
    ["Desktop", "Documents", "Downloads", "Pictures"];

pub(crate) fn normalize_result_limit(limit: usize) -> usize {
    match limit {
        0 => DEFAULT_MAX_RESULTS,
        limit => limit.min(RESULT_LIMIT_CAP),
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(crate) const DEFAULT_HOTKEY_MODIFIERS: tauri_plugin_global_shortcut::Modifiers =
    tauri_plugin_global_shortcut::Modifiers::ALT;

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
pub(crate) const DEFAULT_HOTKEY_CODE: tauri_plugin_global_shortcut::Code =
    tauri_plugin_global_shortcut::Code::Space;
