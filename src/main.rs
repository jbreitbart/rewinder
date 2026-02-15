use clap::Parser;
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::Arc;
use tower_http::services::ServeDir;

use rewinder::config::AppConfig;
use rewinder::routes::AppState;
use rewinder::{auth, db, models, scanner, trash, watcher};

#[derive(Parser)]
#[command(name = "rewinder", about = "Plex media storage manager")]
struct Cli {
    /// Path to config file
    #[arg(long, env = "REWINDER_CONFIG", default_value = "rewinder.toml")]
    config: String,

    /// Dry-run mode: scan and mark as usual, but never move or delete files on disk
    #[arg(long)]
    dry_run: bool,
}

fn ensure_dir_readable_and_writable(path: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !path.is_dir() {
        return Err(format!("path is not a directory: {}", path.display()).into());
    }

    // Readability check.
    std::fs::read_dir(path)
        .map_err(|e| format!("directory not readable ({}): {e}", path.display()))?;

    // Writability check.
    let unique = format!(
        ".rewinder_perm_check_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("system clock error: {e}"))?
            .as_nanos()
    );
    let probe = path.join(unique);
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
        .map_err(|e| format!("directory not writable ({}): {e}", path.display()))?;
    std::fs::remove_file(&probe)
        .map_err(|e| format!("failed to clean up permission probe {}: {e}", probe.display()))?;

    Ok(())
}

fn validate_storage_access(config: &AppConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for media_dir in &config.media_dirs {
        ensure_dir_readable_and_writable(media_dir)?;
    }

    for trash_dir in config.all_trash_dirs() {
        if !trash_dir.exists() {
            std::fs::create_dir_all(&trash_dir).map_err(|e| {
                format!(
                    "failed to create derived trash directory {}: {e}",
                    trash_dir.display()
                )
            })?;
        }
        ensure_dir_readable_and_writable(&trash_dir)?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .compact()
        .with_target(true)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rewinder=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let config = AppConfig::load(&cli.config)?;
    validate_storage_access(&config)?;
    let dry_run = cli.dry_run;
    if dry_run {
        tracing::warn!("*** DRY-RUN MODE ACTIVE — no files will be moved or deleted ***");
        tracing::warn!("Database state will diverge from disk. Back up your database before using this mode.");
    }
    tracing::info!("Loaded config from {}", cli.config);

    let pool = db::init_pool(&config.database_url).await?;
    tracing::info!("Database initialized");

    // Seed admin user if configured
    if let Some(ref admin_user) = config.initial_admin_user {
        auth::seed_admin(&pool, admin_user).await?;
    }

    // Run initial scan
    scanner::full_scan(&pool, &config.media_dirs).await?;

    // Start filesystem watcher
    watcher::start(pool.clone(), config.media_dirs.clone()).await?;

    // Start background maintenance task
    if config.cleanup_interval_hours > 0 {
        let cleanup_pool = pool.clone();
        let grace_period = config.grace_period_days;
        let cleanup_config = config.clone();
        let media_dirs = config.media_dirs.clone();
        let cleanup_interval_hours = config.cleanup_interval_hours;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(
                cleanup_interval_hours * 3600,
            ));
            loop {
                interval.tick().await;
                // Re-scan to detect externally removed directories
                if let Err(e) = scanner::full_scan(&cleanup_pool, &media_dirs).await {
                    tracing::error!("Periodic scan error: {e}");
                }
                // Clean up marks for items that are gone
                match models::media::cleanup_gone_marks(&cleanup_pool).await {
                    Ok(n) if n > 0 => tracing::info!("Cleaned up {n} marks for gone media"),
                    Err(e) => tracing::error!("Mark cleanup error: {e}"),
                    _ => {}
                }
                if let Err(e) = trash::cleanup_missing_trash(&cleanup_pool, &cleanup_config).await {
                    tracing::error!("Missing trash cleanup error: {e}");
                }
                if let Err(e) =
                    trash::cleanup_expired(&cleanup_pool, &cleanup_config, grace_period, dry_run).await
                {
                    tracing::error!("Trash cleanup error: {e}");
                }
                if let Err(e) = auth::session::cleanup_expired(&cleanup_pool).await {
                    tracing::error!("Session cleanup error: {e}");
                }
            }
        });
    } else {
        tracing::info!("Automatic cleanup disabled (cleanup_interval_hours = 0)");
    }

    let state = AppState {
        pool,
        config: Arc::new(config.clone()),
        dry_run,
    };

    let app = rewinder::routes::build_router(state)
        .nest_service("/static", ServeDir::new("static"));

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    tracing::info!("Listening on {}", config.listen_addr);
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rewinder::config::AppConfig;
    use tempfile::tempdir;

    fn test_config_with_media_dirs(media_dirs: Vec<std::path::PathBuf>) -> AppConfig {
        AppConfig {
            database_url: ":memory:".to_string(),
            listen_addr: "127.0.0.1:0".to_string(),
            media_dirs,
            grace_period_days: 7,
            cleanup_interval_hours: 1,
            initial_admin_user: None,
        }
    }

    #[test]
    fn storage_validation_fails_for_nonexistent_media_dir() {
        let base = tempdir().expect("failed to create tempdir");
        let missing = base.path().join("does-not-exist");
        let cfg = test_config_with_media_dirs(vec![missing]);

        let err = validate_storage_access(&cfg).expect_err("expected missing dir failure");
        let msg = err.to_string();
        assert!(
            msg.contains("not a directory") || msg.contains("not readable"),
            "unexpected error message: {msg}"
        );
    }

    #[test]
    fn storage_validation_fails_for_non_directory_media_path() {
        let base = tempdir().expect("failed to create tempdir");
        let file_path = base.path().join("not-a-directory");
        std::fs::write(&file_path, "x").expect("failed to create file");
        let cfg = test_config_with_media_dirs(vec![file_path]);

        let err = validate_storage_access(&cfg).expect_err("expected non-directory failure");
        assert!(err.to_string().contains("not a directory"));
    }

    #[cfg(unix)]
    #[test]
    fn storage_validation_fails_for_unreadable_and_unwritable_directories() {
        use std::os::unix::fs::PermissionsExt;

        let unreadable = tempdir().expect("failed to create unreadable tempdir");
        let unwritable = tempdir().expect("failed to create unwritable tempdir");

        let unreadable_mode = std::fs::Permissions::from_mode(0o333);
        let unwritable_mode = std::fs::Permissions::from_mode(0o555);
        std::fs::set_permissions(unreadable.path(), unreadable_mode)
            .expect("failed to chmod unreadable dir");
        std::fs::set_permissions(unwritable.path(), unwritable_mode)
            .expect("failed to chmod unwritable dir");

        let read_err = ensure_dir_readable_and_writable(unreadable.path())
            .expect_err("expected unreadable dir to fail");
        let read_msg = read_err.to_string();

        let write_err = ensure_dir_readable_and_writable(unwritable.path())
            .expect_err("expected unwritable dir to fail");
        let write_msg = write_err.to_string();

        // Restore permissions so tempdir cleanup can remove directories.
        std::fs::set_permissions(unreadable.path(), std::fs::Permissions::from_mode(0o755))
            .expect("failed to restore unreadable dir perms");
        std::fs::set_permissions(unwritable.path(), std::fs::Permissions::from_mode(0o755))
            .expect("failed to restore unwritable dir perms");

        assert!(
            read_msg.contains("not readable"),
            "unexpected unreadable error message: {read_msg}"
        );
        assert!(
            write_msg.contains("not writable"),
            "unexpected unwritable error message: {write_msg}"
        );
    }
}
