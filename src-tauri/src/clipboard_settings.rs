#![allow(dead_code)]

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

const SETTINGS_VERSION: u32 = 1;
const DEFAULT_ENABLED: bool = false;
const DEFAULT_MAX_ENTRIES: u32 = 100;
const DEFAULT_MAX_TEXT_BYTES: u32 = 10_000;
const DEFAULT_RETENTION_DAYS: u32 = 7;
const DEFAULT_UPDATED_AT_MS: u64 = 0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClipboardSettings {
    path: PathBuf,
    enabled: bool,
    max_entries: u32,
    max_text_bytes: u32,
    retention_days: u32,
    updated_at_ms: u64,
}

impl ClipboardSettings {
    pub(crate) fn load(path: PathBuf) -> Self {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return Self::defaults(path),
            Err(error) => {
                eprintln!(
                    "failed to read clipboard settings '{}': {error}",
                    path.display()
                );
                return Self::defaults(path);
            }
        };

        let file = match serde_json::from_str::<ClipboardSettingsFile>(&contents) {
            Ok(file) if file.version == SETTINGS_VERSION => file,
            Ok(file) => {
                eprintln!(
                    "unsupported clipboard settings version {} in '{}'",
                    file.version,
                    path.display()
                );
                return Self::defaults(path);
            }
            Err(error) => {
                eprintln!(
                    "failed to parse clipboard settings '{}': {error}",
                    path.display()
                );
                return Self::defaults(path);
            }
        };

        Self {
            path,
            enabled: file.enabled,
            max_entries: clamp_to_default(file.max_entries, DEFAULT_MAX_ENTRIES),
            max_text_bytes: clamp_to_default(file.max_text_bytes, DEFAULT_MAX_TEXT_BYTES),
            retention_days: clamp_to_default(file.retention_days, DEFAULT_RETENTION_DAYS),
            updated_at_ms: file.updated_at_ms,
        }
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create clipboard settings directory '{}': {error}",
                    parent.display()
                )
            })?;
        }

        let file = ClipboardSettingsFile {
            version: SETTINGS_VERSION,
            enabled: self.enabled,
            max_entries: self.max_entries,
            max_text_bytes: self.max_text_bytes,
            retention_days: self.retention_days,
            updated_at_ms: self.updated_at_ms,
        };
        let contents = serde_json::to_string_pretty(&file)
            .map_err(|error| format!("failed to serialize clipboard settings: {error}"))?;
        let temp_path = temporary_settings_path(&self.path);

        fs::write(&temp_path, contents).map_err(|error| {
            format!(
                "failed to write temporary clipboard settings '{}': {error}",
                temp_path.display()
            )
        })?;
        fs::rename(&temp_path, &self.path).map_err(|error| {
            format!(
                "failed to persist clipboard settings '{}' from '{}': {error}",
                self.path.display(),
                temp_path.display()
            )
        })
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool, updated_at_ms: u64) {
        self.enabled = enabled;
        self.updated_at_ms = updated_at_ms;
    }

    pub(crate) fn max_entries(&self) -> u32 {
        self.max_entries
    }

    pub(crate) fn max_text_bytes(&self) -> u32 {
        self.max_text_bytes
    }

    pub(crate) fn retention_days(&self) -> u32 {
        self.retention_days
    }

    fn defaults(path: PathBuf) -> Self {
        Self {
            path,
            enabled: DEFAULT_ENABLED,
            max_entries: DEFAULT_MAX_ENTRIES,
            max_text_bytes: DEFAULT_MAX_TEXT_BYTES,
            retention_days: DEFAULT_RETENTION_DAYS,
            updated_at_ms: DEFAULT_UPDATED_AT_MS,
        }
    }
}

fn clamp_to_default(value: u32, default_value: u32) -> u32 {
    if value == 0 {
        default_value
    } else {
        value
    }
}

