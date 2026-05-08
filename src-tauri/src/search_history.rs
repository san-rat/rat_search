#![allow(dead_code)]

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    search_result::{SearchAction, SearchMetadata, SearchResult, SearchSource},
    settings,
};

const HISTORY_VERSION: u32 = 1;
const MAX_HISTORY_ENTRIES: usize = 100;
const PREFIX_MATCH_SCORE: i32 = 420;
const CONTAINS_MATCH_SCORE: i32 = 180;
const FREQUENCY_BOOST_PER_USE: i32 = 10;
const MAX_HISTORY_SCORE: i32 = 520;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchHistory {
    path: PathBuf,
    entries: Vec<SearchHistoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SearchHistoryEntry {
    pub(crate) query: String,
    pub(crate) last_used_ms: u64,
    pub(crate) use_count: u32,
}

impl SearchHistory {
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
                    "failed to read search history '{}': {error}",
                    path.display()
                );
                return Self {
                    path,
                    entries: Vec::new(),
                };
            }
        };

        let file = match serde_json::from_str::<HistoryFile>(&contents) {
            Ok(file) if file.version == HISTORY_VERSION => file,
            Ok(file) => {
                eprintln!(
                    "unsupported search history version {} in '{}'",
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
                    "failed to parse search history '{}': {error}",
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
        history.sort_and_cap_entries();
        history
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create search history directory '{}': {error}",
                    parent.display()
                )
            })?;
        }

        let file = HistoryFile {
            version: HISTORY_VERSION,
            entries: self.entries.clone(),
        };
        let contents = serde_json::to_string_pretty(&file)
            .map_err(|error| format!("failed to serialize search history: {error}"))?;
        let temp_path = temporary_history_path(&self.path);

        fs::write(&temp_path, contents).map_err(|error| {
            format!(
                "failed to write temporary search history '{}': {error}",
                temp_path.display()
            )
        })?;
        fs::rename(&temp_path, &self.path).map_err(|error| {
            format!(
                "failed to persist search history '{}' from '{}': {error}",
                self.path.display(),
                temp_path.display()
            )
        })
    }

    pub(crate) fn record_query_at(&mut self, query: &str, last_used_ms: u64) {
        let query = normalize_query(query);

        if query.is_empty() {
            return;
        }

        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.query == query) {
            entry.last_used_ms = last_used_ms;
            entry.use_count = entry.use_count.saturating_add(1);
        } else {
            self.entries.push(SearchHistoryEntry {
                query,
                last_used_ms,
                use_count: 1,
            });
        }

        self.sort_and_cap_entries();
    }

    pub(crate) fn entries(&self) -> &[SearchHistoryEntry] {
        &self.entries
    }

    fn sort_and_cap_entries(&mut self) {
        self.entries.sort_by(|left, right| {
            right.last_used_ms.cmp(&left.last_used_ms).then_with(|| {
                normalize_for_match(&left.query).cmp(&normalize_for_match(&right.query))
            })
        });
        self.entries.truncate(MAX_HISTORY_ENTRIES);
    }
}

pub(crate) fn normalize_query(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn search_history(
    history: &SearchHistory,
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    let query = normalize_for_match(&normalize_query(query));
    let limit = settings::normalize_result_limit(limit);

    if query.is_empty() {
        return Vec::new();
    }

    let mut matches = history
        .entries()
        .iter()
        .filter_map(|entry| {
            let score = score_entry(entry, &query);
            (score > 0).then_some((entry, score))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| right.last_used_ms.cmp(&left.last_used_ms))
            .then_with(|| normalize_for_match(&left.query).cmp(&normalize_for_match(&right.query)))
    });

    matches
        .into_iter()
        .take(limit)
        .map(|(entry, score)| result_from_entry(entry, score))
        .collect()
}

fn score_entry(entry: &SearchHistoryEntry, query: &str) -> i32 {
    let entry_query = normalize_for_match(&entry.query);
    let base_score = if entry_query.starts_with(query) {
        PREFIX_MATCH_SCORE
    } else if entry_query.contains(query) {
        CONTAINS_MATCH_SCORE
    } else {
        return 0;
    };
    let frequency_boost = entry
        .use_count
        .saturating_sub(1)
        .min(10)
        .try_into()
        .map(|use_count: i32| use_count * FREQUENCY_BOOST_PER_USE)
        .unwrap_or(0);

    (base_score + frequency_boost).min(MAX_HISTORY_SCORE)
}

fn result_from_entry(entry: &SearchHistoryEntry, score: i32) -> SearchResult {
    SearchResult {
        id: format!("history:{}", normalize_query(&entry.query)),
        title: entry.query.clone(),
        subtitle: Some(format!("Search history - used {} times", entry.use_count)),
        icon: Some("history".to_owned()),
        source: SearchSource::History,
        action: SearchAction::ReuseQuery,
        path: None,
        score,
        metadata: Some(SearchMetadata::History {
            query: entry.query.clone(),
            last_used_ms: entry.last_used_ms,
            use_count: entry.use_count,
        }),
    }
}

