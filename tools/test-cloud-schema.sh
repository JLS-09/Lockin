#!/usr/bin/env bash
# Test the cloud schema (supabase/migrations) on a throwaway local Postgres:
# a stand-in for Supabase (supabase/tests/supabase_stub.sql), every migration
# in order, then supabase/tests/schema_test.sql. Nothing touches the real
# Supabase project. Run it after changing or adding a cloud migration.
#
# Needs Postgres installed (CachyOS: sudo pacman -S postgresql); it doesn't
# have to be running, this script starts its own in a temporary folder.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"

# initdb and pg_ctl: on the PATH (Arch) or in Postgres' own bin folder (Debian).
bin=""
if ! command -v initdb >/dev/null 2>&1; then
    if command -v pg_config >/dev/null 2>&1 && [ -x "$(pg_config --bindir)/initdb" ]; then
        bin="$(pg_config --bindir)/"
    else
        bin="$(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -n 1)/"
    fi
fi
if [ ! -x "${bin}initdb" ] && ! command -v "${bin}initdb" >/dev/null 2>&1; then
    echo "Postgres isn't installed (initdb not found)." >&2
    exit 1
fi

folder="$(mktemp -d)"
port=54329
cleanup() {
    "${bin}pg_ctl" -D "$folder/data" -m immediate stop >/dev/null 2>&1 || true
    rm -rf "$folder"
}
trap cleanup EXIT

"${bin}initdb" -D "$folder/data" -U postgres -A trust >/dev/null
"${bin}pg_ctl" -D "$folder/data" -o "-p $port -k $folder -c listen_addresses=''" \
    -l "$folder/log" -w start >/dev/null

run() {
    psql -h "$folder" -p "$port" -U postgres -d postgres -X -q -v ON_ERROR_STOP=1 "$@"
}

run -f "$repo/supabase/tests/supabase_stub.sql"
for migration in "$repo"/supabase/migrations/*.sql; do
    echo "Migration $(basename "$migration")"
    run -f "$migration"
done
run -f "$repo/supabase/tests/schema_test.sql" 2>&1 | sed 's/^psql:[^ ]* NOTICE:  /  /'