fn temporary_settings_path(path: &Path) -> PathBuf {
    match path.file_name().and_then(|file_name| file_name.to_str()) {
        Some(file_name) => path.with_file_name(format!("{file_name}.tmp")),
        None => path.with_extension("tmp"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ClipboardSettingsFile {
    version: u32,
    enabled: bool,
    max_entries: u32,
    max_text_bytes: u32,
    retention_days: u32,
    updated_at_ms: u64,
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;

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

    fn temporary_settings(name: &str) -> (PathBuf, PathBuf) {
        let root = temporary_directory(name);
        let path = root.join("clipboard-settings.json");

        (root, path)
    }

    fn settings_file(
        enabled: bool,
        max_entries: u32,
        max_text_bytes: u32,
        retention_days: u32,
        updated_at_ms: u64,
    ) -> String {
        serde_json::to_string_pretty(&json!({
            "version": SETTINGS_VERSION,
            "enabled": enabled,
            "max_entries": max_entries,
            "max_text_bytes": max_text_bytes,
            "retention_days": retention_days,
            "updated_at_ms": updated_at_ms
        }))
        .expect("settings file should serialize")
    }

    #[test]
    fn defaults_are_privacy_safe_and_disabled() {
        let settings = ClipboardSettings::defaults(PathBuf::from("/tmp/clipboard-settings.json"));

        assert!(!settings.is_enabled());
        assert_eq!(settings.max_entries, DEFAULT_MAX_ENTRIES);
        assert_eq!(settings.max_text_bytes, DEFAULT_MAX_TEXT_BYTES);
        assert_eq!(settings.retention_days, DEFAULT_RETENTION_DAYS);
        assert_eq!(settings.updated_at_ms, DEFAULT_UPDATED_AT_MS);
    }

    #[test]
    fn missing_file_loads_disabled_defaults() {
        let (root, path) = temporary_settings("missing");

        let settings = ClipboardSettings::load(path);

        assert!(!settings.is_enabled());
        assert_eq!(settings.max_entries, DEFAULT_MAX_ENTRIES);
        assert_eq!(settings.updated_at_ms, DEFAULT_UPDATED_AT_MS);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn corrupted_json_loads_disabled_defaults() {
        let (root, path) = temporary_settings("corrupted");
        fs::write(&path, "{not-json").expect("corrupted settings should be written");

        let settings = ClipboardSettings::load(path);

        assert!(!settings.is_enabled());
        assert_eq!(settings.max_text_bytes, DEFAULT_MAX_TEXT_BYTES);
        assert_eq!(settings.updated_at_ms, DEFAULT_UPDATED_AT_MS);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn unsupported_version_loads_disabled_defaults() {
        let (root, path) = temporary_settings("unsupported-version");
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({
                "version": 999,
                "enabled": true,
                "max_entries": 25,
                "max_text_bytes": 500,
                "retention_days": 2,
                "updated_at_ms": 1_700
            }))
            .expect("settings file should serialize"),
        )
        .expect("settings should be written");

        let settings = ClipboardSettings::load(path);

        assert!(!settings.is_enabled());
        assert_eq!(settings.max_entries, DEFAULT_MAX_ENTRIES);
        assert_eq!(settings.updated_at_ms, DEFAULT_UPDATED_AT_MS);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn serialization_round_trip_preserves_settings() {
        let (root, path) = temporary_settings("round-trip");
        let settings = ClipboardSettings {
            path: path.clone(),
            enabled: true,
            max_entries: 42,
            max_text_bytes: 8_192,
            retention_days: 3,
            updated_at_ms: 1_700,
        };

        settings.save().expect("settings should save");
        let loaded = ClipboardSettings::load(path);

        assert_eq!(loaded, settings);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn set_enabled_updates_only_enabled_and_timestamp() {
        let mut settings = ClipboardSettings {
            path: PathBuf::from("/tmp/clipboard-settings.json"),
            enabled: false,
            max_entries: 42,
            max_text_bytes: 8_192,
            retention_days: 3,
            updated_at_ms: 1_000,
        };

        settings.set_enabled(true, 2_000);

        assert!(settings.is_enabled());
        assert_eq!(settings.max_entries, 42);
        assert_eq!(settings.max_text_bytes, 8_192);
        assert_eq!(settings.retention_days, 3);
        assert_eq!(settings.updated_at_ms, 2_000);
    }

    #[test]
    fn invalid_limits_clamp_to_safe_defaults() {
        let (root, path) = temporary_settings("invalid-limits");
        fs::write(&path, settings_file(true, 0, 0, 0, 1_700)).expect("settings should be written");

        let settings = ClipboardSettings::load(path);

        assert!(settings.is_enabled());
        assert_eq!(settings.max_entries, DEFAULT_MAX_ENTRIES);
        assert_eq!(settings.max_text_bytes, DEFAULT_MAX_TEXT_BYTES);
        assert_eq!(settings.retention_days, DEFAULT_RETENTION_DAYS);
        assert_eq!(settings.updated_at_ms, 1_700);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn save_creates_missing_parent_directory() {
        let root = temporary_directory("missing-parent");
        let path = root.join("nested").join("clipboard-settings.json");
        let settings = ClipboardSettings {
            path: path.clone(),
            enabled: true,
            max_entries: DEFAULT_MAX_ENTRIES,
            max_text_bytes: DEFAULT_MAX_TEXT_BYTES,
            retention_days: DEFAULT_RETENTION_DAYS,
            updated_at_ms: 1_700,
        };

        settings.save().expect("settings should save");

        assert!(path.exists());
        assert!(!temporary_settings_path(&path).exists());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }
}
