use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    file_icons,
    file_index::{FileIndex, FileRecord},
    search_result::{SearchAction, SearchMetadata, SearchResult, SearchSource},
    settings,
};

const EXACT_FILE_NAME_SCORE: i32 = 930;
const FILE_NAME_PREFIX_SCORE: i32 = 840;
const FILE_NAME_CONTAINS_SCORE: i32 = 560;
const ABBREVIATION_SCORE: i32 = 390;
const SUBSEQUENCE_SCORE: i32 = 220;
const FOLDER_BOOST: i32 = 25;
const SHORT_NAME_BONUS_LIMIT: i32 = 50;
const RECENT_ONE_DAY_BOOST: i32 = 20;
const RECENT_SEVEN_DAY_BOOST: i32 = 12;
const RECENT_THIRTY_DAY_BOOST: i32 = 6;

pub(crate) fn search_files(index: &FileIndex, query: &str, limit: usize) -> Vec<SearchResult> {
    let query = normalize(query);
    let limit = settings::normalize_result_limit(limit);

    if query.is_empty() {
        return Vec::new();
    }

    let now = SystemTime::now();
    let mut matches = index
        .records()
        .iter()
        .filter_map(|record| {
            let score = score_record(record, &query, now);
            (score > 0).then_some((record, score))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|(left_record, left_score), (right_record, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| compare_records(left_record, right_record))
    });

    matches
        .into_iter()
        .take(limit)
        .map(|(record, score)| search_result_from_record(record, score))
        .collect()
}

fn score_record(record: &FileRecord, query: &str, now: SystemTime) -> i32 {
    let normalized_name = normalize(&record.file_name);
    let base_score = score_name(&normalized_name, query);

    if base_score == 0 {
        return 0;
    }

    base_score + folder_boost(record) + recent_boost(record, now)
}

fn score_name(name: &str, query: &str) -> i32 {
    let base_score = if name == query {
        EXACT_FILE_NAME_SCORE
    } else if name.starts_with(query) {
        FILE_NAME_PREFIX_SCORE
    } else if name.contains(query) {
        FILE_NAME_CONTAINS_SCORE
    } else if abbreviation_matches(name, query) {
        ABBREVIATION_SCORE
    } else if subsequence_matches(name, query) {
        SUBSEQUENCE_SCORE
    } else {
        0
    };

    if base_score >= FILE_NAME_CONTAINS_SCORE {
        base_score + short_name_bonus(name)
    } else {
        base_score
    }
}

fn folder_boost(record: &FileRecord) -> i32 {
    if record.is_dir {
        FOLDER_BOOST
    } else {
        0
    }
}

fn recent_boost(record: &FileRecord, now: SystemTime) -> i32 {
    let Some(modified_time) = record.modified_time else {
        return 0;
    };
    let Ok(age) = now.duration_since(modified_time) else {
        return RECENT_ONE_DAY_BOOST;
    };

    if age <= Duration::from_secs(60 * 60 * 24) {
        RECENT_ONE_DAY_BOOST
    } else if age <= Duration::from_secs(60 * 60 * 24 * 7) {
        RECENT_SEVEN_DAY_BOOST
    } else if age <= Duration::from_secs(60 * 60 * 24 * 30) {
        RECENT_THIRTY_DAY_BOOST
    } else {
        0
    }
}

fn short_name_bonus(name: &str) -> i32 {
    let length = name.chars().filter(|ch| !ch.is_whitespace()).count() as i32;
    (SHORT_NAME_BONUS_LIMIT - length).max(0)
}

fn abbreviation_matches(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return false;
    }

    let initials = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .collect::<String>();

    initials == query
        || initials.starts_with(query)
        || (!initials.is_empty()
            && query.starts_with(&initials)
            && subsequence_matches(name, query))
}

fn subsequence_matches(name: &str, query: &str) -> bool {
    if query.is_empty() {
        return false;
    }

    let mut query_chars = query.chars();
    let mut current = query_chars.next();

    for ch in name.chars() {
        if Some(ch) == current {
            current = query_chars.next();

            if current.is_none() {
                return true;
            }
        }
    }

    false
}

