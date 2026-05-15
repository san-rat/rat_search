use std::{
    env,
    path::{Path, PathBuf},
};

use crate::file_index::FileIndex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedPath {
    pub(crate) path: PathBuf,
    pub(crate) is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedCommand {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PreferredOpen {
    Code(PreparedCommand),
    System,
}

const CODE_EXECUTABLES: &[&str] = &["code", "codium", "code-insiders"];
const CODE_EXTENSIONS: &[&str] = &[
    "txt",
    "md",
    "markdown",
    "rs",
    "js",
    "jsx",
    "ts",
    "tsx",
    "svelte",
    "html",
    "css",
    "scss",
    "json",
    "toml",
    "yaml",
    "yml",
    "xml",
    "py",
    "sh",
    "bash",
    "zsh",
    "fish",
    "go",
    "java",
    "kt",
    "c",
    "h",
    "cpp",
    "hpp",
    "cs",
    "php",
    "rb",
    "swift",
    "sql",
    "env",
    "gitignore",
    "dockerfile",
    "makefile",
];

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

pub(crate) fn prepare_open_in_code(validated_path: &ValidatedPath) -> PreferredOpen {
    if !should_open_in_code(validated_path) {
        return PreferredOpen::System;
    }

    match resolve_code_executable_from_path(&path_entries()) {
        Some(program) => PreferredOpen::Code(PreparedCommand {
            program,
            args: vec![validated_path.path.to_string_lossy().into_owned()],
        }),
        None => PreferredOpen::System,
    }
}

fn should_open_in_code(validated_path: &ValidatedPath) -> bool {
    validated_path.is_dir || is_code_like_file(&validated_path.path)
}

pub(crate) fn is_code_like_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let file_name = file_name.to_ascii_lowercase();
    let normalized_name = file_name.trim_start_matches('.');

    if CODE_EXTENSIONS
        .iter()
        .any(|extension| file_name == *extension || normalized_name == *extension)
    {
        return true;
    }

    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let extension = extension.to_ascii_lowercase();
            CODE_EXTENSIONS
                .iter()
                .any(|code_extension| extension == *code_extension)
        })
        .unwrap_or(false)
}

fn path_entries() -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect())
        .unwrap_or_default()
}

fn resolve_code_executable_from_path(path_entries: &[PathBuf]) -> Option<String> {
    CODE_EXECUTABLES
        .iter()
        .find(|executable| executable_exists(path_entries, executable))
        .map(|executable| (*executable).to_owned())
}

fn executable_exists(path_entries: &[PathBuf], executable: &str) -> bool {
    path_entries
        .iter()
        .map(|entry| entry.join(executable))
        .any(|candidate| candidate.is_file())
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

    fn temporary_executable(root: &Path, name: &str) {
        fs::write(root.join(name), "#!/bin/sh\n").expect("temporary executable should be written");
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

    #[test]
    fn code_like_file_detection_is_conservative_and_case_insensitive() {
        assert!(is_code_like_file(Path::new("/home/sanuk/work/app.SVELTE")));
        assert!(is_code_like_file(Path::new("/home/sanuk/work/Dockerfile")));
        assert!(is_code_like_file(Path::new("/home/sanuk/work/.gitignore")));
        assert!(!is_code_like_file(Path::new("/home/sanuk/work/report.pdf")));
        assert!(!is_code_like_file(Path::new("/home/sanuk/work/photo.png")));
    }

    #[test]
    fn valid_indexed_code_like_file_prepares_code_command() {
        let root = temporary_directory("file-action-code-file");
        let executable_dir = root.join("bin");
        fs::create_dir(&executable_dir).expect("executable directory should be created");
        temporary_executable(&executable_dir, "code");
        let path = root.join("work_done.md");
        fs::write(&path, "notes").expect("temporary file should be written");
        let validated =
            validate_indexed_path(&index_with(path.clone(), false), &path.to_string_lossy())
                .expect("path should validate");

        let command = match resolve_code_executable_from_path(&[executable_dir]).map(|program| {
            PreparedCommand {
                program,
                args: vec![validated.path.to_string_lossy().into_owned()],
            }
        }) {
            Some(command) => command,
            None => panic!("code executable should resolve"),
        };

        assert_eq!(command.program, "code");
        assert_eq!(command.args, vec![path.to_string_lossy().into_owned()]);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn valid_indexed_folder_prepares_code_command() {
        let root = temporary_directory("file-action-code-folder");
        let executable_dir = root.join("bin");
        fs::create_dir(&executable_dir).expect("executable directory should be created");
        temporary_executable(&executable_dir, "code");
        let folder = root.join("rat_search");
        fs::create_dir(&folder).expect("temporary folder should be created");
        let validated =
            validate_indexed_path(&index_with(folder.clone(), true), &folder.to_string_lossy())
                .expect("path should validate");

        assert!(should_open_in_code(&validated));
        assert_eq!(
            resolve_code_executable_from_path(&[executable_dir]),
            Some("code".to_owned())
        );

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn valid_indexed_non_code_file_chooses_system_opener() {
        let root = temporary_directory("file-action-system-file");
        let path = root.join("report.pdf");
        fs::write(&path, "pdf").expect("temporary file should be written");
        let validated = validate_indexed_path(
            &index_with(path, false),
            &root.join("report.pdf").to_string_lossy(),
        )
        .expect("path should validate");

        assert_eq!(prepare_open_in_code(&validated), PreferredOpen::System);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn executable_resolution_uses_documented_priority_order() {
        let root = temporary_directory("file-action-executables");
        temporary_executable(&root, "code-insiders");
        temporary_executable(&root, "codium");
        temporary_executable(&root, "code");

        assert_eq!(
            resolve_code_executable_from_path(&[root.clone()]),
            Some("code".to_owned())
        );

        fs::remove_file(root.join("code")).expect("temporary code executable should be removed");
        assert_eq!(
            resolve_code_executable_from_path(&[root.clone()]),
            Some("codium".to_owned())
        );

        fs::remove_file(root.join("codium"))
            .expect("temporary codium executable should be removed");
        assert_eq!(
            resolve_code_executable_from_path(&[root.clone()]),
            Some("code-insiders".to_owned())
        );

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn missing_code_executable_falls_back_to_system_opener() {
        let root = temporary_directory("file-action-no-code");
        let path = root.join("main.rs");
        fs::write(&path, "fn main() {}").expect("temporary file should be written");
        let validated = validate_indexed_path(
            &index_with(path, false),
            &root.join("main.rs").to_string_lossy(),
        )
        .expect("path should validate");

        assert!(should_open_in_code(&validated));
        assert_eq!(resolve_code_executable_from_path(&[root.clone()]), None);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn prepared_code_command_does_not_use_shell_expansion() {
        let path = PathBuf::from("/home/sanuk/work/file name; rm -rf nope.md");
        let command = PreparedCommand {
            program: "code".to_owned(),
            args: vec![path.to_string_lossy().into_owned()],
        };

        assert_eq!(command.program, "code");
        assert_eq!(command.args.len(), 1);
        assert_eq!(
            command.args[0],
            "/home/sanuk/work/file name; rm -rf nope.md"
        );
    }
}
