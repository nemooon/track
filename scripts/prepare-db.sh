#!/bin/sh
# Copy the real Track DB into ~/.track-gpui for the GPUI prototype.
# Never writes to ~/.track. Use --force to overwrite an existing copy.
set -eu
SRC="$HOME/.track"
DST="$HOME/.track-gpui"
if [ -f "$DST/track.db" ] && [ "${1:-}" != "--force" ]; then
  echo "$DST/track.db already exists (use --force to overwrite)" >&2
  exit 1
fi
mkdir -p "$DST"
rm -f "$DST/track.db" "$DST/track.db-wal" "$DST/track.db-shm"
# The GPUI app seeds gpui-config.json from config.json on its next start.
rm -f "$DST/gpui-config.json"
# .backup gives a consistent snapshot including WAL contents
sqlite3 "$SRC/track.db" ".backup '$DST/track.db'"
if [ -f "$SRC/config.json" ]; then
  # Point exportDir (auto-backup target, rotated by backupKeep) at the copy,
  # so the prototype server never writes or prunes the real backups.
  python3 - "$SRC/config.json" "$DST/config.json" "$DST/exports" <<'PY'
import json, sys
src, dst, exports = sys.argv[1:]
cfg = json.load(open(src))
cfg["exportDir"] = exports
json.dump(cfg, open(dst, "w"), ensure_ascii=False, indent=2)
PY
fi
echo "copied to $DST"