fn compare_records(left: &FileRecord, right: &FileRecord) -> std::cmp::Ordering {
    normalize(&left.file_name)
        .cmp(&normalize(&right.file_name))
        .then_with(|| left.id.cmp(&right.id))
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn search_result_from_record(record: &FileRecord, score: i32) -> SearchResult {
    let source = if record.is_dir {
        SearchSource::Folders
    } else {
        SearchSource::Files
    };
    let metadata = if record.is_dir {
        SearchMetadata::Folder
    } else {
        SearchMetadata::File {
            extension: record.extension.clone(),
            modified_time_ms: modified_time_ms(record.modified_time),
        }
    };

    SearchResult {
        id: record.id.clone(),
        title: record.file_name.clone(),
        subtitle: Some(record.parent_path.to_string_lossy().into_owned()),
        icon: Some(file_icons::icon_for_record(record).to_owned()),
        source,
        action: SearchAction::OpenPath,
        path: Some(record.path.to_string_lossy().into_owned()),
        score,
        metadata: Some(metadata),
    }
}

fn modified_time_ms(modified_time: Option<SystemTime>) -> Option<u64> {
    modified_time?
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{Duration, SystemTime},
    };

    use super::*;

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
    fn empty_query_returns_no_file_results() {
        let results = search_files(
            &index(vec![file("/home/sanuk/Documents/Report.pdf")]),
            "",
            8,
        );

        assert!(results.is_empty());
    }

    #[test]
    fn exact_prefix_contains_abbreviation_and_subsequence_matches_work() {
        let results = search_files(
            &index(vec![
                file("/home/sanuk/Documents/Report.pdf"),
                file("/home/sanuk/Documents/Reporter Notes.txt"),
                file("/home/sanuk/Documents/Annual Report.pdf"),
                file("/home/sanuk/Documents/Project Plan.txt"),
                file("/home/sanuk/Documents/Settings.md"),
            ]),
            "report",
            8,
        );

        assert_eq!(
            results
                .iter()
                .map(|result| result.title.as_str())
                .collect::<Vec<_>>(),
            ["Report.pdf", "Reporter Notes.txt", "Annual Report.pdf"]
        );

        let abbreviation_results = search_files(
            &index(vec![file("/home/sanuk/Documents/Project Plan.txt")]),
            "pp",
            8,
        );
        let subsequence_results = search_files(
            &index(vec![file("/home/sanuk/Documents/Settings.md")]),
            "stg",
            8,
        );

        assert_eq!(abbreviation_results[0].title, "Project Plan.txt");
        assert_eq!(subsequence_results[0].title, "Settings.md");
    }

    #[test]
    fn exact_and_prefix_matches_outrank_weak_matches() {
        let results = search_files(
            &index(vec![
                file("/home/sanuk/Documents/Annual Report.pdf"),
                file("/home/sanuk/Documents/Report.pdf"),
                file("/home/sanuk/Documents/Reporter.txt"),
            ]),
            "report",
            8,
        );

        assert_eq!(
            results
                .iter()
                .map(|result| result.title.as_str())
                .collect::<Vec<_>>(),
            ["Report.pdf", "Reporter.txt", "Annual Report.pdf"]
        );
    }

    #[test]
    fn folder_boost_affects_tied_file_and_folder_matches() {
        let results = search_files(
            &index(vec![
                file("/home/sanuk/Documents/Archive"),
                folder("/home/sanuk/Documents/Archive"),
            ]),
            "archive",
            8,
        );

        assert_eq!(results[0].source, SearchSource::Folders);
        assert_eq!(results[1].source, SearchSource::Files);
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn recent_boost_affects_tied_records_without_dominating_match_quality() {
        let now = SystemTime::now();
        let mut recent_contains = file("/home/sanuk/Documents/Annual Report.pdf");
        recent_contains.modified_time = Some(now);
        let mut old_exact = file("/home/sanuk/Documents/Report.pdf");
        old_exact.modified_time = Some(now - Duration::from_secs(60 * 60 * 24 * 60));
        let mut recent_exact = file("/home/sanuk/Documents/Report Copy.pdf");
        recent_exact.modified_time = Some(now);

        let results = search_files(
            &index(vec![
                recent_contains,
                old_exact.clone(),
                recent_exact.clone(),
            ]),
            "report",
            8,
        );

        assert_eq!(results[0].title, recent_exact.file_name);
        assert_eq!(results[1].title, old_exact.file_name);
        assert_eq!(results[2].title, "Annual Report.pdf");
    }

    #[test]
    fn file_and_folder_results_use_shared_shape() {
        let results = search_files(
            &index(vec![
                file("/home/sanuk/Documents/Report.pdf"),
                folder("/home/sanuk/Documents/Reports"),
            ]),
            "report",
            8,
        );

        let folder_result = results
            .iter()
            .find(|result| result.source == SearchSource::Folders)
            .expect("folder result should exist");
        let file_result = results
            .iter()
            .find(|result| result.source == SearchSource::Files)
            .expect("file result should exist");

        assert_eq!(folder_result.action, SearchAction::OpenPath);
        assert_eq!(folder_result.icon.as_deref(), Some("folder"));
        assert!(folder_result
            .path
            .as_deref()
            .is_some_and(|path| path.ends_with("/Reports")));
        assert_eq!(folder_result.metadata, Some(SearchMetadata::Folder));

        assert_eq!(file_result.action, SearchAction::OpenPath);
        assert_eq!(file_result.icon.as_deref(), Some("file-pdf"));
        assert!(file_result
            .path
            .as_deref()
            .is_some_and(|path| path.ends_with("/Report.pdf")));
        assert_eq!(
            file_result.metadata,
            Some(SearchMetadata::File {
                extension: Some("pdf".to_owned()),
                modified_time_ms: None
            })
        );
    }

    #[test]
    fn file_metadata_includes_modified_time_without_changing_order() {
        let modified_time = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
        let mut report = file("/home/sanuk/Documents/Report.pdf");
        report.modified_time = Some(modified_time);
        let annual_report = file("/home/sanuk/Documents/Annual Report.pdf");

        let results = search_files(&index(vec![annual_report, report]), "report", 8);
        let report_result = results
            .iter()
            .find(|result| result.title == "Report.pdf")
            .expect("report result should exist");

        assert_eq!(results[0].title, "Report.pdf");
        assert_eq!(
            report_result.metadata,
            Some(SearchMetadata::File {
                extension: Some("pdf".to_owned()),
                modified_time_ms: Some(1_700_000_000_123)
            })
        );
    }
}
