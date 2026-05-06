use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppRecord {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) generic_name: Option<String>,
    pub(crate) comment: Option<String>,
    pub(crate) exec: String,
    pub(crate) icon: Option<String>,
    pub(crate) categories: Vec<String>,
    pub(crate) keywords: Vec<String>,
    pub(crate) desktop_file_path: String,
    pub(crate) terminal: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(crate) struct AppCatalog {
    pub(crate) apps: Vec<AppRecord>,
}

impl AppCatalog {
    pub(crate) fn scan() -> Self {
        let mut records_by_id = HashMap::new();

        for directory in application_directories() {
            scan_application_directory(&directory, &mut records_by_id);
        }

        let mut apps = records_by_id.into_values().collect::<Vec<_>>();
        apps.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });

        Self { apps }
    }

    pub(crate) fn len(&self) -> usize {
        self.apps.len()
    }

    pub(crate) fn find_by_id(&self, id: &str) -> Option<&AppRecord> {
        self.apps.iter().find(|app| app.id == id)
    }
}

fn application_directories() -> Vec<PathBuf> {
    let mut directories = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];

    if let Ok(home) = env::var("HOME") {
        directories.push(PathBuf::from(home).join(".local/share/applications"));
    }

    directories
}

fn scan_application_directory(directory: &Path, records_by_id: &mut HashMap<String, AppRecord>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path
            .extension()
            .is_none_or(|extension| extension != "desktop")
        {
            continue;
        }

        let Some(record) = parse_desktop_file(&path) else {
            continue;
        };

        records_by_id.insert(record.id.clone(), record);
    }
}

fn parse_desktop_file(path: &Path) -> Option<AppRecord> {
    let contents = fs::read_to_string(path).ok()?;
    parse_desktop_entry(&contents, path)
}

fn parse_desktop_entry(contents: &str, path: &Path) -> Option<AppRecord> {
    let mut in_desktop_entry = false;
    let mut fields = HashMap::<String, String>::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = &line[1..line.len() - 1] == "Desktop Entry";
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };

        let key = raw_key.trim();
        if key.is_empty() || key.contains('[') {
            continue;
        }

        fields.insert(key.to_owned(), raw_value.trim().to_owned());
    }

    if fields
        .get("Type")
        .is_some_and(|entry_type| !entry_type.eq_ignore_ascii_case("Application"))
    {
        return None;
    }

    if parse_bool(fields.get("NoDisplay")) || parse_bool(fields.get("Hidden")) {
        return None;
    }

    let name = non_empty_field(&fields, "Name")?;
    let exec = non_empty_field(&fields, "Exec")?;

    let id = path.file_name()?.to_string_lossy().into_owned();

    Some(AppRecord {
        id,
        name,
        generic_name: optional_field(&fields, "GenericName"),
        comment: optional_field(&fields, "Comment"),
        exec,
        icon: optional_field(&fields, "Icon"),
        categories: parse_list(fields.get("Categories")),
        keywords: parse_list(fields.get("Keywords")),
        desktop_file_path: path.to_string_lossy().into_owned(),
        terminal: parse_bool(fields.get("Terminal")),
    })
}

fn non_empty_field(fields: &HashMap<String, String>, key: &str) -> Option<String> {
    optional_field(fields, key)
}

fn optional_field(fields: &HashMap<String, String>, key: &str) -> Option<String> {
    fields
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_bool(value: Option<&String>) -> bool {
    value.is_some_and(|value| value.trim().eq_ignore_ascii_case("true"))
}

fn parse_list(value: Option<&String>) -> Vec<String> {
    value
        .map(|value| {
            value
                .split(';')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(contents: &str) -> Option<AppRecord> {
        parse_desktop_entry(contents, Path::new("/tmp/example.desktop"))
    }

    #[test]
    fn parses_valid_desktop_entry() {
        let record = parse(
            r#"
            [Desktop Entry]
            Type=Application
            Name=Example App
            GenericName=Example Generic
            Comment=Example Comment
            Exec=example --open
            Icon=example-icon
            Categories=Utility;Development;
            Keywords=Search;Launch;
            Terminal=true
            "#,
        )
        .expect("valid app should parse");

        assert_eq!(record.id, "example.desktop");
        assert_eq!(record.name, "Example App");
        assert_eq!(record.generic_name.as_deref(), Some("Example Generic"));
        assert_eq!(record.comment.as_deref(), Some("Example Comment"));
        assert_eq!(record.exec, "example --open");
        assert_eq!(record.icon.as_deref(), Some("example-icon"));
        assert_eq!(record.categories, ["Utility", "Development"]);
        assert_eq!(record.keywords, ["Search", "Launch"]);
        assert!(record.terminal);
    }

    #[test]
    fn ignores_no_display_entries() {
        let record = parse(
            r#"
            [Desktop Entry]
            Type=Application
            Name=Hidden App
            Exec=hidden
            NoDisplay=true
            "#,
        );

        assert!(record.is_none());
    }

    #[test]
    fn ignores_hidden_entries() {
        let record = parse(
            r#"
            [Desktop Entry]
            Type=Application
            Name=Hidden App
            Exec=hidden
            Hidden=true
            "#,
        );

        assert!(record.is_none());
    }

    #[test]
    fn ignores_entries_missing_name() {
        let record = parse(
            r#"
            [Desktop Entry]
            Type=Application
            Exec=nameless
            "#,
        );

        assert!(record.is_none());
    }

    #[test]
    fn ignores_entries_missing_exec() {
        let record = parse(
            r#"
            [Desktop Entry]
            Type=Application
            Name=No Exec
            "#,
        );

        assert!(record.is_none());
    }

    #[test]
    fn splits_categories_and_keywords_without_empty_items() {
        let record = parse(
            r#"
            [Desktop Entry]
            Type=Application
            Name=Lists
            Exec=lists
            Categories=Utility;;Office;
            Keywords=one;two;;three;
            "#,
        )
        .expect("valid app should parse");

        assert_eq!(record.categories, ["Utility", "Office"]);
        assert_eq!(record.keywords, ["one", "two", "three"]);
    }

    #[test]
    fn ignores_localized_keys() {
        let record = parse(
            r#"
            [Desktop Entry]
            Type=Application
            Name[en_US]=Localized Name
            Name=Base Name
            Exec=base
            "#,
        )
        .expect("valid app should parse");

        assert_eq!(record.name, "Base Name");
    }
}
