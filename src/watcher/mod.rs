use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::models::media;
use crate::scanner;

pub async fn start(
    pool: SqlitePool,
    media_dirs: Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tx, mut rx) = mpsc::channel::<Event>(100);

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.blocking_send(event);
            }
        },
        notify::Config::default(),
    )?;

    for dir in &media_dirs {
        if dir.exists() {
            watcher.watch(dir, RecursiveMode::NonRecursive)?;
            tracing::info!("Watching directory: {}", dir.display());
        } else {
            tracing::warn!(
                "Media directory does not exist, skipping watch: {}",
                dir.display()
            );
        }
    }

    let media_dirs = Arc::new(media_dirs);

    tokio::spawn(async move {
        // Keep watcher alive
        let _watcher = watcher;

        while let Some(event) = rx.recv().await {
            match event.kind {
                EventKind::Create(_) => {
                    for path in &event.paths {
                        if path.is_dir() {
                            if let Some(parent) = path.parent() {
                                let parent_buf = parent.to_path_buf();
                                if media_dirs.contains(&parent_buf) {
                                    tracing::info!("New directory detected: {}", path.display());
                                    if let Err(e) = scanner::scan_directory(&pool, parent).await {
                                        tracing::error!("Error scanning after create: {e}");
                                    }
                                }
                            }
                        }
                    }
                }
                EventKind::Remove(_) => {
                    for path in &event.paths {
                        let path_str = path.to_string_lossy().to_string();
                        tracing::info!("Directory removed: {path_str}");
                        if let Err(e) = media::mark_gone_by_path(&pool, &path_str).await {
                            tracing::error!("Error marking gone: {e}");
                        }
                    }
                }
                _ => {}
            }
        }
    });

    Ok(())
}
