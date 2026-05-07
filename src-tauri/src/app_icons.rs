use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

pub(crate) const GENERIC_APP_ICON: &str = "app";

const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &["svg", "png", "xpm", "jpg", "jpeg", "webp"];

#[derive(Debug, Clone)]
pub(crate) struct AppIconResolver {
    icon_files: Vec<PathBuf>,
}

impl AppIconResolver {
    pub(crate) fn new() -> Self {
        Self::from_roots(default_icon_roots())
    }

    pub(crate) fn from_roots(roots: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut icon_files = Vec::new();

        for root in roots {
            collect_icon_files(&root, &mut icon_files);
        }

        icon_files.sort();
        icon_files.dedup();

        Self { icon_files }
    }

    pub(crate) fn resolve(&self, icon: Option<&str>) -> String {
        let Some(icon) = icon.map(str::trim).filter(|icon| !icon.is_empty()) else {
            return GENERIC_APP_ICON.to_owned();
        };

        let icon_path = Path::new(icon);
        if icon_path.is_absolute() {
            return is_readable_supported_image(icon_path)
                .then(|| icon_path.to_string_lossy().into_owned())
                .unwrap_or_else(|| GENERIC_APP_ICON.to_owned());
        }

        self.resolve_theme_icon(icon)
            .unwrap_or_else(|| GENERIC_APP_ICON.to_owned())
    }

    fn resolve_theme_icon(&self, icon_name: &str) -> Option<String> {
        self.icon_files
            .iter()
            .filter(|path| icon_name_matches_path(icon_name, path))
            .filter(|path| is_readable_supported_image(path))
            .min_by(|left, right| compare_icon_candidates(left, right))
            .map(|path| path.to_string_lossy().into_owned())
    }
}

fn default_icon_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(home) = env::var("HOME") {
        if !home.trim().is_empty() {
            let home = PathBuf::from(home);
            push_unique_root(&mut roots, &mut seen, home.join(".local/share/icons"));
            push_unique_root(&mut roots, &mut seen, home.join(".icons"));
        }
    }

    push_unique_root(&mut roots, &mut seen, PathBuf::from("/usr/share/icons"));
    push_unique_root(
        &mut roots,
        &mut seen,
        PathBuf::from("/usr/local/share/icons"),
    );
    push_unique_root(&mut roots, &mut seen, PathBuf::from("/usr/share/pixmaps"));

    if let Ok(data_dirs) = env::var("XDG_DATA_DIRS") {
        for data_dir in data_dirs
            .split(':')
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
        {
            let data_dir = PathBuf::from(data_dir);
            push_unique_root(&mut roots, &mut seen, data_dir.join("icons"));
            push_unique_root(&mut roots, &mut seen, data_dir.join("pixmaps"));
        }
    }

    roots
}

fn push_unique_root(roots: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, root: PathBuf) {
    if seen.insert(root.clone()) {
        roots.push(root);
    }
}

fn collect_icon_files(path: &Path, icon_files: &mut Vec<PathBuf>) {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };

    if metadata.file_type().is_symlink() {
        return;
    }

    if metadata.is_file() {
        if has_supported_image_extension(path) {
            icon_files.push(path.to_path_buf());
        }
        return;
    }

    if !metadata.is_dir() {
        return;
    }

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        collect_icon_files(&entry.path(), icon_files);
    }
}

fn icon_name_matches_path(icon_name: &str, path: &Path) -> bool {
    path.file_name()
        .and_then(|file_name| file_name.to_str())
        .is_some_and(|file_name| file_name == icon_name)
        || path
            .file_stem()
            .and_then(|file_stem| file_stem.to_str())
            .is_some_and(|file_stem| file_stem == icon_name)
}

fn compare_icon_candidates(left: &&PathBuf, right: &&PathBuf) -> std::cmp::Ordering {
    icon_size_rank(left)
        .cmp(&icon_size_rank(right))
        .then_with(|| icon_extension_rank(left).cmp(&icon_extension_rank(right)))
        .then_with(|| left.cmp(right))
}

fn icon_size_rank(path: &Path) -> u8 {
    let mut rank = 4;

    for component in path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
    {
        rank = rank.min(match component {
            "48x48" => 0,
            "64x64" => 1,
            "32x32" => 2,
            "scalable" => 3,
            _ => 4,
        });
    }

    rank
}

