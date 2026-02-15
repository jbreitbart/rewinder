use crate::models::media;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

/// Parse a movie directory name like "Inception (2010)" → ("Inception", Some(2010))
pub fn parse_movie_dir(name: &str) -> (String, Option<i64>) {
    if let Some(idx) = name.rfind('(') {
        let year_part = name[idx + 1..].trim_end_matches(')').trim();
        if let Ok(year) = year_part.parse::<i64>() {
            let title = name[..idx].trim().to_string();
            return (title, Some(year));
        }
    }
    (name.to_string(), None)
}

/// Check if a directory contains Season subdirs
pub fn find_seasons(path: &Path) -> Vec<(i64, PathBuf)> {
    let mut seasons = Vec::new();
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return seasons,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if let Some(num) = parse_season_number(&name) {
                seasons.push((num, entry.path()));
            }
        }
    }
    seasons.sort_by_key(|(n, _)| *n);
    seasons
}

fn parse_season_number(name: &str) -> Option<i64> {
    let lower = name.to_lowercase();
    if lower.starts_with("season ") || lower.starts_with("season_") {
        lower[7..].trim().parse().ok()
    } else if lower.starts_with("s") && lower.len() <= 4 {
        lower[1..].trim().parse().ok()
    } else {
        None
    }
}

fn dir_size(path: &Path) -> i64 {
    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_file() {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if ft.is_dir() {
                total += dir_size(&entry.path()) as u64;
            }
        }
    }
    total as i64
}

pub async fn scan_directory(
    pool: &SqlitePool,
    media_dir: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut seen_paths = Vec::new();

    let entries = std::fs::read_dir(media_dir)?;
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }

        let dir_name = entry.file_name().to_string_lossy().to_string();
        let dir_path = entry.path();

        // Check if this is a TV show (has Season subdirs)
        let seasons = find_seasons(&dir_path);
        if !seasons.is_empty() {
            for (season_num, season_path) in &seasons {
                let path_str = season_path.to_string_lossy().to_string();
                let size = dir_size(season_path);
                media::upsert(
                    pool,
                    "tv_season",
                    &dir_name,
                    None,
                    Some(*season_num),
                    &path_str,
                    size,
                )
                .await?;
                seen_paths.push(path_str);
            }
        } else {
            // Treat as movie
            let (title, year) = parse_movie_dir(&dir_name);
            let path_str = dir_path.to_string_lossy().to_string();
            let size = dir_size(&dir_path);
            media::upsert(pool, "movie", &title, year, None, &path_str, size).await?;
            seen_paths.push(path_str);
        }
    }

    Ok(seen_paths)
}

pub async fn full_scan(
    pool: &SqlitePool,
    media_dirs: &[PathBuf],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut all_seen = Vec::new();

    for dir in media_dirs {
        tracing::info!("Scanning media directory: {}", dir.display());
        match scan_directory(pool, dir).await {
            Ok(paths) => all_seen.extend(paths),
            Err(e) => tracing::error!("Error scanning {}: {e}", dir.display()),
        }
    }

    media::mark_gone_except(pool, &all_seen).await?;
    tracing::info!("Scan complete, found {} media entries", all_seen.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_movie_dir_with_year() {
        let (title, year) = parse_movie_dir("Inception (2010)");
        assert_eq!(title, "Inception");
        assert_eq!(year, Some(2010));
    }

    #[test]
    fn parse_movie_dir_without_year() {
        let (title, year) = parse_movie_dir("SomeMovie");
        assert_eq!(title, "SomeMovie");
        assert_eq!(year, None);
    }

    #[test]
    fn parse_movie_dir_with_non_year_parens() {
        let (title, year) = parse_movie_dir("Movie (Extended Cut)");
        assert_eq!(title, "Movie (Extended Cut)");
        assert_eq!(year, None);
    }
}
