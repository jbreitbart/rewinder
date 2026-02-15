CREATE TABLE IF NOT EXISTS persistent_media (
    media_id     INTEGER PRIMARY KEY REFERENCES media(id) ON DELETE CASCADE,
    user_id      INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    persisted_at TEXT NOT NULL DEFAULT (datetime('now'))
);

PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS media_new (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    media_type TEXT NOT NULL CHECK(media_type IN ('movie', 'tv_season')),
    title      TEXT NOT NULL,
    year       INTEGER,
    season     INTEGER,
    path       TEXT NOT NULL UNIQUE,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    status     TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'trashed', 'gone', 'permanent')),
    trashed_at TEXT,
    first_seen TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen  TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO media_new (
    id, media_type, title, year, season, path, size_bytes, status, trashed_at, first_seen, last_seen
)
SELECT
    id, media_type, title, year, season, path, size_bytes, status, trashed_at, first_seen, last_seen
FROM media;

DROP TABLE media;
ALTER TABLE media_new RENAME TO media;

PRAGMA foreign_keys = ON;
