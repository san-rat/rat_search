#![allow(dead_code)]

use std::{
    env,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::settings;

const GENERATED_OR_HEAVY_DIRECTORY_NAMES: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "build",
    "dist",
    ".cache",
    "tmp",
    "temp",
];

pub(crate) fn default_index_roots() -> Vec<PathBuf> {
    let Ok(home) = env::var("HOME") else {
        return Vec::new();
    };

    if home.trim().is_empty() {
        return Vec::new();
    }

    default_index_roots_from_home(Path::new(&home))
}

pub(crate) fn default_index_roots_from_home(home: &Path) -> Vec<PathBuf> {
    if home.as_os_str().is_empty() {
        return Vec::new();
    }

    settings::DEFAULT_INDEX_ROOT_NAMES
        .iter()
        .map(|root_name| home.join(root_name))
        .filter(|path| path.is_dir())
        .collect()
}

pub(crate) fn should_skip_directory_name(name: &str) -> bool {
    name.starts_with('.') || GENERATED_OR_HEAVY_DIRECTORY_NAMES.contains(&name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileRecord {
    pub(crate) id: String,
    pub(crate) path: PathBuf,
    pub(crate) file_name: String,
    pub(crate) parent_path: PathBuf,
    pub(crate) is_dir: bool,
    pub(crate) extension: Option<String>,
    pub(crate) modified_time: Option<SystemTime>,
    pub(crate) size: Option<u64>,
}

impl FileRecord {
    pub(crate) fn new(
        path: PathBuf,
        is_dir: bool,
        modified_time: Option<SystemTime>,
        size: Option<u64>,
    ) -> Self {
        let id_prefix = if is_dir { "folder" } else { "file" };
        let id = format!("{id_prefix}:{}", path.to_string_lossy());
        let file_name = path
            .file_name()
            .map(|file_name| file_name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parent_path = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let extension = (!is_dir)
            .then(|| {
                path.extension()
                    .map(|extension| extension.to_string_lossy().to_lowercase())
            })
            .flatten();

        Self {
            id,
            path,
            file_name,
            parent_path,
            is_dir,
            extension,
            modified_time,
            size,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FileIndex {
    records: Vec<FileRecord>,
}

impl FileIndex {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_records(records: Vec<FileRecord>) -> Self {
        Self { records }
    }

    pub(crate) fn push(&mut self, record: FileRecord) {
        self.records.push(record);
    }

    pub(crate) fn extend(&mut self, records: impl IntoIterator<Item = FileRecord>) {
        self.records.extend(records);
    }

    pub(crate) fn records(&self) -> &[FileRecord] {
        &self.records
    }

    pub(crate) fn len(&self) -> usize {
        self.records.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temporary_home(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let path =
            env::temp_dir().join(format!("rat-search-{name}-{}-{unique}", std::process::id()));

        fs::create_dir_all(&path).expect("temporary home should be created");

        path
    }

    #[test]
    fn file_record_derives_file_metadata_from_path() {
        let modified_time = SystemTime::UNIX_EPOCH;
        let record = FileRecord::new(
            PathBuf::from("/home/sanuk/Documents/Report.PDF"),
            false,
            Some(modified_time),
            Some(1234),
        );

        assert_eq!(record.id, "file:/home/sanuk/Documents/Report.PDF");
        assert_eq!(
            record.path,
            PathBuf::from("/home/sanuk/Documents/Report.PDF")
        );
        assert_eq!(record.file_name, "Report.PDF");
        assert_eq!(record.parent_path, PathBuf::from("/home/sanuk/Documents"));
        assert!(!record.is_dir);
        assert_eq!(record.extension.as_deref(), Some("pdf"));
        assert_eq!(record.modified_time, Some(modified_time));
        assert_eq!(record.size, Some(1234));
    }

    #[test]
    fn folder_record_derives_folder_id_and_has_no_extension() {
        let record = FileRecord::new(
            PathBuf::from("/home/sanuk/Documents/Projects"),
            true,
            None,
            None,
        );

        assert_eq!(record.id, "folder:/home/sanuk/Documents/Projects");
        assert_eq!(record.file_name, "Projects");
        assert_eq!(record.parent_path, PathBuf::from("/home/sanuk/Documents"));
        assert!(record.is_dir);
        assert_eq!(record.extension, None);
    }

    #[test]
    fn extensionless_file_has_no_extension() {
        let record = FileRecord::new(PathBuf::from("/home/sanuk/README"), false, None, Some(12));

        assert_eq!(record.id, "file:/home/sanuk/README");
        assert_eq!(record.file_name, "README");
        assert_eq!(record.extension, None);
    }

    #[test]
    fn index_preserves_insertion_order_and_exposes_records() {
        let first = FileRecord::new(PathBuf::from("/home/sanuk/first.txt"), false, None, None);
        let second = FileRecord::new(PathBuf::from("/home/sanuk/second.txt"), false, None, None);
        let third = FileRecord::new(PathBuf::from("/home/sanuk/third"), true, None, None);

        let mut index = FileIndex::new();
        assert!(index.is_empty());

        index.push(first.clone());
        index.extend([second.clone(), third.clone()]);

        assert_eq!(index.len(), 3);
        assert_eq!(index.records(), &[first, second, third]);
    }

    #[test]
    fn index_can_be_created_from_records() {
        let record = FileRecord::new(PathBuf::from("/home/sanuk/file.txt"), false, None, None);
        let index = FileIndex::from_records(vec![record.clone()]);

        assert_eq!(index.records(), &[record]);
    }

    #[test]
    fn default_roots_include_only_existing_directories_in_stable_order() {
        let home = temporary_home("ordered-roots");
        fs::create_dir(home.join("Pictures")).expect("Pictures should be created");
        fs::create_dir(home.join("Desktop")).expect("Desktop should be created");
        fs::create_dir(home.join("Documents")).expect("Documents should be created");
        fs::write(home.join("Downloads"), "").expect("Downloads file should be created");

        let roots = default_index_roots_from_home(&home);

        assert_eq!(
            roots,
            [
                home.join("Desktop"),
                home.join("Documents"),
                home.join("Pictures")
            ]
        );

        fs::remove_dir_all(home).expect("temporary home should be removed");
    }

    #[test]
    fn missing_default_roots_are_skipped_quietly() {
        let home = temporary_home("missing-roots");

        let roots = default_index_roots_from_home(&home);

        assert!(roots.is_empty());

        fs::remove_dir_all(home).expect("temporary home should be removed");
    }

    #[test]
    fn empty_home_path_returns_no_roots() {
        assert!(default_index_roots_from_home(Path::new("")).is_empty());
    }

    #[test]
    fn directory_name_exclusion_identifies_hidden_and_heavy_directories() {
        for name in [
            ".hidden",
            ".git",
            ".cache",
            "node_modules",
            "target",
            "build",
            "dist",
            "tmp",
            "temp",
        ] {
            assert!(should_skip_directory_name(name), "{name} should be skipped");
        }

        for name in ["Desktop", "Documents", "src", "photos"] {
            assert!(
                !should_skip_directory_name(name),
                "{name} should not be skipped"
            );
        }
    }
}
