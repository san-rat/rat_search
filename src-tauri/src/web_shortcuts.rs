#![allow(dead_code)]

use crate::{
    search_result::{SearchAction, SearchMetadata, SearchResult, SearchSource},
    settings,
};

const WEB_SHORTCUT_SCORE: i32 = 700;
const GOOGLE_SHORTCUT: &str = "google";
const GOOGLE_LABEL: &str = "Google";
const GOOGLE_SEARCH_URL_PREFIX: &str = "https://www.google.com/search?q=";
const QUESTION_WORDS: &[&str] = &["what", "how", "is", "will", "should", "do"];

pub(crate) fn search_web_shortcuts(query: &str, limit: usize) -> Vec<SearchResult> {
    let limit = settings::normalize_result_limit(limit);

    if limit == 0 {
        return Vec::new();
    }

    let Some(search_query) = question_search_query(query) else {
        return Vec::new();
    };

    let encoded_query = percent_encode(search_query);
    let url = google_url_for(&encoded_query);

    vec![SearchResult {
        id: format!("web:{GOOGLE_SHORTCUT}:{encoded_query}"),
        title: format!("Search {GOOGLE_LABEL}"),
        subtitle: Some(search_query.to_owned()),
        icon: Some("web".to_owned()),
        source: SearchSource::Web,
        action: SearchAction::OpenUrl,
        path: None,
        score: WEB_SHORTCUT_SCORE,
        metadata: Some(SearchMetadata::Web {
            shortcut: GOOGLE_SHORTCUT.to_owned(),
            query: search_query.to_owned(),
            url,
        }),
    }]
}

pub(crate) fn is_allowed_url(url: &str) -> bool {
    if url.as_bytes().iter().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }

    let Some(encoded_query) = url.strip_prefix(GOOGLE_SEARCH_URL_PREFIX) else {
        return false;
    };

    !encoded_query.is_empty() && is_valid_generated_query(encoded_query)
}

fn question_search_query(query: &str) -> Option<&str> {
    let query = query.trim();

    if query.is_empty() {
        return None;
    }

    (query.ends_with('?') || contains_question_word(query)).then_some(query)
}

fn contains_question_word(query: &str) -> bool {
    query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .any(|word| {
            QUESTION_WORDS
                .iter()
                .any(|question_word| word.eq_ignore_ascii_case(question_word))
        })
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

fn google_url_for(encoded_query: &str) -> String {
    format!("{GOOGLE_SEARCH_URL_PREFIX}{encoded_query}")
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
    fn question_queries_return_expected_google_urls() {
        let cases = [
            (
                "what is rust",
                "https://www.google.com/search?q=what%20is%20rust",
            ),
            (
                "How does Tauri work",
                "https://www.google.com/search?q=How%20does%20Tauri%20work",
            ),
            (
                "rust tauri?",
                "https://www.google.com/search?q=rust%20tauri%3F",
            ),
            (
                "the question is simple",
                "https://www.google.com/search?q=the%20question%20is%20simple",
            ),
        ];

        for (query, expected_url) in cases {
            let result = web_result(query).expect("web result");

            assert_eq!(metadata_url(&result), expected_url);
            assert!(expected_url.starts_with("https://"));
        }
    }

    #[test]
    fn configured_question_words_match_case_insensitively() {
        for query in [
            "WHAT is rust",
            "how does this work",
            "is this open",
            "will it launch",
            "should I test this",
            "do files open",
        ] {
            assert!(web_result(query).is_some(), "{query} should match");
        }
    }

    #[test]
    fn percent_encoding_handles_spaces_symbols_and_non_ascii() {
        let result = web_result("what is café & rust?").expect("web result");

        assert_eq!(
            metadata_url(&result),
            "https://www.google.com/search?q=what%20is%20caf%C3%A9%20%26%20rust%3F"
        );
    }

    #[test]
    fn generated_web_urls_are_allowed() {
        for query in ["what is rust", "How does Tauri work", "maps café & rust?"] {
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
            "https://www.google.com/search?q=rust%",
            "https://www.google.com/search?q=rust%2",
            "https://www.google.com/search?q=rust%XZ",
            "https://www.google.com/search?q=rust%2ftauri",
            "https://en.wikipedia.org/wiki/Special:Search?search=rust",
            "https://www.youtube.com/results?search_query=lofi",
            "https://www.google.com/maps/search/colombo",
            "https://github.com/search?q=rust%2ftauri",
        ] {
            assert!(!is_allowed_url(url), "{url} should be rejected");
        }
    }

    #[test]
    fn old_web_prefixes_return_no_result() {
        for query in [
            "g rust",
            "? rust",
            "w rust",
            "yt lofi",
            "gh tauri",
            "maps colombo",
        ] {
            assert!(web_result(query).is_none(), "{query} should be rejected");
        }
    }

    #[test]
    fn empty_or_non_question_queries_return_no_result() {
        for query in ["", "   ", "firefox", "report pdf", "rust", "maps"] {
            assert!(web_result(query).is_none(), "{query} should be rejected");
        }
    }

    #[test]
    fn question_words_require_word_boundaries() {
        for query in [
            "history notes",
            "this report",
            "doing tasks",
            "show files",
            "whatnot",
            "island maps",
        ] {
            assert!(web_result(query).is_none(), "{query} should not match");
        }
    }

    #[test]
    fn zero_limit_uses_default_limit() {
        assert_eq!(
            search_web_shortcuts("what is rust", 0)
                .first()
                .expect("web result")
                .title,
            "Search Google"
        );
    }

    #[test]
    fn result_uses_expected_frontend_shape() {
        let result = web_result(" what is colombo fort? ").expect("web result");

        assert_eq!(
            serde_json::to_value(result).expect("result should serialize"),
            json!({
                "id": "web:google:what%20is%20colombo%20fort%3F",
                "title": "Search Google",
                "subtitle": "what is colombo fort?",
                "icon": "web",
                "source": "web",
                "action": "open_url",
                "path": null,
                "score": 700,
                "metadata": {
                    "kind": "web",
                    "shortcut": "google",
                    "query": "what is colombo fort?",
                    "url": "https://www.google.com/search?q=what%20is%20colombo%20fort%3F"
                }
            })
        );
    }
}
