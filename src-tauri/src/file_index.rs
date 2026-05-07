#![allow(dead_code)]

use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

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
    use super::*;

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
}
