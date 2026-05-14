#![allow(dead_code)]

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{
    clipboard_settings::ClipboardSettings,
    search_result::{SearchAction, SearchMetadata, SearchResult, SearchSource},
    settings,
};

const HISTORY_VERSION: u32 = 1;
const PREVIEW_CHAR_LIMIT: usize = 120;
const MILLIS_PER_DAY: u64 = 24 * 60 * 60 * 1_000;
const EXACT_MATCH_SCORE: i32 = 560;
const PREFIX_MATCH_SCORE: i32 = 500;
const CONTAINS_MATCH_SCORE: i32 = 420;
const SUBSEQUENCE_MATCH_SCORE: i32 = 280;
const MAX_RECENCY_BOOST: i32 = 40;
const MAX_USE_COUNT_BOOST: i32 = 30;
const RECENCY_WINDOW_MS: u64 = 7 * MILLIS_PER_DAY;

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
    IgnoredSensitive,
    IgnoredDisabled,
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

        if !should_store_clipboard_text(text, settings) {
            return ClipboardRecordOutcome::IgnoredSensitive;
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

pub(crate) fn should_store_clipboard_text(text: &str, settings: &ClipboardSettings) -> bool {
    let normalized_text = normalize_clipboard_text(text);

    if normalized_text.is_empty() || text.len() > settings.max_text_bytes() as usize {
        return false;
    }

    let lowercase_text = normalized_text.to_lowercase();

    !looks_like_private_key(&lowercase_text) && !looks_like_secret_label(&lowercase_text)
}

pub(crate) fn search_clipboard(
    history: &ClipboardHistory,
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    search_clipboard_at(history, query, limit, current_time_ms())
}

fn search_clipboard_at(
    history: &ClipboardHistory,
    query: &str,
    limit: usize,
    now_ms: u64,
) -> Vec<SearchResult> {
    let query = normalize_for_match(query);
    let limit = settings::normalize_result_limit(limit);

    if query.is_empty() {
        return Vec::new();
    }

    let mut matches = history
        .entries()
        .iter()
        .filter_map(|entry| {
            let base_score = score_entry_match(entry, &query)?;
            let score = (base_score + recency_boost(entry, now_ms) + use_count_boost(entry))
                .min(EXACT_MATCH_SCORE);

            Some((entry, score))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| right.copied_at_ms.cmp(&left.copied_at_ms))
            .then_with(|| left.preview.cmp(&right.preview))
            .then_with(|| left.id.cmp(&right.id))
    });

    matches
        .into_iter()
        .take(limit)
        .map(|(entry, score)| result_from_entry(entry, score, now_ms))
        .collect()
}

fn score_entry_match(entry: &ClipboardHistoryEntry, query: &str) -> Option<i32> {
    let preview = normalize_for_match(&entry.preview);
    let normalized_text = normalize_for_match(&entry.normalized_text);
    let text = normalize_for_match(&entry.text);

    if [preview.as_str(), normalized_text.as_str(), text.as_str()]
        .iter()
        .any(|value| *value == query)
    {
        return Some(EXACT_MATCH_SCORE);
    }

    if [preview.as_str(), normalized_text.as_str(), text.as_str()]
        .iter()
        .any(|value| value.starts_with(query))
    {
        return Some(PREFIX_MATCH_SCORE);
    }

    if [preview.as_str(), normalized_text.as_str(), text.as_str()]
        .iter()
        .any(|value| value.contains(query))
    {
        return Some(CONTAINS_MATCH_SCORE);
    }

    if [preview.as_str(), normalized_text.as_str(), text.as_str()]
        .iter()
        .any(|value| is_subsequence(query, value))
    {
        return Some(SUBSEQUENCE_MATCH_SCORE);
    }

    None
}

fn recency_boost(entry: &ClipboardHistoryEntry, now_ms: u64) -> i32 {
    let age_ms = now_ms.saturating_sub(entry.copied_at_ms);

    if age_ms >= RECENCY_WINDOW_MS {
        return 0;
    }

    let remaining_ms = RECENCY_WINDOW_MS - age_ms;

    ((remaining_ms.saturating_mul(MAX_RECENCY_BOOST as u64)) / RECENCY_WINDOW_MS) as i32
}