fn icon_extension_rank(path: &Path) -> u8 {
    match normalized_extension(path).as_deref() {
        Some("svg") => 0,
        Some("png") => 1,
        Some("xpm") => 2,
        Some("webp") => 3,
        Some("jpg" | "jpeg") => 4,
        _ => 5,
    }
}

fn is_readable_supported_image(path: &Path) -> bool {
    path.is_file() && has_supported_image_extension(path) && fs::File::open(path).is_ok()
}

fn has_supported_image_extension(path: &Path) -> bool {
    normalized_extension(path)
        .as_deref()
        .is_some_and(|extension| SUPPORTED_IMAGE_EXTENSIONS.contains(&extension))
}

fn normalized_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_lowercase)
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::*;

    fn temporary_directory(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time should be after Unix epoch")
            .as_nanos();
        let path =
            env::temp_dir().join(format!("rat-search-{name}-{}-{unique}", std::process::id()));

        fs::create_dir_all(&path).expect("temporary directory should be created");

        path
    }

    fn write_file(path: &Path) {
        fs::create_dir_all(path.parent().expect("test path should have a parent"))
            .expect("test parent directory should be created");
        fs::write(path, "icon").expect("test icon should be written");
    }

    #[test]
    fn absolute_readable_image_path_is_returned_unchanged() {
        let root = temporary_directory("absolute-icon");
        let icon = root.join("app.png");
        write_file(&icon);
        let resolver = AppIconResolver::from_roots([]);

        assert_eq!(
            resolver.resolve(Some(&icon.to_string_lossy())),
            icon.to_string_lossy()
        );

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn missing_or_unsupported_absolute_path_falls_back() {
        let root = temporary_directory("absolute-icon-fallback");
        let missing_icon = root.join("missing.png");
        let unsupported_icon = root.join("app.txt");
        write_file(&unsupported_icon);
        let resolver = AppIconResolver::from_roots([]);

        assert_eq!(
            resolver.resolve(Some(&missing_icon.to_string_lossy())),
            GENERIC_APP_ICON
        );
        assert_eq!(
            resolver.resolve(Some(&unsupported_icon.to_string_lossy())),
            GENERIC_APP_ICON
        );

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn theme_name_lookup_finds_nested_icon_assets() {
        let root = temporary_directory("theme-icon");
        let icon = root.join("hicolor/48x48/apps/firefox.png");
        write_file(&icon);
        let resolver = AppIconResolver::from_roots([root.clone()]);

        assert_eq!(
            resolver.resolve(Some("firefox")),
            icon.to_string_lossy().into_owned()
        );

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn direct_pixmaps_style_assets_are_supported() {
        let root = temporary_directory("pixmaps-icon");
        let icon = root.join("org.example.App.xpm");
        write_file(&icon);
        let resolver = AppIconResolver::from_roots([root.clone()]);

        assert_eq!(
            resolver.resolve(Some("org.example.App")),
            icon.to_string_lossy().into_owned()
        );

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn candidate_ranking_prefers_desired_size_and_format_deterministically() {
        let root = temporary_directory("ranked-icons");
        let scalable_svg = root.join("hicolor/scalable/apps/editor.svg");
        let preferred_png = root.join("hicolor/48x48/apps/editor.png");
        let larger_png = root.join("hicolor/64x64/apps/editor.png");
        let preferred_xpm = root.join("legacy/48x48/apps/editor.xpm");
        write_file(&scalable_svg);
        write_file(&larger_png);
        write_file(&preferred_xpm);
        write_file(&preferred_png);
        let resolver = AppIconResolver::from_roots([root.clone()]);

        assert_eq!(
            resolver.resolve(Some("editor")),
            preferred_png.to_string_lossy().into_owned()
        );

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }

    #[test]
    fn unresolved_icons_use_generic_app_fallback() {
        let root = temporary_directory("unresolved-icon");
        let resolver = AppIconResolver::from_roots([root.clone()]);

        assert_eq!(resolver.resolve(None), GENERIC_APP_ICON);
        assert_eq!(resolver.resolve(Some("missing-app")), GENERIC_APP_ICON);

        fs::remove_dir_all(root).expect("temporary directory should be removed");
    }
}
