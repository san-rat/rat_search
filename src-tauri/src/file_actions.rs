use std::path::{Path, PathBuf};

use crate::file_index::FileIndex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedPath {
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
}

pub(crate) fn validate_indexed_path(
    index: &FileIndex,
    requested_path: &str,
) -> Result<ValidatedPath, String> {
    if requested_path.trim().is_empty() {
        return Err("Path is required".to_owned());
    }

    let requested_path = PathBuf::from(requested_path);
    let Some(record) = index
        .records()
        .iter()
        .find(|record| record.path == requested_path)
    else {
        return Err("Path is not indexed".to_owned());
    };

    match record.path.try_exists() {
        Ok(true) => {}
        Ok(false) => return Err("Path no longer exists".to_owned()),
        Err(_) => return Err("Could not access item".to_owned()),
    }

    Ok(ValidatedPath {
        path: record.path.clone(),
        is_dir: record.is_dir,
    })
}

pub(crate) fn reveal_target(validated_path: &ValidatedPath) -> Result<PathBuf, String> {
    if validated_path.is_dir {
        return Ok(validated_path.path.clone());
    }

    validated_path
        .path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Could not determine parent folder".to_owned())
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use crate::file_index::FileRecord;

    use super::*;

    fn temporary_directory(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("rat-search-{name}-{}-{unique}", std::process::id()));

        fs::create_dir_all(&path).expect("temporary directory should be created");

        path
    }

    fn index_with(record_path: PathBuf, is_dir: bool) -> FileIndex {
        FileIndex::from_records(vec![FileRecord::new(record_path, is_dir, None, None)])
    }

    #[test]
    fn indexed_existing_file_is_accepted() {
        let root = temporary_directory("file-action-file");
        let path = root.join("report.txt");
        fs::write(&path, "report").expect("temporary file should be written");
        let index = index_with(path.clone(), false);

        let validated =
            validate_indexed_path(&index, &path.to_string_lossy()).expect("path should validate");

        assert_eq!(validated.path, path);
        assert!(!validated.is_dir);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn indexed_existing_folder_is_accepted() {
        let root = temporary_directory("file-action-folder");
        let folder = root.join("Projects");
        fs::create_dir(&folder).expect("temporary folder should be created");
        let index = index_with(folder.clone(), true);

        let validated =
            validate_indexed_path(&index, &folder.to_string_lossy()).expect("path should validate");

        assert_eq!(validated.path, folder);
        assert!(validated.is_dir);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn unindexed_path_is_rejected() {
        let root = temporary_directory("file-action-unindexed");
        let indexed = root.join("indexed.txt");
        let unindexed = root.join("unindexed.txt");
        fs::write(&indexed, "indexed").expect("indexed file should be written");
        fs::write(&unindexed, "unindexed").expect("unindexed file should be written");
        let index = index_with(indexed, false);

        let error = validate_indexed_path(&index, &unindexed.to_string_lossy())
            .expect_err("unindexed path should be rejected");

        assert_eq!(error, "Path is not indexed");

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn missing_indexed_path_is_rejected() {
        let root = temporary_directory("file-action-missing");
        let path = root.join("moved.txt");
        fs::write(&path, "moved").expect("temporary file should be written");
        let index = index_with(path.clone(), false);
        fs::remove_file(&path).expect("temporary file should be removed");

        let error = validate_indexed_path(&index, &path.to_string_lossy())
            .expect_err("missing path should be rejected");

        assert_eq!(error, "Path no longer exists");

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn reveal_target_uses_parent_for_files_and_self_for_folders() {
        let folder = PathBuf::from("/home/sanuk/Documents");
        let file = folder.join("report.pdf");
        let file_target = reveal_target(&ValidatedPath {
            path: file,
            is_dir: false,
        })
        .expect("file reveal target should resolve");
        let folder_target = reveal_target(&ValidatedPath {
            path: folder.clone(),
            is_dir: true,
        })
        .expect("folder reveal target should resolve");

        assert_eq!(file_target, folder);
        assert_eq!(folder_target, folder);
    }
}
