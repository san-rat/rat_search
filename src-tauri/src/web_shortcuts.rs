#![allow(dead_code)]

use crate::{
    search_result::{SearchAction, SearchMetadata, SearchResult, SearchSource},
    settings,
};

const WEB_SHORTCUT_SCORE: i32 = 700;

pub(crate) fn search_web_shortcuts(query: &str, limit: usize) -> Vec<SearchResult> {
    let limit = settings::normalize_result_limit(limit);

    if limit == 0 {
        return Vec::new();
    }

    let Some((shortcut, search_query)) = parse_shortcut_query(query) else {
        return Vec::new();
    };
    let Some(web_shortcut) = shortcut_for(shortcut) else {
        return Vec::new();
    };

    let encoded_query = percent_encode(search_query);
    let url = web_shortcut.url_for(&encoded_query);

    vec![SearchResult {
        id: format!("web:{shortcut}:{encoded_query}"),
        title: format!("Search {}", web_shortcut.label),
        subtitle: Some(search_query.to_owned()),
        icon: Some("web".to_owned()),
        source: SearchSource::Web,
        action: SearchAction::OpenUrl,
        path: None,
        score: WEB_SHORTCUT_SCORE,
        metadata: Some(SearchMetadata::Web {
            shortcut: shortcut.to_owned(),
            query: search_query.to_owned(),
            url,
        }),
    }]
}

pub(crate) fn is_allowed_url(url: &str) -> bool {
    if url.as_bytes().iter().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }

    supported_url_prefixes().iter().any(|prefix| {
        let Some(encoded_query) = url.strip_prefix(prefix) else {
            return false;
        };

        !encoded_query.is_empty() && is_valid_generated_query(encoded_query)
    })
}

fn parse_shortcut_query(query: &str) -> Option<(&str, &str)> {
    let query = query.trim();
    let separator_index = query.find(char::is_whitespace)?;
    let shortcut = &query[..separator_index];
    let search_query = query[separator_index..].trim();

    (!search_query.is_empty()).then_some((shortcut, search_query))
}

fn shortcut_for(shortcut: &str) -> Option<WebShortcut> {
    match shortcut {
        "?" => Some(WebShortcut::new("Google", WebUrlTemplate::Google)),
        "g" => Some(WebShortcut::new("Google", WebUrlTemplate::Google)),
        "w" => Some(WebShortcut::new("Wikipedia", WebUrlTemplate::Wikipedia)),
        "yt" => Some(WebShortcut::new("YouTube", WebUrlTemplate::YouTube)),
        "gh" => Some(WebShortcut::new("GitHub", WebUrlTemplate::GitHub)),
        "maps" => Some(WebShortcut::new("Google Maps", WebUrlTemplate::Maps)),
        _ => None,
    }
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.as_bytes() {
        match byte {
            b' ' => encoded.push_str("%20"),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

fn supported_url_prefixes() -> [&'static str; 5] {
    [
        WebUrlTemplate::Google.url_prefix(),
        WebUrlTemplate::Wikipedia.url_prefix(),
        WebUrlTemplate::YouTube.url_prefix(),
        WebUrlTemplate::GitHub.url_prefix(),
        WebUrlTemplate::Maps.url_prefix(),
    ]
}

fn is_valid_generated_query(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                index += 1;
            }
            b'%' if index + 2 < bytes.len()
                && is_upper_hex_digit(bytes[index + 1])
                && is_upper_hex_digit(bytes[index + 2]) =>
            {
                index += 3;
            }
            _ => return false,
        }
    }

    true
}

fn is_upper_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'A'..=b'F')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WebShortcut {
    label: &'static str,
    template: WebUrlTemplate,
}

impl WebShortcut {
    fn new(label: &'static str, template: WebUrlTemplate) -> Self {
        Self { label, template }
    }

