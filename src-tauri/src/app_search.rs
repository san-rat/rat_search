use serde::Serialize;

use crate::{
    app_discovery::{AppCatalog, AppRecord},
    settings,
};

const EXACT_NAME_SCORE: i32 = 1000;
const NAME_PREFIX_SCORE: i32 = 900;
const KEYWORD_CATEGORY_EXACT_SCORE: i32 = 760;
const NAME_CONTAINS_SCORE: i32 = 650;
const DESCRIPTION_CONTAINS_SCORE: i32 = 480;
const ABBREVIATION_SCORE: i32 = 420;
const SUBSEQUENCE_SCORE: i32 = 260;
const SHORT_NAME_BONUS_LIMIT: i32 = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AppSearchResult {
    pub(crate) app_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) terminal: bool,
}

pub(crate) fn search_apps(catalog: &AppCatalog, query: &str, limit: usize) -> Vec<AppSearchResult> {
    let query = normalize(query);
    let limit = normalize_limit(limit);

    if query.is_empty() {
        let mut apps = catalog.apps.iter().collect::<Vec<_>>();
        apps.sort_by(compare_apps_alphabetically);

        return apps
            .into_iter()
            .take(limit)
            .map(AppSearchResult::from)
            .collect();
    }

    let mut matches = catalog
        .apps
        .iter()
        .filter_map(|app| {
            let score = score_app(app, &query);
            (score > 0).then_some((app, score))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|(left_app, left_score), (right_app, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| compare_apps_alphabetically(left_app, right_app))
    });

    matches
        .into_iter()
        .take(limit)
        .map(|(app, _score)| AppSearchResult::from(app))
        .collect()
}

fn normalize_limit(limit: usize) -> usize {
    match limit {
        0 => settings::DEFAULT_MAX_RESULTS,
        limit => limit.min(settings::RESULT_LIMIT_CAP),
    }
}

fn score_app(app: &AppRecord, query: &str) -> i32 {
    let normalized_name = normalize(&app.name);
    let mut best_score = score_name(&normalized_name, query);

    if exact_list_match(&app.keywords, query) || exact_list_match(&app.categories, query) {
        best_score = best_score.max(KEYWORD_CATEGORY_EXACT_SCORE);
    }

    if optional_contains(&app.generic_name, query) || optional_contains(&app.comment, query) {
        best_score = best_score.max(DESCRIPTION_CONTAINS_SCORE);
    }

    best_score
}

fn score_name(name: &str, query: &str) -> i32 {
    let base_score = if name == query {
        EXACT_NAME_SCORE
    } else if name.starts_with(query) {
        NAME_PREFIX_SCORE
    } else if name.contains(query) {
        NAME_CONTAINS_SCORE
    } else if abbreviation_matches(name, query) {
        ABBREVIATION_SCORE
    } else if subsequence_matches(name, query) {
        SUBSEQUENCE_SCORE
    } else {
        0
    };

    if base_score >= NAME_CONTAINS_SCORE {
        base_score + short_name_bonus(name)
    } else {
        base_score
    }
}

fn short_name_bonus(name: &str) -> i32 {
    let length = name.chars().filter(|ch| !ch.is_whitespace()).count() as i32;
    (SHORT_NAME_BONUS_LIMIT - length).max(0)
}

fn exact_list_match(values: &[String], query: &str) -> bool {
    values.iter().any(|value| normalize(value) == query)
}

