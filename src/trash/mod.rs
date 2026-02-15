use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

use crate::config::AppConfig;
use crate::models::{mark, media};

pub fn trash_path_for(media_dir: &Path, trash_dir: &Path, original_path: &Path) -> Option<PathBuf> {
    let relative = original_path.strip_prefix(media_dir).ok()?;
    Some(trash_dir.join(relative))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn move_path(src: &Path, dst: &Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            if src.is_dir() {
                copy_dir_recursive(src, dst)?;
                std::fs::remove_dir_all(src)?;
            } else {
                std::fs::copy(src, dst)?;
                std::fs::remove_file(src)?;
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub async fn move_to_trash(
    pool: &SqlitePool,
    media_id: i64,
    config: &AppConfig,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let item = media::get_by_id(pool, media_id)
        .await?
        .ok_or("Media not found")?;
    let original_path = Path::new(&item.path);
    let media_dir = config
        .media_dirs
        .iter()
        .filter(|dir| original_path.starts_with(dir))
        .max_by_key(|dir| dir.components().count())
        .ok_or_else(|| format!("no matching media_dir configured for path {}", item.path))?;
    let trash_dir = AppConfig::trash_dir_for_media_dir(media_dir)
        .ok_or_else(|| format!("no matching media_dir configured for path {}", item.path))?;

    let dest = trash_path_for(media_dir, &trash_dir, original_path)
        .ok_or_else(|| format!("failed to derive trash path for {}", item.path))?;

    if dry_run {
        tracing::info!("DRY RUN: would move {} → {}", item.path, dest.display());
    } else {
        // Ensure destination parent exists
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Move to trash; fall back to copy+delete for cross-device moves
        move_path(original_path, &dest)?;

        tracing::info!("Moved to trash: {} → {}", item.path, dest.display());
    }

    media::set_trashed(pool, media_id).await?;

    Ok(())
}

pub async fn rescue_from_trash(
    pool: &SqlitePool,
    media_id: i64,
    config: &AppConfig,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let item = media::get_by_id(pool, media_id)
        .await?
        .ok_or("Media not found")?;
    let original_path = Path::new(&item.path);
    let media_dir = config
        .media_dirs
        .iter()
        .filter(|dir| original_path.starts_with(dir))
        .max_by_key(|dir| dir.components().count())
        .ok_or_else(|| format!("no matching media_dir configured for path {}", item.path))?;
    let trash_dir = AppConfig::trash_dir_for_media_dir(media_dir)
        .ok_or_else(|| format!("no matching media_dir configured for path {}", item.path))?;

    let trash_location = trash_path_for(media_dir, &trash_dir, original_path)
        .ok_or_else(|| format!("failed to derive trash path for {}", item.path))?;

    if dry_run {
        tracing::info!(
            "DRY RUN: would rescue {} → {}",
            trash_location.display(),
            item.path
        );
    } else if trash_location.exists() {
        // Ensure parent directory exists
        if let Some(parent) = original_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        move_path(&trash_location, original_path)?;
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
    config: &AppConfig,
    grace_period_days: u64,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let expired = media::list_expired_trash(pool, grace_period_days).await?;

    for item in &expired {
        let original_path = Path::new(&item.path);
        let Some(media_dir) = config
            .media_dirs
            .iter()
            .filter(|dir| original_path.starts_with(dir))
            .max_by_key(|dir| dir.components().count())
        else {
            tracing::warn!(
                "Skipping cleanup for {}: no matching media_dir configured",
                item.path
            );
            continue;
        };
        let Some(trash_dir) = AppConfig::trash_dir_for_media_dir(media_dir) else {
            tracing::warn!(
                "Skipping cleanup for {}: cannot derive trash dir",
                item.path
            );
            continue;
        };
        let Some(trash_location) = trash_path_for(media_dir, &trash_dir, original_path) else {
            tracing::warn!(
                "Skipping cleanup for {}: cannot derive trash location",
                item.path
            );
            continue;
        };
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
    config: &AppConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let trashed = media::list_trashed(pool).await?;

    for item in &trashed {
        let original_path = Path::new(&item.path);
        let Some(media_dir) = config
            .media_dirs
            .iter()
            .filter(|dir| original_path.starts_with(dir))
            .max_by_key(|dir| dir.components().count())
        else {
            tracing::warn!(
                "Skipping missing-trash check for {}: no matching media_dir configured",
                item.path
            );
            continue;
        };
        let Some(trash_dir) = AppConfig::trash_dir_for_media_dir(media_dir) else {
            tracing::warn!(
                "Skipping missing-trash check for {}: cannot derive trash dir",
                item.path
            );
            continue;
        };
        let Some(trash_location) = trash_path_for(media_dir, &trash_dir, original_path) else {
            tracing::warn!(
                "Skipping missing-trash check for {}: cannot derive trash location",
                item.path
            );
            continue;
        };
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
    config: &AppConfig,
    dry_run: bool,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    if mark::all_users_marked(pool, media_id).await? {
        move_to_trash(pool, media_id, config, dry_run).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}