fn sanitize_entries(entries: Vec<SearchHistoryEntry>) -> Vec<SearchHistoryEntry> {
    let mut sanitized = Vec::new();

    for entry in entries {
        let query = normalize_query(&entry.query);

        if query.is_empty() {
            continue;
        }

        sanitized.push(SearchHistoryEntry {
            query,
            last_used_ms: entry.last_used_ms,
            use_count: entry.use_count.max(1),
        });
    }

    sanitized
}

fn normalize_for_match(value: &str) -> String {
    normalize_query(value).to_lowercase()
}

fn temporary_history_path(path: &Path) -> PathBuf {
    match path.file_name().and_then(|file_name| file_name.to_str()) {
        Some(file_name) => path.with_file_name(format!("{file_name}.tmp")),
        None => path.with_extension("tmp"),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct HistoryFile {
    version: u32,
    entries: Vec<SearchHistoryEntry>,
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
        let path = root.join("history.json");

        (root, path)
    }

    fn history_with(entries: Vec<SearchHistoryEntry>) -> SearchHistory {
        let mut history = SearchHistory {
            path: PathBuf::from("/tmp/history.json"),
            entries,
        };
        history.sort_and_cap_entries();
        history
    }

    fn entry(query: &str, last_used_ms: u64, use_count: u32) -> SearchHistoryEntry {
        SearchHistoryEntry {
            query: query.to_owned(),
            last_used_ms,
            use_count,
        }
    }

    #[test]
    fn query_normalization_trims_and_collapses_whitespace() {
        assert_eq!(normalize_query("  wifi   settings  "), "wifi settings");
    }

    #[test]
    fn empty_normalized_queries_are_ignored() {
        let mut history = history_with(Vec::new());

        history.record_query_at("   ", 1);

        assert!(history.entries().is_empty());
    }

    #[test]
    fn new_query_insert_sets_timestamp_and_use_count() {
        let mut history = history_with(Vec::new());

        history.record_query_at(" wifi ", 1_700);

        assert_eq!(history.entries(), &[entry("wifi", 1_700, 1)]);
    }

    #[test]
    fn existing_query_update_increments_count_and_refreshes_timestamp() {
        let mut history = history_with(vec![entry("wifi", 1_000, 1)]);

        history.record_query_at("wifi", 2_000);

        assert_eq!(history.entries(), &[entry("wifi", 2_000, 2)]);
    }

    #[test]
    fn entries_cap_at_one_hundred_and_sort_newest_first() {
        let mut history = history_with(Vec::new());

        for index in 0..105 {
            history.record_query_at(&format!("query {index:03}"), index);
        }

        assert_eq!(history.entries().len(), 100);
        assert_eq!(history.entries()[0].query, "query 104");
        assert_eq!(history.entries()[99].query, "query 005");
    }

    #[test]
    fn serialization_round_trip_preserves_entries() {
        let (root, path) = temporary_history("round-trip");
        let mut history = SearchHistory {
            path: path.clone(),
            entries: Vec::new(),
        };
        history.record_query_at("wifi", 1_700);
        history.record_query_at("display", 1_800);

        history.save().expect("history should save");
        let loaded = SearchHistory::load(path);

        assert_eq!(loaded.entries(), history.entries());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn missing_file_loads_empty_history() {
        let (root, path) = temporary_history("missing");

        let loaded = SearchHistory::load(path);

        assert!(loaded.entries().is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn corrupted_json_loads_empty_history() {
        let (root, path) = temporary_history("corrupted");
        fs::write(&path, "{not-json").expect("corrupted history should be written");

        let loaded = SearchHistory::load(path);

        assert!(loaded.entries().is_empty());

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn search_prefix_matches_outrank_contains_matches() {
        let history = history_with(vec![
            entry("my rust notes", 2_000, 10),
            entry("rust docs", 1_000, 1),
        ]);
        let results = search_history(&history, "rust", 8);

        assert_eq!(results[0].title, "rust docs");
        assert_eq!(results[0].score, PREFIX_MATCH_SCORE);
        assert_eq!(results[1].title, "my rust notes");
    }

    #[test]
    fn frequency_boost_affects_similar_matches_without_exceeding_cap() {
        let history = history_with(vec![
            entry("rust book", 1_000, 1),
            entry("rust blog", 900, 20),
        ]);
        let results = search_history(&history, "rust b", 8);

        assert_eq!(results[0].title, "rust blog");
        assert_eq!(results[0].score, MAX_HISTORY_SCORE);
        assert!(results[1].score < results[0].score);
    }

    #[test]
    fn result_uses_expected_frontend_shape() {
        let history = history_with(vec![entry("wifi", 1_700_000_000_123, 3)]);
        let result = search_history(&history, "wifi", 8)
            .into_iter()
            .next()
            .expect("history result");

        assert_eq!(
            serde_json::to_value(result).expect("result should serialize"),
            json!({
                "id": "history:wifi",
                "title": "wifi",
                "subtitle": "Search history - used 3 times",
                "icon": "history",
                "source": "history",
                "action": "reuse_query",
                "path": null,
                "score": 440,
                "metadata": {
                    "kind": "history",
                    "query": "wifi",
                    "last_used_ms": 1_700_000_000_123_u64,
                    "use_count": 3
                }
            })
        );
    }
}
