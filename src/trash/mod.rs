use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

use crate::models::{mark, media};

pub fn trash_path_for(trash_dir: &Path, original_path: &str) -> PathBuf {
    let original = Path::new(original_path);
    let name = original
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    trash_dir.join(name)
}

pub async fn move_to_trash(
    pool: &SqlitePool,
    media_id: i64,
    trash_dir: &Path,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let item = media::get_by_id(pool, media_id)
        .await?
        .ok_or("Media not found")?;

    let dest = trash_path_for(trash_dir, &item.path);

    if dry_run {
        tracing::info!("DRY RUN: would move {} → {}", item.path, dest.display());
    } else {
        // Ensure trash dir exists
        std::fs::create_dir_all(trash_dir)?;

        // Move to trash
        std::fs::rename(&item.path, &dest)?;

        tracing::info!("Moved to trash: {} → {}", item.path, dest.display());
    }

    media::set_trashed(pool, media_id).await?;

    Ok(())
}

pub async fn rescue_from_trash(
    pool: &SqlitePool,
    media_id: i64,
    trash_dir: &Path,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let item = media::get_by_id(pool, media_id)
        .await?
        .ok_or("Media not found")?;

    let trash_location = trash_path_for(trash_dir, &item.path);

    if dry_run {
        tracing::info!("DRY RUN: would rescue {} → {}", trash_location.display(), item.path);
    } else if trash_location.exists() {
        // Ensure parent directory exists
        if let Some(parent) = Path::new(&item.path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&trash_location, &item.path)?;
    } else {
        return Err(format!(
            "Cannot rescue: file no longer exists in trash at {}",
            trash_location.display()
        )
        .into());
    }

    media::set_active(pool, media_id).await?;
    mark::clear_marks(pool, media_id).await?;
    tracing::info!("Rescued from trash: {}", item.path);

    Ok(())
}

pub async fn cleanup_expired(
    pool: &SqlitePool,
    trash_dir: &Path,
    grace_period_days: u64,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let expired = media::list_expired_trash(pool, grace_period_days).await?;

    for item in &expired {
        let trash_location = trash_path_for(trash_dir, &item.path);
        if dry_run {
            tracing::info!("DRY RUN: would delete {}", trash_location.display());
        } else if trash_location.exists() {
            if let Err(e) = std::fs::remove_dir_all(&trash_location) {
                tracing::error!("Failed to delete {}: {e}", trash_location.display());
                continue;
            }
        }
        media::set_gone(pool, item.id).await?;
        tracing::info!("Permanently deleted: {}", item.path);
    }

    if !expired.is_empty() {
        tracing::info!("Cleaned up {} expired trash items", expired.len());
    }

    Ok(())
}

/// Mark trashed items as gone if their files were manually removed from the trash dir.
pub async fn cleanup_missing_trash(
    pool: &SqlitePool,
    trash_dir: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let trashed = media::list_trashed(pool).await?;

    for item in &trashed {
        let trash_location = trash_path_for(trash_dir, &item.path);
        if !trash_location.exists() {
            media::set_gone(pool, item.id).await?;
            mark::clear_marks(pool, item.id).await?;
            tracing::info!("Trashed item missing from disk, marked gone: {}", item.path);
        }
    }

    Ok(())
}

pub async fn check_and_trash(
    pool: &SqlitePool,
    media_id: i64,
    trash_dir: &Path,
    dry_run: bool,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    if mark::all_users_marked(pool, media_id).await? {
        move_to_trash(pool, media_id, trash_dir, dry_run).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}
