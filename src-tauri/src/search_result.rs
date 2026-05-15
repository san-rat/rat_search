#![allow(dead_code)]

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchSource {
    Applications,
    Files,
    Folders,
    Calculator,
    Web,
    Clipboard,
    Settings,
    History,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SearchAction {
    LaunchApp,
    OpenPath,
    OpenInCode,
    RevealPath,
    CopyPath,
    CopyText,
    OpenCalculatorApp,
    CopyClipboardText,
    DeleteClipboardItem,
    OpenUrl,
    OpenSetting,
    ReuseQuery,
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
    Calculator {
        expression: String,
        result: String,
        copy_text: String,
    },
    Web {
        shortcut: String,
        query: String,
        url: String,
    },
    Clipboard {
        item_id: String,
        preview: String,
        copied_at_ms: u64,
        last_used_ms: Option<u64>,
        use_count: u32,
        text_len: usize,
    },
    Setting {
        setting_id: String,
        panel: String,
        command: String,
    },
    History {
        query: String,
        last_used_ms: u64,
        use_count: u32,
    },
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
        assert_eq!(
            serde_json::to_value(SearchSource::Calculator).expect("source should serialize"),
            json!("calculator")
        );
        assert_eq!(
            serde_json::to_value(SearchSource::Web).expect("source should serialize"),
            json!("web")
        );
        assert_eq!(
            serde_json::to_value(SearchSource::Clipboard).expect("source should serialize"),
            json!("clipboard")
        );
        assert_eq!(
            serde_json::to_value(SearchSource::Settings).expect("source should serialize"),
            json!("settings")
        );
        assert_eq!(
            serde_json::to_value(SearchSource::History).expect("source should serialize"),
            json!("history")
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
            serde_json::to_value(SearchAction::OpenInCode).expect("action should serialize"),
            json!("open_in_code")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::RevealPath).expect("action should serialize"),
            json!("reveal_path")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::CopyPath).expect("action should serialize"),
            json!("copy_path")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::CopyText).expect("action should serialize"),
            json!("copy_text")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::OpenCalculatorApp).expect("action should serialize"),
            json!("open_calculator_app")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::CopyClipboardText).expect("action should serialize"),
            json!("copy_clipboard_text")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::DeleteClipboardItem)
                .expect("action should serialize"),
            json!("delete_clipboard_item")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::OpenUrl).expect("action should serialize"),
            json!("open_url")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::OpenSetting).expect("action should serialize"),
            json!("open_setting")
        );
        assert_eq!(
            serde_json::to_value(SearchAction::ReuseQuery).expect("action should serialize"),
            json!("reuse_query")
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

    #[test]
    fn calculator_metadata_serializes_to_frontend_shape() {
        assert_eq!(
            serde_json::to_value(SearchMetadata::Calculator {
                expression: "2+2".to_owned(),
                result: "4".to_owned(),
                copy_text: "4".to_owned(),
            })
            .expect("calculator metadata should serialize"),
            json!({
                "kind": "calculator",
                "expression": "2+2",
                "result": "4",
                "copy_text": "4"
            })
        );
    }

    #[test]
    fn web_metadata_serializes_to_frontend_shape() {
        assert_eq!(
            serde_json::to_value(SearchMetadata::Web {
                shortcut: "google".to_owned(),
                query: "what is rust".to_owned(),
                url: "https://www.google.com/search?q=what%20is%20rust".to_owned(),
            })
            .expect("web metadata should serialize"),
            json!({
                "kind": "web",
                "shortcut": "google",
                "query": "what is rust",
                "url": "https://www.google.com/search?q=what%20is%20rust"
            })
        );
    }

    #[test]
    fn setting_metadata_serializes_to_frontend_shape() {
        assert_eq!(
            serde_json::to_value(SearchMetadata::Setting {
                setting_id: "wifi".to_owned(),
                panel: "wifi".to_owned(),
                command: "gnome-control-center wifi".to_owned(),
            })
            .expect("setting metadata should serialize"),
            json!({
                "kind": "setting",
                "setting_id": "wifi",
                "panel": "wifi",
                "command": "gnome-control-center wifi"
            })
        );
    }

    #[test]
    fn clipboard_metadata_serializes_to_frontend_shape() {
        assert_eq!(
            serde_json::to_value(SearchMetadata::Clipboard {
                item_id: "clip:abc123".to_owned(),
                preview: "Copied text".to_owned(),
                copied_at_ms: 1_700_000_000_123,
                last_used_ms: Some(1_700_000_000_456),
                use_count: 2,
                text_len: 11,
            })
            .expect("clipboard metadata should serialize"),
            json!({
                "kind": "clipboard",
                "item_id": "clip:abc123",
                "preview": "Copied text",
                "copied_at_ms": 1_700_000_000_123_u64,
                "last_used_ms": 1_700_000_000_456_u64,
                "use_count": 2,
                "text_len": 11
            })
        );
    }

    #[test]
    fn history_metadata_serializes_to_frontend_shape() {
        assert_eq!(
            serde_json::to_value(SearchMetadata::History {
                query: "wifi".to_owned(),
                last_used_ms: 1_700_000_000_123,
                use_count: 3,
            })
            .expect("history metadata should serialize"),
            json!({
                "kind": "history",
                "query": "wifi",
                "last_used_ms": 1_700_000_000_123_u64,
                "use_count": 3
            })
        );
    }
}
