use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

use crate::config::AppConfig;
use crate::models::{mark, media, persistent};

fn permanent_path_for(
    media_dir: &Path,
    permanent_dir: &Path,
    original_path: &Path,
) -> Option<PathBuf> {
    let relative = original_path.strip_prefix(media_dir).ok()?;
    Some(permanent_dir.join(relative))
}

fn move_path(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::rename(src, dst)
}

fn best_media_dir<'a>(config: &'a AppConfig, original_path: &Path) -> Option<&'a PathBuf> {
    config
        .media_dirs
        .iter()
        .filter(|dir| original_path.starts_with(dir))
        .max_by_key(|dir| dir.components().count())
}

pub async fn move_to_permanent(
    pool: &SqlitePool,
    media_id: i64,
    user_id: i64,
    config: &AppConfig,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let item = media::get_by_id(pool, media_id)
        .await?
        .ok_or("Media not found")?;
    if item.status != "active" {
        return Err(format!("cannot persist media in status {}", item.status).into());
    }

    let original_path = Path::new(&item.path);
    let media_dir = best_media_dir(config, original_path)
        .ok_or_else(|| format!("no matching media_dir configured for path {}", item.path))?;
    let permanent_dir = AppConfig::permanent_dir_for_media_dir(media_dir)
        .ok_or_else(|| format!("cannot derive permanent dir for {}", item.path))?;
    let dest = permanent_path_for(media_dir, &permanent_dir, original_path)
        .ok_or_else(|| format!("cannot derive permanent path for {}", item.path))?;

    if dry_run {
        tracing::info!("DRY RUN: would persist {} → {}", item.path, dest.display());
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        move_path(original_path, &dest)?;
        tracing::info!("Persisted media: {} → {}", item.path, dest.display());
    }

    media::set_permanent(pool, media_id).await?;
    persistent::set_owner(pool, media_id, user_id).await?;
    mark::clear_marks(pool, media_id).await?;

    Ok(())
}

pub async fn restore_from_permanent(
    pool: &SqlitePool,
    media_id: i64,
    user_id: i64,
    config: &AppConfig,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let item = media::get_by_id(pool, media_id)
        .await?
        .ok_or("Media not found")?;
    if item.status != "permanent" {
        return Err(format!("cannot unpersist media in status {}", item.status).into());
    }
    let owner = persistent::get_owner(pool, media_id)
        .await?
        .ok_or("persistent owner missing")?;
    if owner.user_id != user_id {
        return Err("forbidden".into());
    }

    restore_from_permanent_unchecked(pool, media_id, config, dry_run).await
}

pub async fn restore_from_permanent_unchecked(
    pool: &SqlitePool,
    media_id: i64,
    config: &AppConfig,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let item = media::get_by_id(pool, media_id)
        .await?
        .ok_or("Media not found")?;
    if item.status != "permanent" {
        return Ok(());
    }

    let original_path = Path::new(&item.path);
    let media_dir = best_media_dir(config, original_path)
        .ok_or_else(|| format!("no matching media_dir configured for path {}", item.path))?;
    let permanent_dir = AppConfig::permanent_dir_for_media_dir(media_dir)
        .ok_or_else(|| format!("cannot derive permanent dir for {}", item.path))?;
    let permanent_path = permanent_path_for(media_dir, &permanent_dir, original_path)
        .ok_or_else(|| format!("cannot derive permanent path for {}", item.path))?;

    if dry_run {
        tracing::info!(
            "DRY RUN: would unpersist {} → {}",
            permanent_path.display(),
            item.path
        );
    } else if permanent_path.exists() {
        if let Some(parent) = original_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        move_path(&permanent_path, original_path)?;
        tracing::info!(
            "Unpersisted media: {} → {}",
            permanent_path.display(),
            item.path
        );
    } else {
        return Err(format!(
            "cannot unpersist: path missing at {}",
            permanent_path.display()
        )
        .into());
    }

    media::set_active(pool, media_id).await?;
    persistent::clear_owner(pool, media_id).await?;
    mark::clear_marks(pool, media_id).await?;

    Ok(())
}