fn use_count_boost(entry: &ClipboardHistoryEntry) -> i32 {
    entry.use_count.min(MAX_USE_COUNT_BOOST as u32) as i32
}

fn result_from_entry(entry: &ClipboardHistoryEntry, score: i32, now_ms: u64) -> SearchResult {
    SearchResult {
        id: format!("clipboard:{}", entry.id),
        title: entry.preview.clone(),
        subtitle: Some(format!(
            "Clipboard - copied {}",
            relative_time_label(entry.copied_at_ms, now_ms)
        )),
        icon: Some("clipboard".to_owned()),
        source: SearchSource::Clipboard,
        action: SearchAction::CopyClipboardText,
        path: None,
        score,
        metadata: Some(SearchMetadata::Clipboard {
            item_id: entry.id.clone(),
            preview: entry.preview.clone(),
            copied_at_ms: entry.copied_at_ms,
            last_used_ms: entry.last_used_ms,
            use_count: entry.use_count,
            text_len: entry.text_len,
        }),
    }
}

fn relative_time_label(copied_at_ms: u64, now_ms: u64) -> String {
    let age_seconds = now_ms.saturating_sub(copied_at_ms) / 1_000;

    if age_seconds < 60 {
        "just now".to_owned()
    } else if age_seconds < 60 * 60 {
        format!("{}m ago", age_seconds / 60)
    } else if age_seconds < 24 * 60 * 60 {
        format!("{}h ago", age_seconds / (60 * 60))
    } else {
        format!("{}d ago", age_seconds / (24 * 60 * 60))
    }
}

fn normalize_for_match(value: &str) -> String {
    normalize_clipboard_text(value).to_lowercase()
}

