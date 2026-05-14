#![allow(dead_code)]

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::clipboard_settings::ClipboardSettings;

const HISTORY_VERSION: u32 = 1;
const PREVIEW_CHAR_LIMIT: usize = 120;
const MILLIS_PER_DAY: u64 = 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClipboardHistory {
    path: PathBuf,
    entries: Vec<ClipboardHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ClipboardHistoryEntry {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) normalized_text: String,
    pub(crate) preview: String,
    pub(crate) copied_at_ms: u64,
    pub(crate) last_used_ms: Option<u64>,
    pub(crate) use_count: u32,
    pub(crate) text_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClipboardRecordOutcome {
    Recorded,
    UpdatedExisting,
    IgnoredEmpty,
    IgnoredDuplicate,
    IgnoredTooLarge,
}

impl ClipboardHistory {
    pub(crate) fn load(path: PathBuf) -> Self {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Self {
                    path,
                    entries: Vec::new(),
                };
            }
            Err(error) => {
                eprintln!(
                    "failed to read clipboard history '{}': {error}",
                    path.display()
                );
                return Self {
                    path,
                    entries: Vec::new(),
                };
            }
        };

        let file = match serde_json::from_str::<ClipboardHistoryFile>(&contents) {
            Ok(file) if file.version == HISTORY_VERSION => file,
            Ok(file) => {
                eprintln!(
                    "unsupported clipboard history version {} in '{}'",
                    file.version,
                    path.display()
                );
                return Self {
                    path,
                    entries: Vec::new(),
                };
            }
            Err(error) => {
                eprintln!(
                    "failed to parse clipboard history '{}': {error}",
                    path.display()
                );
                return Self {
                    path,
                    entries: Vec::new(),
                };
            }
        };

        let mut history = Self {
            path,
            entries: sanitize_entries(file.entries),
        };
        history.sort_entries();
        history
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create clipboard history directory '{}': {error}",
                    parent.display()
                )
            })?;
        }

        let file = ClipboardHistoryFile {
            version: HISTORY_VERSION,
            entries: self.entries.clone(),
        };
        let contents = serde_json::to_string_pretty(&file)
            .map_err(|error| format!("failed to serialize clipboard history: {error}"))?;
        let temp_path = temporary_history_path(&self.path);

        fs::write(&temp_path, contents).map_err(|error| {
            format!(
                "failed to write temporary clipboard history '{}': {error}",
                temp_path.display()
            )
        })?;
        fs::rename(&temp_path, &self.path).map_err(|error| {
            format!(
                "failed to persist clipboard history '{}' from '{}': {error}",
                self.path.display(),
                temp_path.display()
            )
        })
    }

    pub(crate) fn record_text_at(
        &mut self,
        text: &str,
        copied_at_ms: u64,
        settings: &ClipboardSettings,
    ) -> ClipboardRecordOutcome {
        let normalized_text = normalize_clipboard_text(text);

        if normalized_text.is_empty() {
            return ClipboardRecordOutcome::IgnoredEmpty;
        }

        if text.len() > settings.max_text_bytes() as usize {
            return ClipboardRecordOutcome::IgnoredTooLarge;
        }

        if self
            .entries
            .first()
            .is_some_and(|entry| entry.normalized_text == normalized_text)
        {
            return ClipboardRecordOutcome::IgnoredDuplicate;
        }

        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.normalized_text == normalized_text)
        {
            let mut entry = self.entries.remove(index);
            entry.text = text.to_owned();
            entry.preview = preview_for_text(&normalized_text);
            entry.copied_at_ms = copied_at_ms;
            entry.last_used_ms = Some(copied_at_ms);
            entry.use_count = entry.use_count.saturating_add(1);
            entry.text_len = text.chars().count();
            self.entries.insert(0, entry);
            self.cap_entries(settings.max_entries());
            return ClipboardRecordOutcome::UpdatedExisting;
        }

        let entry = ClipboardHistoryEntry {
            id: item_id_for(&normalized_text, copied_at_ms),
            text: text.to_owned(),
            normalized_text: normalized_text.clone(),
            preview: preview_for_text(&normalized_text),
            copied_at_ms,
            last_used_ms: None,
            use_count: 0,
            text_len: text.chars().count(),
        };

        self.entries.insert(0, entry);
        self.cap_entries(settings.max_entries());

        ClipboardRecordOutcome::Recorded
    }

    pub(crate) fn delete_item(&mut self, item_id: &str) -> bool {
        let before_len = self.entries.len();
        self.entries.retain(|entry| entry.id != item_id);

        self.entries.len() != before_len
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn prune_expired_at(&mut self, now_ms: u64, retention_days: u32) {
        if retention_days == 0 {
            self.entries.clear();
            return;
        }

        let retention_ms = u64::from(retention_days).saturating_mul(MILLIS_PER_DAY);
        self.entries
            .retain(|entry| now_ms.saturating_sub(entry.copied_at_ms) <= retention_ms);
    }

    pub(crate) fn entries(&self) -> &[ClipboardHistoryEntry] {
        &self.entries
    }

    fn sort_entries(&mut self) {
        self.entries.sort_by(|left, right| {
            right
                .copied_at_ms
                .cmp(&left.copied_at_ms)
                .then_with(|| left.normalized_text.cmp(&right.normalized_text))
        });
    }

    fn cap_entries(&mut self, max_entries: u32) {
        self.sort_entries();
        self.entries.truncate(max_entries as usize);
    }
}