    fn url_for(&self, encoded_query: &str) -> String {
        format!("{}{encoded_query}", self.template.url_prefix())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebUrlTemplate {
    Google,
    Wikipedia,
    YouTube,
    GitHub,
    Maps,
}

impl WebUrlTemplate {
    fn url_prefix(&self) -> &'static str {
        match self {
            WebUrlTemplate::Google => "https://www.google.com/search?q=",
            WebUrlTemplate::Wikipedia => "https://en.wikipedia.org/wiki/Special:Search?search=",
            WebUrlTemplate::YouTube => "https://www.youtube.com/results?search_query=",
            WebUrlTemplate::GitHub => "https://github.com/search?q=",
            WebUrlTemplate::Maps => "https://www.google.com/maps/search/",
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn web_result(query: &str) -> Option<SearchResult> {
        search_web_shortcuts(query, 8).into_iter().next()
    }

    fn metadata_url(result: &SearchResult) -> &str {
        match result.metadata.as_ref().expect("metadata") {
            SearchMetadata::Web { url, .. } => url,
            metadata => panic!("expected web metadata, got {metadata:?}"),
        }
    }

    #[test]
    fn supported_prefixes_return_expected_https_urls() {
        let cases = [
            (
                "? rust tauri",
                "https://www.google.com/search?q=rust%20tauri",
            ),
            (
                "g rust tauri",
                "https://www.google.com/search?q=rust%20tauri",
            ),
            (
                "w rust language",
                "https://en.wikipedia.org/wiki/Special:Search?search=rust%20language",
            ),
            (
                "yt lofi beats",
                "https://www.youtube.com/results?search_query=lofi%20beats",
            ),
            ("gh tauri apps", "https://github.com/search?q=tauri%20apps"),
            ("maps colombo", "https://www.google.com/maps/search/colombo"),
        ];

        for (query, expected_url) in cases {
            let result = web_result(query).expect("web result");

            assert_eq!(metadata_url(&result), expected_url);
            assert!(expected_url.starts_with("https://"));
        }
    }

    #[test]
    fn question_mark_and_g_prefixes_both_search_google() {
        assert_eq!(
            metadata_url(&web_result("? rust").expect("web result")),
            "https://www.google.com/search?q=rust"
        );
        assert_eq!(
            metadata_url(&web_result("g rust").expect("web result")),
            "https://www.google.com/search?q=rust"
        );
    }

    #[test]
    fn percent_encoding_handles_spaces_symbols_and_non_ascii() {
        let result = web_result("g café & rust").expect("web result");

        assert_eq!(
            metadata_url(&result),
            "https://www.google.com/search?q=caf%C3%A9%20%26%20rust"
        );
    }

    #[test]
    fn generated_web_urls_are_allowed() {
        for query in [
            "? rust tauri",
            "w rust language",
            "yt lofi beats",
            "gh tauri apps",
            "maps café & rust",
        ] {
            let result = web_result(query).expect("web result");

            assert!(
                is_allowed_url(metadata_url(&result)),
                "{query} should be allowed"
            );
        }
    }

    #[test]
    fn unsupported_web_urls_are_rejected() {
        for url in [
            "http://www.google.com/search?q=rust",
            "https://example.com/search?q=rust",
            "https://www.google.com/search?q=",
            "https://www.google.com/search?q=rust&source=rat",
            "https://www.google.com/search?q=rust tauri",
            "https://github.com/search?q=rust%2ftauri",
            "https://github.com/search?q=rust%",
            "https://github.com/search?q=rust%2",
            "https://github.com/search?q=rust%XZ",
        ] {
            assert!(!is_allowed_url(url), "{url} should be rejected");
        }
    }

    #[test]
    fn unsupported_prefixes_return_no_result() {
        for query in ["x rust", "google rust", "github rust", "m rust"] {
            assert!(web_result(query).is_none(), "{query} should be rejected");
        }
    }

    #[test]
    fn prefixes_without_query_return_no_result() {
        for query in ["?", "g", "w ", "yt    ", "gh", "maps"] {
            assert!(web_result(query).is_none(), "{query} should be rejected");
        }
    }

    #[test]
    fn ordinary_queries_do_not_produce_web_results() {
        for query in ["firefox", "report pdf", "rust", "maps"] {
            assert!(web_result(query).is_none(), "{query} should not match");
        }
    }

    #[test]
    fn zero_limit_uses_default_limit() {
        assert_eq!(
            search_web_shortcuts("g rust", 0)
                .first()
                .expect("web result")
                .title,
            "Search Google"
        );
    }

    #[test]
    fn result_uses_expected_frontend_shape() {
        let result = web_result(" maps colombo fort ").expect("web result");

        assert_eq!(
            serde_json::to_value(result).expect("result should serialize"),
            json!({
                "id": "web:maps:colombo%20fort",
                "title": "Search Google Maps",
                "subtitle": "colombo fort",
                "icon": "web",
                "source": "web",
                "action": "open_url",
                "path": null,
                "score": 700,
                "metadata": {
                    "kind": "web",
                    "shortcut": "maps",
                    "query": "colombo fort",
                    "url": "https://www.google.com/maps/search/colombo%20fort"
                }
            })
        );
    }
}
