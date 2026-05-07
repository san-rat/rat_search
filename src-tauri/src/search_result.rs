#![allow(dead_code)]

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchSource {
    Applications,
    Files,
    Folders,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchAction {
    LaunchApp,
    OpenPath,
    RevealPath,
    CopyPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SearchResult {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) subtitle: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) source: SearchSource,
    pub(crate) action: SearchAction,
    pub(crate) path: Option<String>,
    pub(crate) score: i32,
    pub(crate) metadata: Option<SearchMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SearchMetadata {
    Application {
        app_id: String,
        terminal: bool,
    },
    File {
        extension: Option<String>,
        modified_time_ms: Option<u64>,
    },
    Folder,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn source_serializes_as_snake_case_strings() {
        assert_eq!(
            serde_json::to_value(SearchSource::Applications).expect("source should serialize"),
            json!("applications")
        );
        assert_eq!(
            serde_json::to_value(SearchSource::Files).expect("source should serialize"),
            json!("files")
        );
        assert_eq!(
            serde_json::to_value(SearchSource::Folders).expect("source should serialize"),
            json!("folders")
        );
    }

    #[test]
    fn action_serializes_as_snake_case_strings() {
        assert_eq!(
            serde_json::to_value(SearchAction::LaunchApp).expect("action should serialize"),
            json!("launch_app")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::OpenPath).expect("action should serialize"),
            json!("open_path")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::RevealPath).expect("action should serialize"),
            json!("reveal_path")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::CopyPath).expect("action should serialize"),
            json!("copy_path")
        );
    }

    #[test]
    fn result_serializes_to_frontend_shape() {
        let result = SearchResult {
            id: "firefox.desktop".to_owned(),
            title: "Firefox".to_owned(),
            subtitle: Some("Browse the web".to_owned()),
            icon: Some("firefox".to_owned()),
            source: SearchSource::Applications,
            action: SearchAction::LaunchApp,
            path: None,
            score: 950,
            metadata: Some(SearchMetadata::Application {
                app_id: "firefox.desktop".to_owned(),
                terminal: false,
            }),
        };

        assert_eq!(
            serde_json::to_value(result).expect("result should serialize"),
            json!({
                "id": "firefox.desktop",
                "title": "Firefox",
                "subtitle": "Browse the web",
                "icon": "firefox",
                "source": "applications",
                "action": "launch_app",
                "path": null,
                "score": 950,
                "metadata": {
                    "kind": "application",
                    "app_id": "firefox.desktop",
                    "terminal": false
                }
            })
        );
    }

    #[test]
    fn file_metadata_serializes_modified_time() {
        assert_eq!(
            serde_json::to_value(SearchMetadata::File {
                extension: Some("pdf".to_owned()),
                modified_time_ms: Some(1_700_000_000_123),
            })
            .expect("file metadata should serialize"),
            json!({
                "kind": "file",
                "extension": "pdf",
                "modified_time_ms": 1_700_000_000_123_u64
            })
        );
    }
}
