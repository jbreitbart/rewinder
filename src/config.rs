use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub listen_addr: String,
    #[serde(default = "default_session_ttl")]
    pub session_ttl_hours: u64,
    pub media_dirs: Vec<PathBuf>,
    pub trash_dir: PathBuf,
    #[serde(default = "default_grace_period")]
    pub grace_period_days: u64,
    pub initial_admin_user: Option<String>,
}

fn default_session_ttl() -> u64 {
    720
}

fn default_grace_period() -> u64 {
    7
}

impl AppConfig {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read config file '{path}': {e}"))?;
        let config: AppConfig = toml::from_str(&content)?;

        // Validate trash_dir is not inside any media_dir
        for media_dir in &config.media_dirs {
            if config.trash_dir.starts_with(media_dir) {
                return Err(format!(
                    "trash_dir {:?} must not be inside media_dir {:?}",
                    config.trash_dir, media_dir
                )
                .into());
            }
        }

        Ok(config)
    }
}
