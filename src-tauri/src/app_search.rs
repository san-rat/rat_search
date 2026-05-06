use serde::Serialize;

use crate::app_discovery::{AppCatalog, AppRecord};

const DEFAULT_RESULT_LIMIT: usize = 8;
const MAX_RESULT_LIMIT: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AppSearchResult {
    pub(crate) app_id: String,
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) terminal: bool,
}

pub(crate) fn search_apps(catalog: &AppCatalog, query: &str, limit: usize) -> Vec<AppSearchResult> {
    let query = query.trim().to_lowercase();
    let limit = normalize_limit(limit);

    let mut matches = catalog
        .apps
        .iter()
        .filter(|app| query.is_empty() || app_matches_query(app, &query))
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });

    matches
        .into_iter()
        .take(limit)
        .map(AppSearchResult::from)
        .collect()
}

fn normalize_limit(limit: usize) -> usize {
    match limit {
        0 => DEFAULT_RESULT_LIMIT,
        limit => limit.min(MAX_RESULT_LIMIT),
    }
}

fn app_matches_query(app: &AppRecord, query: &str) -> bool {
    contains_query(&app.name, query)
        || app
            .generic_name
            .as_deref()
            .is_some_and(|value| contains_query(value, query))
        || app
            .comment
            .as_deref()
            .is_some_and(|value| contains_query(value, query))
        || app
            .keywords
            .iter()
            .any(|keyword| contains_query(keyword, query))
        || app
            .categories
            .iter()
            .any(|category| contains_query(category, query))
}

fn contains_query(value: &str, query: &str) -> bool {
    value.to_lowercase().contains(query)
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
    fn no_match_returns_an_empty_list() {
        let results = search_apps(&catalog(vec![app("files.desktop", "Files")]), "firefox", 8);

        assert!(results.is_empty());
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
