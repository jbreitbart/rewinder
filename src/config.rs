use serde::Deserialize;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub listen_addr: String,
    pub media_dirs: Vec<PathBuf>,
    #[serde(default = "default_grace_period")]
    pub grace_period_days: u64,
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_hours: u64,
    pub initial_admin_user: Option<String>,
    pub tmdb_api_key: Option<String>,
}

fn default_grace_period() -> u64 {
    7
}

fn default_cleanup_interval() -> u64 {
    1
}

impl AppConfig {
    pub fn trash_dir_for_media_dir(media_dir: &std::path::Path) -> Option<PathBuf> {
        let parent = media_dir.parent()?;
        let name = media_dir.file_name()?;
        let mut trash_name = OsString::from(name);
        trash_name.push("_trash");
        Some(parent.join(trash_name))
    }

    pub fn trash_dir_for_media_path(&self, media_path: &std::path::Path) -> Option<PathBuf> {
        // Pick the most specific matching media dir in case of nested paths.
        let best_match = self
            .media_dirs
            .iter()
            .filter(|dir| media_path.starts_with(dir))
            .max_by_key(|dir| dir.components().count())?;
        Self::trash_dir_for_media_dir(best_match)
    }

    pub fn all_trash_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = self
            .media_dirs
            .iter()
            .filter_map(|d| Self::trash_dir_for_media_dir(d))
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    }

    pub fn permanent_dir_for_media_dir(media_dir: &std::path::Path) -> Option<PathBuf> {
        let parent = media_dir.parent()?;
        let name = media_dir.file_name()?;
        let mut permanent_name = OsString::from(name);
        permanent_name.push("_permanent");
        Some(parent.join(permanent_name))
    }

    pub fn permanent_dir_for_media_path(&self, media_path: &std::path::Path) -> Option<PathBuf> {
        let best_match = self
            .media_dirs
            .iter()
            .filter(|dir| media_path.starts_with(dir))
            .max_by_key(|dir| dir.components().count())?;
        Self::permanent_dir_for_media_dir(best_match)
    }

    pub fn all_permanent_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = self
            .media_dirs
            .iter()
            .filter_map(|d| Self::permanent_dir_for_media_dir(d))
            .collect();
        dirs.sort();
        dirs.dedup();
        dirs
    }

    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read config file '{path}': {e}"))?;
        let config: AppConfig = toml::from_str(&content)?;

        // Validate each media_dir can produce a sibling trash directory name.
        for media_dir in &config.media_dirs {
            if Self::trash_dir_for_media_dir(media_dir).is_none() {
                return Err(format!(
                    "media_dir {:?} has no valid parent or name to derive trash directory",
                    media_dir
                )
                .into());
            }
            if Self::permanent_dir_for_media_dir(media_dir).is_none() {
                return Err(format!(
                    "media_dir {:?} has no valid parent or name to derive permanent directory",
                    media_dir
                )
                .into());
            }
        }

        Ok(config)
    }
}