fn optional_contains(value: &Option<String>, query: &str) -> bool {
    value
        .as_deref()
        .is_some_and(|value| normalize(value).contains(query))
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

fn compare_apps_alphabetically(left: &&AppRecord, right: &&AppRecord) -> std::cmp::Ordering {
    normalize(&left.name)
        .cmp(&normalize(&right.name))
        .then_with(|| left.id.cmp(&right.id))
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

impl From<&AppRecord> for AppSearchResult {
    fn from(app: &AppRecord) -> Self {
        Self {
            app_id: app.id.clone(),
            title: app.name.clone(),
            subtitle: app
                .comment
                .clone()
                .or_else(|| app.generic_name.clone())
                .or_else(|| app.categories.first().cloned()),
            icon: app.icon.clone(),
            terminal: app.terminal,
        }
    }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn empty_query_returns_limited_catalog_results() {
        let results = search_apps(
            &catalog(vec![
                app("bravo.desktop", "Bravo"),
                app("alpha.desktop", "Alpha"),
                app("charlie.desktop", "Charlie"),
            ]),
            "",
            2,
        );

        assert_eq!(
            results
                .iter()
                .map(|result| result.title.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "Bravo"]
        );
    }

    #[test]
    fn zero_limit_defaults_to_eight() {
        let apps = (0..10)
            .map(|index| app(&format!("app-{index}.desktop"), &format!("App {index}")))
            .collect::<Vec<_>>();

        let results = search_apps(&catalog(apps), "", 0);

        assert_eq!(results.len(), 8);
    }

    #[test]
    fn limit_clamps_to_twenty() {
        let apps = (0..25)
            .map(|index| app(&format!("app-{index}.desktop"), &format!("App {index:02}")))
            .collect::<Vec<_>>();

        let results = search_apps(&catalog(apps), "", 999);

        assert_eq!(results.len(), 20);
    }

    #[test]
    fn name_match_returns_an_app() {
        let results = search_apps(
            &catalog(vec![
                app("firefox.desktop", "Firefox"),
                app("files.desktop", "Files"),
            ]),
            "fire",
            8,
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, "firefox.desktop");
    }

    #[test]
    fn exact_name_match_ranks_first() {
        let results = search_apps(
            &catalog(vec![
                app("terminal-emulator.desktop", "Terminal Emulator"),
                app("terminal.desktop", "Terminal"),
            ]),
            "terminal",
            8,
        );

        assert_eq!(results[0].app_id, "terminal.desktop");
    }

    #[test]
    fn prefix_name_match_beats_contains_match() {
        let results = search_apps(
            &catalog(vec![
                app("wildfire.desktop", "Wildfire"),
                app("firefox.desktop", "Firefox"),
            ]),
            "fire",
            8,
        );

        assert_eq!(results[0].app_id, "firefox.desktop");
    }

    #[test]
    fn keyword_and_category_matches_return_apps() {
        let mut browser = app("browser.desktop", "Browser");
        browser.keywords = vec!["Web".to_owned(), "Internet".to_owned()];

        let mut settings = app("settings.desktop", "Settings");
        settings.categories = vec!["System".to_owned(), "Utility".to_owned()];

        let results = search_apps(&catalog(vec![browser, settings]), "utility", 8);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].app_id, "settings.desktop");
    }

    #[test]
    fn keyword_and_category_exact_match_beats_description_contains() {
        let mut exact_category = app("settings.desktop", "Settings");
        exact_category.categories = vec!["Utility".to_owned()];

        let mut comment_contains = app("manual.desktop", "Manual");
        comment_contains.comment = Some("Utility documentation".to_owned());

        let results = search_apps(
            &catalog(vec![comment_contains, exact_category]),
            "utility",
            8,
        );

        assert_eq!(results[0].app_id, "settings.desktop");
    }

    #[test]
    fn comment_and_generic_name_matches_return_apps() {
        let mut terminal = app("terminal.desktop", "Console");
        terminal.generic_name = Some("Terminal Emulator".to_owned());

        let mut browser = app("browser.desktop", "Web");
        browser.comment = Some("Browse the internet".to_owned());

        let generic_results = search_apps(&catalog(vec![terminal.clone()]), "emulator", 8);
        let comment_results = search_apps(&catalog(vec![browser]), "internet", 8);

        assert_eq!(generic_results[0].app_id, "terminal.desktop");
        assert_eq!(comment_results[0].app_id, "browser.desktop");
    }

    #[test]
    fn name_contains_beats_weak_fuzzy_match() {
        let results = search_apps(
            &catalog(vec![
                app("environment.desktop", "Cool Desktop Environment"),
                app("tool.desktop", "My Code Tool"),
            ]),
            "code",
            8,
        );

        assert_eq!(results[0].app_id, "tool.desktop");
    }

    #[test]
    fn abbreviation_match_can_find_firefox() {
        let results = search_apps(&catalog(vec![app("firefox.desktop", "Firefox")]), "ff", 8);

        assert_eq!(results[0].app_id, "firefox.desktop");
    }

    #[test]
    fn subsequence_match_can_find_settings() {
        let results = search_apps(
            &catalog(vec![app("settings.desktop", "Settings")]),
            "stg",
            8,
        );

        assert_eq!(results[0].app_id, "settings.desktop");
    }

    #[test]
    fn no_match_returns_an_empty_list() {
        let results = search_apps(&catalog(vec![app("files.desktop", "Files")]), "firefox", 8);

        assert!(results.is_empty());
    }

    #[test]
    fn score_ties_sort_alphabetically_then_by_id() {
        let mut zed_same = app("zed.desktop", "Same");
        zed_same.keywords = vec!["Utility".to_owned()];

        let mut alpha_same = app("alpha.desktop", "Same");
        alpha_same.keywords = vec!["Utility".to_owned()];

        let mut bravo = app("bravo.desktop", "Bravo");
        bravo.keywords = vec!["Utility".to_owned()];

        let results = search_apps(&catalog(vec![zed_same, alpha_same, bravo]), "utility", 8);

        assert_eq!(
            results
                .iter()
                .map(|result| result.app_id.as_str())
                .collect::<Vec<_>>(),
            ["bravo.desktop", "alpha.desktop", "zed.desktop"]
        );
    }

    #[test]
    fn subtitle_fallback_order_is_comment_generic_name_category() {
        let mut comment_app = app("comment.desktop", "Comment");
        comment_app.comment = Some("Comment subtitle".to_owned());
        comment_app.generic_name = Some("Generic subtitle".to_owned());
        comment_app.categories = vec!["Category subtitle".to_owned()];

        let mut generic_app = app("generic.desktop", "Generic");
        generic_app.generic_name = Some("Generic subtitle".to_owned());
        generic_app.categories = vec!["Category subtitle".to_owned()];

        let mut category_app = app("category.desktop", "Category");
        category_app.categories = vec!["Category subtitle".to_owned()];

        let results = search_apps(
            &catalog(vec![comment_app, generic_app, category_app]),
            "",
            8,
        );

        let subtitles = results
            .into_iter()
            .map(|result| (result.app_id, result.subtitle))
            .collect::<Vec<_>>();

        assert_eq!(
            subtitles,
            [
                (
                    "category.desktop".to_owned(),
                    Some("Category subtitle".to_owned())
                ),
                (
                    "comment.desktop".to_owned(),
                    Some("Comment subtitle".to_owned())
                ),
                (
                    "generic.desktop".to_owned(),
                    Some("Generic subtitle".to_owned())
                ),
            ]
        );
    }
}
