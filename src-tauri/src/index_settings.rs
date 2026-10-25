use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::file_index;

const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IndexConfiguration {
    path: PathBuf,
    roots: Vec<PathBuf>,
    exclusions: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ConfigurationFile {
    version: u32,
    roots: Vec<PathBuf>,
    exclusions: Vec<PathBuf>,
}

impl IndexConfiguration {
    pub(crate) fn defaults(path: PathBuf, home: &Path) -> Self {
        Self {
            path,
            roots: file_index::default_index_roots_from_home(home),
            exclusions: Vec::new(),
        }
    }

    pub(crate) fn load(path: PathBuf, home: &Path) -> Self {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return Self::defaults(path, home),
            Err(error) => {
                eprintln!(
                    "failed to read index settings '{}': {error}",
                    path.display()
                );
                return Self::defaults(path, home);
            }
        };

        let file = match serde_json::from_str::<ConfigurationFile>(&contents) {
            Ok(file) if file.version == CONFIG_VERSION => file,
            Ok(file) => {
                eprintln!("unsupported index settings version {}", file.version);
                return Self::defaults(path, home);
            }
            Err(error) => {
                eprintln!(
                    "failed to parse index settings '{}': {error}",
                    path.display()
                );
                return Self::defaults(path, home);
            }
        };

        let roots = file
            .roots
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let exclusions = file
            .exclusions
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        match Self::validated(path.clone(), &roots, &exclusions, home) {
            Ok(configuration) => configuration,
            Err(error) => {
                eprintln!("invalid index settings '{}': {error}", path.display());
                Self::defaults(path, home)
            }
        }
    }

    pub(crate) fn validated(
        path: PathBuf,
        roots: &[String],
        exclusions: &[String],
        home: &Path,
    ) -> Result<Self, String> {
        let mut normalized_roots = normalize_paths(roots, home)?;
        let normalized_exclusions = normalize_paths(exclusions, home)?;

        if normalized_roots.is_empty() {
            return Err("Choose at least one folder to index".to_owned());
        }

        normalized_roots.sort_by(|left, right| {
            left.components()
                .count()
                .cmp(&right.components().count())
                .then_with(|| left.cmp(right))
        });

        let mut roots_without_overlaps: Vec<PathBuf> = Vec::new();
        for root in normalized_roots {
            if !roots_without_overlaps
                .iter()
                .any(|parent| root.starts_with(parent))
            {
                roots_without_overlaps.push(root);
            }
        }

        Ok(Self {
            path,
            roots: roots_without_overlaps,
            exclusions: normalized_exclusions,
        })
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create index settings directory '{}': {error}",
                    parent.display()
                )
            })?;
        }

        let file = ConfigurationFile {
            version: CONFIG_VERSION,
            roots: self.roots.clone(),
            exclusions: self.exclusions.clone(),
        };
        let contents = serde_json::to_string_pretty(&file)
            .map_err(|error| format!("failed to serialize index settings: {error}"))?;
        let temp_path = self.path.with_extension("json.tmp");
        fs::write(&temp_path, contents).map_err(|error| {
            format!(
                "failed to write index settings '{}': {error}",
                temp_path.display()
            )
        })?;
        fs::rename(&temp_path, &self.path).map_err(|error| {
            format!(
                "failed to persist index settings '{}': {error}",
                self.path.display()
            )
        })
    }

    pub(crate) fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn exclusions(&self) -> &[PathBuf] {
        &self.exclusions
    }

    pub(crate) fn roots_for_frontend(&self) -> Vec<String> {
        self.roots
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    }

    pub(crate) fn exclusions_for_frontend(&self) -> Vec<String> {
        self.exclusions
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    }
}

fn normalize_paths(paths: &[String], home: &Path) -> Result<Vec<PathBuf>, String> {
    let mut normalized = Vec::new();
    for raw_path in paths {
        let raw_path = raw_path.trim();
        if raw_path.is_empty() {
            continue;
        }
        let expanded = if raw_path == "~" {
            home.to_path_buf()
        } else if let Some(rest) = raw_path.strip_prefix("~/") {
            home.join(rest)
        } else {
            PathBuf::from(raw_path)
        };

        if !expanded.is_absolute() {
            return Err(format!("index paths must be absolute: {raw_path}"));
        }

        let path = normalize_absolute_path(&expanded)?;
        if !normalized.contains(&path) {
            normalized.push(path);
        }
    }
    Ok(normalized)
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!("index path escapes its root: {}", path.display()));
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rat-search-{name}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn defaults_use_existing_user_folders_and_no_exclusions() {
        let home = temp_dir("index-defaults");
        fs::create_dir_all(home.join("Documents")).unwrap();
        fs::create_dir_all(home.join("Downloads")).unwrap();
        let config = IndexConfiguration::defaults(home.join("index.json"), &home);

        assert_eq!(
            config.roots(),
            &[home.join("Documents"), home.join("Downloads")]
        );
        assert!(config.exclusions().is_empty());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn validation_expands_home_deduplicates_and_removes_nested_roots() {
        let home = temp_dir("index-normalize");
        let config = IndexConfiguration::validated(
            home.join("index.json"),
            &[
                "~/Projects".into(),
                "~/Projects/app".into(),
                "~/Projects/./".into(),
            ],
            &[
                "~/Projects/app/target".into(),
                "~/Projects/app/target".into(),
            ],
            &home,
        )
        .unwrap();

        assert_eq!(config.roots(), &[home.join("Projects")]);
        assert_eq!(config.exclusions(), &[home.join("Projects/app/target")]);
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn validation_rejects_relative_paths_and_empty_roots() {
        let home = temp_dir("index-invalid");
        assert!(IndexConfiguration::validated(
            home.join("index.json"),
            &["Documents".into()],
            &[],
            &home
        )
        .is_err());
        assert!(IndexConfiguration::validated(home.join("index.json"), &[], &[], &home).is_err());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn missing_paths_can_be_saved_and_reloaded_for_future_mounts() {
        let home = temp_dir("index-missing");
        let path = home.join("index.json");
        let config = IndexConfiguration::validated(
            path.clone(),
            &[home.join("unmounted").to_string_lossy().into_owned()],
            &[],
            &home,
        )
        .unwrap();
        config.save().unwrap();
        let loaded = IndexConfiguration::load(path, &home);
        assert_eq!(loaded.roots(), &[home.join("unmounted")]);
        fs::remove_dir_all(home).unwrap();
    }
}
