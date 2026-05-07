use crate::file_index::FileRecord;

pub(crate) const FOLDER_ICON: &str = "folder";
pub(crate) const GENERIC_FILE_ICON: &str = "file";
pub(crate) const PDF_FILE_ICON: &str = "file-pdf";
pub(crate) const TEXT_FILE_ICON: &str = "file-text";
pub(crate) const IMAGE_FILE_ICON: &str = "file-image";
pub(crate) const VIDEO_FILE_ICON: &str = "file-video";
pub(crate) const AUDIO_FILE_ICON: &str = "file-audio";
pub(crate) const ARCHIVE_FILE_ICON: &str = "file-archive";
pub(crate) const DOCUMENT_FILE_ICON: &str = "file-document";

pub(crate) fn icon_for_record(record: &FileRecord) -> &'static str {
    if record.is_dir {
        return FOLDER_ICON;
    }

    icon_for_extension(record.extension.as_deref())
}

fn icon_for_extension(extension: Option<&str>) -> &'static str {
    let Some(extension) = extension.map(|extension| extension.to_ascii_lowercase()) else {
        return GENERIC_FILE_ICON;
    };

    match extension.as_str() {
        "pdf" => PDF_FILE_ICON,
        "txt" | "md" | "markdown" | "json" | "toml" | "yaml" | "yml" | "xml" | "csv" | "log"
        | "rs" | "js" | "ts" | "svelte" | "html" | "css" | "scss" | "py" | "sh" => TEXT_FILE_ICON,
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "bmp" | "tiff" | "xpm" => IMAGE_FILE_ICON,
        "mp4" | "mkv" | "webm" | "mov" | "avi" => VIDEO_FILE_ICON,
        "mp3" | "wav" | "ogg" | "flac" | "m4a" => AUDIO_FILE_ICON,
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "deb" | "rpm" | "appimage" => {
            ARCHIVE_FILE_ICON
        }
        "doc" | "docx" | "odt" | "rtf" | "xls" | "xlsx" | "ods" | "ppt" | "pptx" | "odp" => {
            DOCUMENT_FILE_ICON
        }
        _ => GENERIC_FILE_ICON,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn file_with_extension(extension: Option<&str>) -> FileRecord {
        FileRecord {
            id: "file:/tmp/example".to_owned(),
            path: PathBuf::from("/tmp/example"),
            file_name: "example".to_owned(),
            parent_path: PathBuf::from("/tmp"),
            is_dir: false,
            extension: extension.map(ToOwned::to_owned),
            modified_time: None,
            size: Some(10),
        }
    }

    #[test]
    fn folder_records_use_folder_icon() {
        let record = FileRecord::new(PathBuf::from("/tmp/Projects"), true, None, None);

        assert_eq!(icon_for_record(&record), FOLDER_ICON);
    }

    #[test]
    fn extension_matching_is_case_insensitive() {
        let record = file_with_extension(Some("PDF"));

        assert_eq!(icon_for_record(&record), PDF_FILE_ICON);
    }

    #[test]
    fn supported_categories_map_to_expected_icons() {
        let categories = [
            ("pdf", PDF_FILE_ICON),
            ("md", TEXT_FILE_ICON),
            ("json", TEXT_FILE_ICON),
            ("png", IMAGE_FILE_ICON),
            ("svg", IMAGE_FILE_ICON),
            ("mp4", VIDEO_FILE_ICON),
            ("mp3", AUDIO_FILE_ICON),
            ("zip", ARCHIVE_FILE_ICON),
            ("appimage", ARCHIVE_FILE_ICON),
            ("docx", DOCUMENT_FILE_ICON),
            ("ppt", DOCUMENT_FILE_ICON),
        ];

        for (extension, expected_icon) in categories {
            let record = file_with_extension(Some(extension));

            assert_eq!(icon_for_record(&record), expected_icon);
        }
    }

    #[test]
    fn extensionless_and_unknown_files_use_generic_file_icon() {
        assert_eq!(
            icon_for_record(&file_with_extension(None)),
            GENERIC_FILE_ICON
        );
        assert_eq!(
            icon_for_record(&file_with_extension(Some("unknown"))),
            GENERIC_FILE_ICON
        );
    }
}
