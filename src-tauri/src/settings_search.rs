#![allow(dead_code)]

use crate::{
    search_result::{SearchAction, SearchMetadata, SearchResult, SearchSource},
    settings,
};

const EXACT_SCORE: i32 = 880;
const TITLE_PREFIX_SCORE: i32 = 820;
const TITLE_CONTAINS_SCORE: i32 = 620;
const KEYWORD_CONTAINS_SCORE: i32 = 580;
const ABBREVIATION_SCORE: i32 = 420;
const SUBSEQUENCE_SCORE: i32 = 260;
const SETTINGS_SUBTITLE: &str = "System Settings";
const SETTINGS_PROGRAM: &str = "gnome-control-center";

pub(crate) fn search_settings(query: &str, limit: usize) -> Vec<SearchResult> {
    let query = normalize(query);
    let limit = settings::normalize_result_limit(limit);

    if query.is_empty() {
        return Vec::new();
    }

    let mut matches = SETTINGS
        .iter()
        .filter_map(|setting| {
            let score = score_setting(setting, &query);
            (score > 0).then_some((setting, score))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|(left, left_score), (right, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| normalize(left.title).cmp(&normalize(right.title)))
            .then_with(|| left.id.cmp(right.id))
    });

    matches
        .into_iter()
        .take(limit)
        .map(|(setting, score)| result_from_setting(setting, score))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedSettingCommand {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

pub(crate) fn command_for_setting(setting_id: &str) -> Option<PreparedSettingCommand> {
    let setting = SETTINGS.iter().find(|setting| setting.id == setting_id)?;

    Some(PreparedSettingCommand {
        program: SETTINGS_PROGRAM.to_owned(),
        args: vec![setting.panel.to_owned()],
    })
}

fn result_from_setting(setting: &SettingRecord, score: i32) -> SearchResult {
    SearchResult {
        id: format!("setting:{}", setting.id),
        title: setting.title.to_owned(),
        subtitle: Some(SETTINGS_SUBTITLE.to_owned()),
        icon: Some(setting.icon.to_owned()),
        source: SearchSource::Settings,
        action: SearchAction::OpenSetting,
        path: None,
        score,
        metadata: Some(SearchMetadata::Setting {
            setting_id: setting.id.to_owned(),
            panel: setting.panel.to_owned(),
            command: command_string(setting),
        }),
    }
}

fn command_string(setting: &SettingRecord) -> String {
    format!("{SETTINGS_PROGRAM} {}", setting.panel)
}

fn score_setting(setting: &SettingRecord, query: &str) -> i32 {
    let title = normalize(setting.title);
    let id = normalize(setting.id);
    let keywords = setting.keywords.iter().map(|keyword| normalize(keyword));

    if title == query || id == query || keywords.clone().any(|keyword| keyword == query) {
        return EXACT_SCORE;
    }

    if title.starts_with(query) {
        return TITLE_PREFIX_SCORE;
    }

    if title.contains(query) {
        return TITLE_CONTAINS_SCORE;
    }

    if keywords.clone().any(|keyword| keyword.contains(query)) {
        return KEYWORD_CONTAINS_SCORE;
    }

    if abbreviation_matches(&title, query) {
        return ABBREVIATION_SCORE;
    }

    if subsequence_matches(&title, query) || subsequence_matches(&id, query) {
        return SUBSEQUENCE_SCORE;
    }

    0
}

fn abbreviation_matches(value: &str, query: &str) -> bool {
    if query.is_empty() {
        return false;
    }

    let initials = value
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .collect::<String>();

    initials == query || initials.starts_with(query)
}

fn subsequence_matches(value: &str, query: &str) -> bool {
    if query.is_empty() {
        return false;
    }

    let mut query_chars = query.chars();
    let mut current = query_chars.next();

    for ch in value.chars() {
        if Some(ch) == current {
            current = query_chars.next();

            if current.is_none() {
                return true;
            }
        }
    }

    false
}

fn normalize(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SettingRecord {
    id: &'static str,
    title: &'static str,
    keywords: &'static [&'static str],
    panel: &'static str,
    icon: &'static str,
}

const SETTINGS: &[SettingRecord] = &[
    SettingRecord {
        id: "wifi",
        title: "Wi-Fi",
        keywords: &["wireless", "internet", "network"],
        panel: "wifi",
        icon: "settings",
    },
    SettingRecord {
        id: "bluetooth",
        title: "Bluetooth",
        keywords: &["devices", "pairing"],
        panel: "bluetooth",
        icon: "settings",
    },
    SettingRecord {
        id: "display",
        title: "Displays",
        keywords: &["monitor", "screen", "resolution"],
        panel: "display",
        icon: "settings",
    },
    SettingRecord {
        id: "keyboard",
        title: "Keyboard",
        keywords: &["typing", "shortcuts", "input"],
        panel: "keyboard",
        icon: "settings",
    },
    SettingRecord {
        id: "sound",
        title: "Sound",
        keywords: &["audio", "volume", "microphone"],
        panel: "sound",
        icon: "settings",
    },
    SettingRecord {
        id: "privacy",
        title: "Privacy",
        keywords: &["permissions", "location", "camera"],
        panel: "privacy",
        icon: "settings",
    },
    SettingRecord {
        id: "appearance",
        title: "Appearance",
        keywords: &["theme", "wallpaper", "background"],
        panel: "appearance",
        icon: "settings",
    },
    SettingRecord {
        id: "power",
        title: "Power",
        keywords: &["battery", "suspend", "sleep"],
        panel: "power",
        icon: "settings",
    },
    SettingRecord {
        id: "mouse",
        title: "Mouse and Touchpad",
        keywords: &["touchpad", "pointer", "click"],
        panel: "mouse",
        icon: "settings",
    },
    SettingRecord {
        id: "network",
        title: "Network",
        keywords: &["ethernet", "vpn", "internet"],
        panel: "network",
        icon: "settings",
    },
    SettingRecord {
        id: "printers",
        title: "Printers",
        keywords: &["print", "scanner"],
        panel: "printers",
        icon: "settings",
    },
    SettingRecord {
        id: "about",
        title: "About",
        keywords: &["system", "device", "version"],
        panel: "info-overview",
        icon: "settings",
    },
];

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn setting_results(query: &str) -> Vec<SearchResult> {
        search_settings(query, 8)
    }

    fn first_setting(query: &str) -> SearchResult {
        setting_results(query)
            .into_iter()
            .next()
            .expect("setting result")
    }

    #[test]
    fn exact_wifi_returns_wifi_first() {
        let result = first_setting("wifi");

        assert_eq!(result.title, "Wi-Fi");
        assert_eq!(result.score, EXACT_SCORE);
    }

    #[test]
    fn keyboard_returns_keyboard() {
        let result = first_setting("keyboard");

        assert_eq!(result.id, "setting:keyboard");
        assert_eq!(result.title, "Keyboard");
    }

    #[test]
    fn common_aliases_match_expected_settings() {
        for (query, expected_title) in [
            ("wireless", "Wi-Fi"),
            ("monitor", "Displays"),
            ("audio", "Sound"),
            ("battery", "Power"),
            ("wallpaper", "Appearance"),
            ("vpn", "Network"),
            ("system", "About"),
        ] {
            assert_eq!(first_setting(query).title, expected_title);
        }
    }

    #[test]
    fn unknown_terms_return_no_settings() {
        assert!(setting_results("definitely-not-a-panel").is_empty());
    }

    #[test]
    fn ranking_prefers_stronger_matches() {
        let results = setting_results("power");

        assert_eq!(results[0].title, "Power");
        assert_eq!(results[0].score, EXACT_SCORE);

        let display_results = setting_results("disp");

        assert_eq!(display_results[0].title, "Displays");
        assert_eq!(display_results[0].score, TITLE_PREFIX_SCORE);
    }

    #[test]
    fn zero_limit_uses_default_limit() {
        assert_eq!(search_settings("wifi", 0)[0].title, "Wi-Fi");
    }

    #[test]
    fn command_for_setting_resolves_only_known_ids() {
        assert_eq!(
            command_for_setting("wifi"),
            Some(PreparedSettingCommand {
                program: "gnome-control-center".to_owned(),
                args: vec!["wifi".to_owned()],
            })
        );
        assert_eq!(
            command_for_setting("about"),
            Some(PreparedSettingCommand {
                program: "gnome-control-center".to_owned(),
                args: vec!["info-overview".to_owned()],
            })
        );
        assert_eq!(command_for_setting("unknown"), None);
    }

    #[test]
    fn result_uses_expected_frontend_shape() {
        let result = first_setting("wifi");

        assert_eq!(
            serde_json::to_value(result).expect("result should serialize"),
            json!({
                "id": "setting:wifi",
                "title": "Wi-Fi",
                "subtitle": "System Settings",
                "icon": "settings",
                "source": "settings",
                "action": "open_setting",
                "path": null,
                "score": 880,
                "metadata": {
                    "kind": "setting",
                    "setting_id": "wifi",
                    "panel": "wifi",
                    "command": "gnome-control-center wifi"
                }
            })
        );
    }
}