fn is_subsequence(query: &str, value: &str) -> bool {
    let mut query_chars = query.chars();
    let Some(mut query_char) = query_chars.next() else {
        return true;
    };

    for value_char in value.chars() {
        if value_char == query_char {
            match query_chars.next() {
                Some(next_query_char) => query_char = next_query_char,
                None => return true,
            }
        }
    }

    false
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn looks_like_private_key(value: &str) -> bool {
    value.contains("begin private key")
        || value.contains("begin rsa private key")
        || value.contains("begin openssh private key")
}

fn looks_like_secret_label(value: &str) -> bool {
    const SECRET_TERMS: [&str; 8] = [
        "password",
        "passwd",
        "secret",
        "token",
        "api_key",
        "apikey",
        "access_key",
        "private key",
    ];

    SECRET_TERMS
        .iter()
        .any(|term| has_secret_label(value, term))
}

fn has_secret_label(value: &str, term: &str) -> bool {
    let mut search_start = 0;

    while let Some(relative_index) = value[search_start..].find(term) {
        let term_start = search_start + relative_index;
        let term_end = term_start + term.len();

        if has_label_boundary(value, term_start, term_end) && has_label_value(value, term_end) {
            return true;
        }

        search_start = term_end;
    }

    false
}

fn has_label_boundary(value: &str, term_start: usize, term_end: usize) -> bool {
    let before = value[..term_start].chars().next_back();
    let after = value[term_end..].chars().next();

    !before.is_some_and(is_label_char) && !after.is_some_and(is_label_char)
}

fn has_label_value(value: &str, term_end: usize) -> bool {
    let mut chars = value[term_end..].chars().skip_while(|character| {
        character.is_whitespace() || matches!(character, ':' | '=' | '-' | '_')
    });

    chars.next().is_some()
}

fn is_label_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
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

    fn used_entry(
        id: &str,
        text: &str,
        copied_at_ms: u64,
        last_used_ms: Option<u64>,
        use_count: u32,
    ) -> ClipboardHistoryEntry {
        let mut entry = entry(id, text, copied_at_ms);
        entry.last_used_ms = last_used_ms;
        entry.use_count = use_count;
        entry
    }

    fn assert_rejected_without_secret_output(sample: &str, settings: &ClipboardSettings) {
        assert!(!should_store_clipboard_text(sample, settings));
    }

    #[test]
    fn clipboard_search_empty_query_returns_no_results() {
        let history = history_with(vec![entry("clip:one", "Alpha", 1_000)]);

        assert!(search_clipboard_at(&history, "   ", 8, 2_000).is_empty());
    }

    #[test]
    fn clipboard_search_exact_matches_outrank_contains_matches() {
        let history = history_with(vec![
            entry("clip:contains", "alpha release notes", 1_000),
            entry("clip:exact", "alpha", 900),
        ]);

        let results = search_clipboard_at(&history, "alpha", 8, RECENCY_WINDOW_MS + 10_000);

        assert_eq!(
            results[0].metadata.as_ref().and_then(clipboard_item_id),
            Some("clip:exact")
        );
        assert_eq!(results[0].score, EXACT_MATCH_SCORE);
        assert_eq!(
            results[1].metadata.as_ref().and_then(clipboard_item_id),
            Some("clip:contains")
        );
    }

    #[test]
    fn clipboard_search_prefix_matches_outrank_contains_matches() {
        let history = history_with(vec![
            entry("clip:contains", "my alpha notes", 1_000),
            entry("clip:prefix", "alphabet soup", 900),
        ]);

        let results = search_clipboard_at(&history, "alpha", 8, RECENCY_WINDOW_MS + 10_000);

        assert_eq!(
            results[0].metadata.as_ref().and_then(clipboard_item_id),
            Some("clip:prefix")
        );
        assert_eq!(results[0].score, PREFIX_MATCH_SCORE);
        assert_eq!(
            results[1].metadata.as_ref().and_then(clipboard_item_id),
            Some("clip:contains")
        );
    }

    #[test]
    fn clipboard_search_subsequence_matches_below_stronger_matches() {
        let history = history_with(vec![
            entry("clip:subsequence", "alphabet", 1_000),
            entry("clip:contains", "my apt notes", 900),
        ]);

        let results = search_clipboard_at(&history, "apt", 8, RECENCY_WINDOW_MS + 10_000);

        assert_eq!(
            results[0].metadata.as_ref().and_then(clipboard_item_id),
            Some("clip:contains")
        );
        assert_eq!(
            results[1].metadata.as_ref().and_then(clipboard_item_id),
            Some("clip:subsequence")
        );
        assert_eq!(results[1].score, SUBSEQUENCE_MATCH_SCORE);
    }

    #[test]
    fn clipboard_search_recent_entries_get_small_boost() {
        let history = history_with(vec![
            entry("clip:old", "alpha old", 1_000),
            entry("clip:new", "alpha new", RECENCY_WINDOW_MS),
        ]);

        let results = search_clipboard_at(&history, "alpha", 8, RECENCY_WINDOW_MS + 1_000);

        assert_eq!(
            results[0].metadata.as_ref().and_then(clipboard_item_id),
            Some("clip:new")
        );
        assert!(results[0].score > results[1].score);
        assert!(results[0].score <= PREFIX_MATCH_SCORE + MAX_RECENCY_BOOST);
    }

    #[test]
    fn clipboard_search_frequency_boost_is_capped() {
        let history = history_with(vec![used_entry(
            "clip:used",
            "my alpha note",
            1_000,
            Some(2_000),
            100,
        )]);

        let result = search_clipboard_at(&history, "alpha", 8, RECENCY_WINDOW_MS + 10_000)
            .into_iter()
            .next()
            .expect("clipboard result");

        assert_eq!(result.score, CONTAINS_MATCH_SCORE + MAX_USE_COUNT_BOOST);
    }

    #[test]
    fn clipboard_search_limit_is_applied_after_ranking() {
        let history = history_with(vec![
            entry("clip:one", "alpha one", 1_000),
            entry("clip:two", "alpha two", 2_000),
            entry("clip:three", "alpha three", 3_000),
        ]);

        let results = search_clipboard_at(&history, "alpha", 2, 4_000);

        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].metadata.as_ref().and_then(clipboard_item_id),
            Some("clip:three")
        );
        assert_eq!(
            results[1].metadata.as_ref().and_then(clipboard_item_id),
            Some("clip:two")
        );
    }

    #[test]
    fn clipboard_search_result_uses_expected_frontend_shape_without_full_text() {
        let history = history_with(vec![used_entry(
            "clip:result",
            "alpha private long body",
            1_700_000_000_000,
            Some(1_700_000_001_000),
            2,
        )]);

        let result = search_clipboard_at(&history, "alpha", 8, 1_700_000_030_000)
            .into_iter()
            .next()
            .expect("clipboard result");
        let value = serde_json::to_value(result).expect("result should serialize");

        assert_eq!(
            value,
            json!({
                "id": "clipboard:clip:result",
                "title": "alpha private long body",
                "subtitle": "Clipboard - copied just now",
                "icon": "clipboard",
                "source": "clipboard",
                "action": "copy_clipboard_text",
                "path": null,
                "score": 541,
                "metadata": {
                    "kind": "clipboard",
                    "item_id": "clip:result",
                    "preview": "alpha private long body",
                    "copied_at_ms": 1_700_000_000_000_u64,
                    "last_used_ms": 1_700_000_001_000_u64,
                    "use_count": 2,
                    "text_len": 23
                }
            })
        );
        assert!(value["metadata"].get("text").is_none());
        assert!(value["metadata"].get("normalized_text").is_none());
    }

    fn clipboard_item_id(metadata: &SearchMetadata) -> Option<&str> {
        match metadata {
            SearchMetadata::Clipboard { item_id, .. } => Some(item_id.as_str()),
            _ => None,
        }
    }

    #[test]
    fn normal_short_text_is_accepted_by_sensitive_filter() {
        let settings = settings(100, 10_000, 7);

        assert!(should_store_clipboard_text(
            "meeting notes for tomorrow",
            &settings
        ));
    }

    #[test]
    fn empty_text_is_rejected_by_sensitive_filter() {
        let settings = settings(100, 10_000, 7);

        assert!(!should_store_clipboard_text(" \n\t ", &settings));
    }

    #[test]
    fn over_limit_text_is_rejected_by_sensitive_filter() {
        let settings = settings(100, 3, 7);

        assert!(!should_store_clipboard_text("four", &settings));
    }

    #[test]
    fn private_key_blocks_are_rejected() {
        let settings = settings(100, 10_000, 7);

        assert_rejected_without_secret_output("-----BEGIN PRIVATE KEY----- abc", &settings);
        assert_rejected_without_secret_output("-----BEGIN RSA PRIVATE KEY----- abc", &settings);
        assert_rejected_without_secret_output("-----BEGIN OPENSSH PRIVATE KEY----- abc", &settings);
    }

    #[test]
    fn password_labelled_text_is_rejected() {
        let settings = settings(100, 10_000, 7);

        assert_rejected_without_secret_output("password=example", &settings);
        assert_rejected_without_secret_output("passwd: example", &settings);
    }

    #[test]
    fn token_labelled_text_is_rejected() {
        let settings = settings(100, 10_000, 7);

        assert_rejected_without_secret_output("token: example", &settings);
        assert_rejected_without_secret_output("secret = example", &settings);
    }

    #[test]
    fn api_key_and_access_key_labels_are_rejected() {
        let settings = settings(100, 10_000, 7);

        assert_rejected_without_secret_output("api_key=example", &settings);
        assert_rejected_without_secret_output("apikey: example", &settings);
        assert_rejected_without_secret_output("access_key example", &settings);
    }

    #[test]
    fn record_text_ignores_sensitive_content() {
        let mut history = history_with(Vec::new());
        let settings = settings(100, 10_000, 7);

        let outcome = history.record_text_at("password=example", 1_700, &settings);

        assert!(matches!(outcome, ClipboardRecordOutcome::IgnoredSensitive));
        assert!(history.entries().is_empty());
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
