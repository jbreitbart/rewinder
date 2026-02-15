#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="$SCRIPT_DIR/media"
RESET=false

usage() {
    cat <<'EOF'
Usage: generate_media_fixtures.sh [--target <dir>] [--reset]

Create a realistic media fixture tree for local testing/debugging.

Options:
  --target <dir>  Root media fixture directory (default: tests/fixtures/media)
  --reset         Remove existing fixture tree before re-creating it
  -h, --help      Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --target)
            TARGET_DIR="$2"
            shift 2
            ;;
        --reset)
            RESET=true
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage
            exit 1
            ;;
    esac
done

write_fake_media_file() {
    local path="$1"
    local label="$2"
    mkdir -p "$(dirname "$path")"
    printf '%s\n' "$label" > "$path"
}

if [[ "$RESET" == "true" ]]; then
    rm -rf \
        "$TARGET_DIR/Movies" \
        "$TARGET_DIR/TV Shows" \
        "$TARGET_DIR/Movies_trash" \
        "$TARGET_DIR/TV Shows_trash" \
        "$TARGET_DIR/Movies_permanent" \
        "$TARGET_DIR/TV Shows_permanent"
fi

mkdir -p \
    "$TARGET_DIR/Movies" \
    "$TARGET_DIR/TV Shows" \
    "$TARGET_DIR/Movies_trash" \
    "$TARGET_DIR/TV Shows_trash" \
    "$TARGET_DIR/Movies_permanent" \
    "$TARGET_DIR/TV Shows_permanent"

write_fake_media_file \
    "$TARGET_DIR/Movies/Interstellar (2014)/Interstellar (2014).mkv" \
    "fixture: Interstellar (2014)"
write_fake_media_file \
    "$TARGET_DIR/Movies/Mad Max Fury Road (2015)/Mad Max Fury Road (2015).mkv" \
    "fixture: Mad Max Fury Road (2015)"
write_fake_media_file \
    "$TARGET_DIR/Movies/Spider-Man Into the Spider-Verse (2018)/Spider-Man Into the Spider-Verse (2018).mkv" \
    "fixture: Spider-Man Into the Spider-Verse (2018)"
write_fake_media_file \
    "$TARGET_DIR/Movies/Dune Part Two (2024)/Dune Part Two (2024).mkv" \
    "fixture: Dune Part Two (2024)"

write_fake_media_file \
    "$TARGET_DIR/TV Shows/Breaking Bad/Season 1/Breaking.Bad.S01E01.mkv" \
    "fixture: Breaking Bad S01E01"
write_fake_media_file \
    "$TARGET_DIR/TV Shows/Breaking Bad/Season 2/Breaking.Bad.S02E01.mkv" \
    "fixture: Breaking Bad S02E01"
write_fake_media_file \
    "$TARGET_DIR/TV Shows/The Bear/Season 1/The.Bear.S01E01.mkv" \
    "fixture: The Bear S01E01"
write_fake_media_file \
    "$TARGET_DIR/TV Shows/The Bear/Season 2/The.Bear.S02E01.mkv" \
    "fixture: The Bear S02E01"
write_fake_media_file \
    "$TARGET_DIR/TV Shows/Stranger Things/Season 1/Stranger.Things.S01E01.mkv" \
    "fixture: Stranger Things S01E01"
write_fake_media_file \
    "$TARGET_DIR/TV Shows/Severance/Season 1/Severance.S01E01.mkv" \
    "fixture: Severance S01E01"

echo "Fixture tree generated at: $TARGET_DIR"
