# Rewinder

Rewinder is a web application for shared Plex servers. Users mark movies and TV seasons they no longer need. Once every user has marked an item, Rewinder moves it to a trash directory. Users can also persist items to a permanent directory to protect them from deletion.

## Configuration

Copy the example config and adjust it:

```bash
cp rewinder.toml.example rewinder.toml
```

See `rewinder.toml.example` for all available options. The key settings are:

- `database_url` — SQLite database path
- `listen_addr` — address and port to listen on
- `media_dirs` — list of directories to scan for movies and TV shows
- `grace_period_days` — days to wait before cleaning trashed items
- `initial_admin_user` — username for the admin account created on first run
- `tmdb_api_key` — optional [TMDB](https://www.themoviedb.org/settings/api) API key for poster images

## Deployment

### Docker (recommended)

Docker builds and packages the application automatically — no Rust toolchain needed on the host.

1. Edit `deploy/docker-compose.yml` and replace `/path/to/media` with your actual media directory on the host. The right side of the `:` is the path inside the container:

   ```yaml
   volumes:
     - /path/to/media:/media
   ```

   The media volume needs read-write access so Rewinder can move files to trash and permanent directories.

2. Copy and edit the config file. The paths in `rewinder.toml` must match the **container-internal** mount paths (the right side from step 1), not the host paths:

   ```bash
   cp rewinder.toml.example rewinder.toml
   ```

   With the default volume mapping above, the example config works as-is (`media_dirs = ["/media/Movies", "/media/TV Shows"]` and `database_url = "sqlite:///data/rewinder.db?mode=rwc"`).

3. Start the container:

   ```bash
   docker compose -f deploy/docker-compose.yml up -d
   ```

The container exposes port 3000 by default. The database is stored in `deploy/data/` on the host, so you can back it up directly:

```bash
sqlite3 deploy/data/rewinder.db ".backup /path/to/backup.db"
```

### Standalone binary

Requires Rust 1.88 or later.

Build and install the binary and its supporting files:

```bash
cargo build --release
sudo install -Dm755 target/release/rewinder /usr/local/bin/rewinder
sudo install -d /usr/local/share/rewinder
sudo cp -r static templates /usr/local/share/rewinder/
sudo install -Dm644 rewinder.toml.example /etc/rewinder/rewinder.toml
```

Then edit `/etc/rewinder/rewinder.toml` and run from the install directory:

```bash
cd /usr/local/share/rewinder
rewinder --config /etc/rewinder/rewinder.toml
```

### systemd

After installing the standalone binary (see above), set up a systemd service. Service files are in `deploy/`:

```bash
# Basic service
sudo install -Dm644 deploy/rewinder.service /etc/systemd/system/rewinder.service

# Or: service that waits for a mount point (edit the mount path first)
sudo install -Dm644 deploy/rewinder-mount.service /etc/systemd/system/rewinder.service
```

If your media lives on a network mount or external drive, use `rewinder-mount.service` and edit the `RequiresMountsFor=` line to match your mount path. This ensures Rewinder only starts after the filesystem is available.

Then enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now rewinder
```

Check logs with:

```bash
journalctl -u rewinder -f
```
