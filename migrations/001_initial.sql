CREATE TABLE IF NOT EXISTS users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT NOT NULL UNIQUE,
    password_hash TEXT,
    is_admin      INTEGER NOT NULL DEFAULT 0,
    invite_token  TEXT UNIQUE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS sessions (
    token      TEXT PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS media (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    media_type TEXT NOT NULL CHECK(media_type IN ('movie', 'tv_season')),
    title      TEXT NOT NULL,
    year       INTEGER,
    season     INTEGER,
    path       TEXT NOT NULL UNIQUE,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    status     TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'trashed', 'gone')),
    trashed_at TEXT,
    first_seen TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS marks (
    user_id   INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    media_id  INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    marked_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (user_id, media_id)
);
