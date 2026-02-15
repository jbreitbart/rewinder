use clap::Parser;
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

    // Start background maintenance task (hourly)
    let cleanup_pool = pool.clone();
    let trash_dir = config.trash_dir.clone();
    let grace_period = config.grace_period_days;
    let media_dirs = config.media_dirs.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
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
            if let Err(e) = trash::cleanup_missing_trash(&cleanup_pool, &trash_dir).await {
                tracing::error!("Missing trash cleanup error: {e}");
            }
            if let Err(e) = trash::cleanup_expired(&cleanup_pool, &trash_dir, grace_period, dry_run).await {
                tracing::error!("Trash cleanup error: {e}");
            }
            if let Err(e) = auth::session::cleanup_expired(&cleanup_pool).await {
                tracing::error!("Session cleanup error: {e}");
            }
        }
    });

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
