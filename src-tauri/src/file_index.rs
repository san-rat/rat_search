#![allow(dead_code)]

use std::{
    env, fs,
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

pub(crate) fn scan_default_roots() -> FileIndex {
    scan_roots(&default_index_roots())
}

pub(crate) fn scan_roots(roots: &[PathBuf]) -> FileIndex {
    let mut records = Vec::new();

    for root in roots {
        scan_path(root, &mut records);
    }

    FileIndex::from_records(records)
}

fn scan_path(path: &Path, records: &mut Vec<FileRecord>) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };

    if metadata.file_type().is_symlink() {
        return;
    }

    let is_dir = metadata.is_dir();
    let size = (!is_dir).then_some(metadata.len());
    records.push(FileRecord::new(
        path.to_path_buf(),
        is_dir,
        metadata.modified().ok(),
        size,
    ));

    if !is_dir {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut child_paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    child_paths.sort();

    for child_path in child_paths {
        if should_skip_directory(&child_path) {
            continue;
        }

        scan_path(&child_path, records);
    }
}

fn should_skip_directory(path: &Path) -> bool {
    if !fs::symlink_metadata(path)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
    {
        return false;
    }

    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(should_skip_directory_name)
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

    pub(crate) fn replace_records(&mut self, records: Vec<FileRecord>) {
        self.records = records;
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
    fn index_records_can_be_replaced() {
        let original =
            FileRecord::new(PathBuf::from("/home/sanuk/original.txt"), false, None, None);
        let replacement = FileRecord::new(
            PathBuf::from("/home/sanuk/replacement.txt"),
            false,
            None,
            None,
        );
        let mut index = FileIndex::from_records(vec![original]);

        index.replace_records(vec![replacement.clone()]);

        assert_eq!(index.records(), &[replacement]);
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

    #[test]
    fn scan_includes_roots_nested_folders_and_files() {
        let home = temporary_home("scan-contents");
        let root = home.join("Documents");
        let nested = root.join("Projects");
        let file = nested.join("Notes.TXT");
        fs::create_dir_all(&nested).expect("nested directory should be created");
        fs::write(&file, "hello").expect("file should be created");

        let index = scan_roots(std::slice::from_ref(&root));
        let paths = index
            .records()
            .iter()
            .map(|record| record.path.clone())
            .collect::<Vec<_>>();

        assert_eq!(paths, [root.clone(), nested.clone(), file.clone()]);

        fs::remove_dir_all(home).expect("temporary home should be removed");
    }

    #[test]
    fn scan_skips_missing_roots_quietly() {
        let home = temporary_home("scan-missing");
        let missing = home.join("Documents");

        let index = scan_roots(&[missing]);

        assert!(index.is_empty());

        fs::remove_dir_all(home).expect("temporary home should be removed");
    }

    #[test]
    fn scan_skips_hidden_and_heavy_directories_but_keeps_similarly_named_files() {
        let home = temporary_home("scan-skips");
        let root = home.join("Documents");
        fs::create_dir_all(root.join(".hidden")).expect("hidden directory should be created");
        fs::create_dir_all(root.join("node_modules")).expect("heavy directory should be created");
        fs::write(root.join(".hidden/secret.txt"), "secret")
            .expect("hidden file should be created");
        fs::write(root.join("node_modules/package.json"), "{}")
            .expect("package file should be created");
        fs::write(root.join("build"), "not a directory").expect("build file should be created");

        let index = scan_roots(std::slice::from_ref(&root));
        let names = index
            .records()
            .iter()
            .map(|record| record.file_name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, ["Documents", "build"]);

        fs::remove_dir_all(home).expect("temporary home should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn scan_skips_symlinks() {
        use std::os::unix::fs::symlink;

        let home = temporary_home("scan-symlink");
        let root = home.join("Documents");
        let target = root.join("target.txt");
        let link = root.join("target-link.txt");
        fs::create_dir_all(&root).expect("root directory should be created");
        fs::write(&target, "target").expect("target file should be created");
        symlink(&target, &link).expect("symlink should be created");

        let index = scan_roots(std::slice::from_ref(&root));
        let paths = index
            .records()
            .iter()
            .map(|record| record.path.clone())
            .collect::<Vec<_>>();

        assert_eq!(paths, [root.clone(), target.clone()]);

        fs::remove_dir_all(home).expect("temporary home should be removed");
    }

    #[test]
    fn scan_populates_file_and_folder_metadata() {
        let home = temporary_home("scan-metadata");
        let root = home.join("Documents");
        let file = root.join("Report.PDF");
        fs::create_dir_all(&root).expect("root directory should be created");
        fs::write(&file, "report").expect("file should be created");

        let index = scan_roots(std::slice::from_ref(&root));
        let root_record = index
            .records()
            .iter()
            .find(|record| record.path == root)
            .expect("root record should exist");
        let file_record = index
            .records()
            .iter()
            .find(|record| record.path == file)
            .expect("file record should exist");

        assert!(root_record.is_dir);
        assert_eq!(root_record.size, None);
        assert!(root_record.modified_time.is_some());
        assert!(!file_record.is_dir);
        assert_eq!(file_record.size, Some(6));
        assert_eq!(file_record.extension.as_deref(), Some("pdf"));
        assert!(file_record.modified_time.is_some());

        fs::remove_dir_all(home).expect("temporary home should be removed");
    }

    #[test]
    fn scan_order_is_deterministic_by_path() {
        let home = temporary_home("scan-order");
        let root = home.join("Documents");
        fs::create_dir_all(&root).expect("root directory should be created");
        fs::write(root.join("zeta.txt"), "").expect("zeta should be created");
        fs::write(root.join("alpha.txt"), "").expect("alpha should be created");
        fs::create_dir(root.join("middle")).expect("middle directory should be created");
        fs::write(root.join("middle/beta.txt"), "").expect("beta should be created");

        let index = scan_roots(std::slice::from_ref(&root));
        let names = index
            .records()
            .iter()
            .map(|record| record.file_name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            ["Documents", "alpha.txt", "middle", "beta.txt", "zeta.txt"]
        );

        fs::remove_dir_all(home).expect("temporary home should be removed");
    }
}