pub(crate) fn normalize_clipboard_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn preview_for_text(normalized_text: &str) -> String {
    normalized_text.chars().take(PREVIEW_CHAR_LIMIT).collect()
}

fn item_id_for(normalized_text: &str, copied_at_ms: u64) -> String {
    let mut hasher = DefaultHasher::new();
    normalized_text.hash(&mut hasher);
    copied_at_ms.hash(&mut hasher);

    format!("clip:{:016x}", hasher.finish())
}

fn sanitize_entries(entries: Vec<ClipboardHistoryEntry>) -> Vec<ClipboardHistoryEntry> {
    let mut sanitized = Vec::new();

    for mut entry in entries {
        entry.normalized_text = normalize_clipboard_text(&entry.normalized_text);

        if entry.normalized_text.is_empty() || entry.id.trim().is_empty() || entry.text.is_empty() {
            continue;
        }

        if !entry.id.starts_with("clip:") {
            continue;
        }

        entry.preview = preview_for_text(&entry.normalized_text);
        entry.text_len = entry.text.chars().count();
        sanitized.push(entry);
    }

    sanitized
}

fn temporary_history_path(path: &Path) -> PathBuf {
    match path.file_name().and_then(|file_name| file_name.to_str()) {
        Some(file_name) => path.with_file_name(format!("{file_name}.tmp")),
        None => path.with_extension("tmp"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ClipboardHistoryFile {
    version: u32,
    entries: Vec<ClipboardHistoryEntry>,
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

    fn temporary_history(name: &str) -> (PathBuf, PathBuf) {
        let root = temporary_directory(name);
        let path = root.join("clipboard-history.json");

        (root, path)
    }

    fn settings(max_entries: u32, max_text_bytes: u32, retention_days: u32) -> ClipboardSettings {
        let root = temporary_directory("settings");
        let path = root.join("clipboard-settings.json");
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({
                "version": 1,
                "enabled": true,
                "max_entries": max_entries,
                "max_text_bytes": max_text_bytes,
                "retention_days": retention_days,
                "updated_at_ms": 1
            }))
            .expect("settings should serialize"),
        )
        .expect("settings should be written");

        ClipboardSettings::load(path)
    }

    fn history_with(entries: Vec<ClipboardHistoryEntry>) -> ClipboardHistory {
        let mut history = ClipboardHistory {
            path: PathBuf::from("/tmp/clipboard-history.json"),
            entries,
        };
        history.sort_entries();
        history
    }

    fn entry(id: &str, text: &str, copied_at_ms: u64) -> ClipboardHistoryEntry {
        let normalized_text = normalize_clipboard_text(text);
        ClipboardHistoryEntry {
            id: id.to_owned(),
            text: text.to_owned(),
            preview: preview_for_text(&normalized_text),
            normalized_text,
            copied_at_ms,
            last_used_ms: None,
            use_count: 0,
            text_len: text.chars().count(),
        }
    }

    #[test]
    fn empty_and_whitespace_text_are_ignored() {
        let mut history = history_with(Vec::new());
        let settings = settings(100, 10_000, 7);

        assert_eq!(
            history.record_text_at("", 1_700, &settings),
            ClipboardRecordOutcome::IgnoredEmpty
        );
        assert_eq!(
            history.record_text_at(" \n\t ", 1_700, &settings),
            ClipboardRecordOutcome::IgnoredEmpty
        );
        assert!(history.entries().is_empty());
    }

    #[test]
    fn over_limit_text_is_ignored() {
        let mut history = history_with(Vec::new());
        let settings = settings(100, 3, 7);

        assert_eq!(
            history.record_text_at("four", 1_700, &settings),
            ClipboardRecordOutcome::IgnoredTooLarge
        );
        assert!(history.entries().is_empty());
    }

    #[test]
    fn duplicate_newest_text_is_ignored() {
        let mut history = history_with(Vec::new());
        let settings = settings(100, 10_000, 7);

        assert_eq!(
            history.record_text_at("copy text", 1_700, &settings),
            ClipboardRecordOutcome::Recorded
        );
        assert_eq!(
            history.record_text_at(" copy   text ", 1_800, &settings),
            ClipboardRecordOutcome::IgnoredDuplicate
        );

        assert_eq!(history.entries().len(), 1);
        assert_eq!(history.entries()[0].copied_at_ms, 1_700);
        assert_eq!(history.entries()[0].use_count, 0);
    }

    #[test]
    fn new_text_creates_expected_entry_without_leaking_text_in_id() {
        let mut history = history_with(Vec::new());
        let settings = settings(100, 10_000, 7);

        let outcome = history.record_text_at("  copied   text  ", 1_700, &settings);

        assert_eq!(outcome, ClipboardRecordOutcome::Recorded);
        assert_eq!(history.entries().len(), 1);

        let entry = &history.entries()[0];
        assert!(entry.id.starts_with("clip:"));
        assert!(!entry.id.contains("copied"));
        assert!(!entry.id.contains("text"));
        assert_eq!(entry.text, "  copied   text  ");
        assert_eq!(entry.normalized_text, "copied text");
        assert_eq!(entry.preview, "copied text");
        assert_eq!(entry.copied_at_ms, 1_700);
        assert_eq!(entry.last_used_ms, None);
        assert_eq!(entry.use_count, 0);
        assert_eq!(entry.text_len, 17);
    }

    #[test]
    fn existing_non_newest_text_updates_and_moves_to_front() {
        let mut history = history_with(Vec::new());
        let settings = settings(100, 10_000, 7);

        history.record_text_at("first", 1_000, &settings);
        history.record_text_at("second", 2_000, &settings);

        let first_id = history
            .entries()
            .iter()
            .find(|entry| entry.normalized_text == "first")
            .expect("first entry")
            .id
            .clone();

        assert_eq!(
            history.record_text_at(" first ", 3_000, &settings),
            ClipboardRecordOutcome::UpdatedExisting
        );

        assert_eq!(history.entries().len(), 2);
        assert_eq!(history.entries()[0].id, first_id);
        assert_eq!(history.entries()[0].text, " first ");
        assert_eq!(history.entries()[0].copied_at_ms, 3_000);
        assert_eq!(history.entries()[0].last_used_ms, Some(3_000));
        assert_eq!(history.entries()[0].use_count, 1);
    }

    #[test]
    fn entry_cap_is_enforced_from_settings() {
        let mut history = history_with(Vec::new());
        let settings = settings(2, 10_000, 7);

        history.record_text_at("one", 1_000, &settings);
        history.record_text_at("two", 2_000, &settings);
        history.record_text_at("three", 3_000, &settings);

        assert_eq!(
            history
                .entries()
                .iter()
                .map(|entry| entry.normalized_text.as_str())
                .collect::<Vec<_>>(),
            ["three", "two"]
        );
    }

    #[test]
    fn expired_entries_are_pruned() {
        let mut history = history_with(vec![
            entry("clip:old", "old", 1_000),
            entry("clip:fresh", "fresh", MILLIS_PER_DAY + 1_000),
        ]);

        history.prune_expired_at((2 * MILLIS_PER_DAY) + 1_000, 1);

        assert_eq!(history.entries().len(), 1);
        assert_eq!(history.entries()[0].id, "clip:fresh");
    }

    #[test]
    fn delete_item_removes_one_matching_entry() {
        let mut history = history_with(vec![
            entry("clip:first", "first", 1_000),
            entry("clip:second", "second", 2_000),
        ]);

        assert!(history.delete_item("clip:first"));
        assert!(!history.delete_item("clip:missing"));
        assert_eq!(history.entries().len(), 1);
        assert_eq!(history.entries()[0].id, "clip:second");
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut history = history_with(vec![entry("clip:first", "first", 1_000)]);

        history.clear();

        assert!(history.entries().is_empty());
    }

    #[test]
    fn serialization_round_trip_preserves_entries() {
        let (root, path) = temporary_history("round-trip");
        let settings = settings(100, 10_000, 7);
        let mut history = ClipboardHistory {
            path: path.clone(),
            entries: Vec::new(),
        };
        history.record_text_at("wifi", 1_700, &settings);
        history.record_text_at("display", 1_800, &settings);

        history.save().expect("history should save");
        let loaded = ClipboardHistory::load(path);

        assert_eq!(loaded.entries(), history.entries());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn missing_file_loads_empty_history() {
        let (root, path) = temporary_history("missing");

        let history = ClipboardHistory::load(path);

        assert!(history.entries().is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn corrupted_json_loads_empty_history() {
        let (root, path) = temporary_history("corrupted");
        fs::write(&path, "{not-json").expect("corrupted history should be written");

        let history = ClipboardHistory::load(path);

        assert!(history.entries().is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn unsupported_version_loads_empty_history() {
        let (root, path) = temporary_history("unsupported-version");
        fs::write(
            &path,
            serde_json::to_string_pretty(&json!({
                "version": 999,
                "entries": []
            }))
            .expect("history file should serialize"),
        )
        .expect("history should be written");

        let history = ClipboardHistory::load(path);

        assert!(history.entries().is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn save_does_not_leave_temp_file_after_success() {
        let (root, path) = temporary_history("temp-file");
        let settings = settings(100, 10_000, 7);
        let mut history = ClipboardHistory {
            path: path.clone(),
            entries: Vec::new(),
        };
        history.record_text_at("wifi", 1_700, &settings);

        history.save().expect("history should save");

        assert!(path.exists());
        assert!(!temporary_history_path(&path).exists());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }
}
