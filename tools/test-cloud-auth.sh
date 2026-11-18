#!/usr/bin/env bash
# Test making accounts, logging in and out (crates/lockin-core/tests/account.rs)
# against a real Supabase Auth (GoTrue) on this computer: a throwaway Postgres
# with Supabase's roles, GoTrue's own migrations, Lockin's cloud schema
# (supabase/migrations) and the invite hook, like the real project. Nothing
# touches the real project.
#
# GoTrue is downloaded once from github.com/supabase/auth into ~/.cache/lockin/.
# Needs Postgres installed (CachyOS: sudo pacman -S postgresql), not running.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
gotrue_version=v2.198.0
pg_port=54331
auth_port=9998

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

# GoTrue, once.
cache="${XDG_CACHE_HOME:-$HOME/.cache}/lockin/gotrue-$gotrue_version"
if [ ! -x "$cache/auth" ]; then
    echo "Downloading Supabase Auth $gotrue_version"
    mkdir -p "$cache"
    curl -fsSL "https://github.com/supabase/auth/releases/download/$gotrue_version/auth-$gotrue_version-x86.tar.gz" \
        | tar -xz -C "$cache"
fi

folder="$(mktemp -d)"
gotrue_pid=""
cleanup() {
    [ -n "$gotrue_pid" ] && kill "$gotrue_pid" 2>/dev/null || true
    "${bin}pg_ctl" -D "$folder/data" -m immediate stop >/dev/null 2>&1 || true
    rm -rf "$folder"
}
trap cleanup EXIT

"${bin}initdb" -D "$folder/data" -U postgres -A trust >/dev/null
"${bin}pg_ctl" -D "$folder/data" -o "-p $pg_port -k $folder -c listen_addresses=127.0.0.1" \
    -l "$folder/postgres.log" -w start >/dev/null

database="postgres://postgres@127.0.0.1:$pg_port/postgres"
run() {
    psql "$database" -X -q -v ON_ERROR_STOP=1 "$@"
}

# What a Supabase project has before GoTrue and Lockin's schema come in.
run <<'SQL'
create role anon nologin;
create role authenticated nologin;
create role service_role nologin bypassrls;
create role supabase_auth_admin login password 'auth' noinherit createrole;
create schema auth authorization supabase_auth_admin;
create schema extensions;
create extension pgcrypto schema extensions;
grant usage on schema extensions to anon, authenticated, supabase_auth_admin;
grant usage on schema public to anon, authenticated;
grant create on database postgres to supabase_auth_admin;
alter role supabase_auth_admin set search_path = auth;
SQL

export GOTRUE_DB_DRIVER=postgres
export GOTRUE_DB_DATABASE_URL="postgres://supabase_auth_admin:auth@127.0.0.1:$pg_port/postgres?sslmode=disable"
export GOTRUE_DB_NAMESPACE=auth
export GOTRUE_DB_MIGRATIONS_PATH="$cache/migrations"
export GOTRUE_JWT_SECRET=lockin-local-test-secret-of-32-characters
export GOTRUE_JWT_EXP=3600
export GOTRUE_JWT_AUD=authenticated
export GOTRUE_JWT_DEFAULT_GROUP_NAME=authenticated
export GOTRUE_JWT_ADMIN_ROLES=service_role
export API_EXTERNAL_URL="http://127.0.0.1:$auth_port"
export GOTRUE_SITE_URL="http://127.0.0.1:$auth_port"
export GOTRUE_API_HOST=127.0.0.1
export PORT=$auth_port
# As in supabase/README.md: no email confirmation, and the invite hook.
export GOTRUE_MAILER_AUTOCONFIRM=true
export GOTRUE_EXTERNAL_EMAIL_ENABLED=true
export GOTRUE_HOOK_BEFORE_USER_CREATED_ENABLED=true
export GOTRUE_HOOK_BEFORE_USER_CREATED_URI="pg-functions://postgres/public/lockin_check_invite"
# The tests make several accounts quickly.
export GOTRUE_RATE_LIMIT_EMAIL_SENT=1000
export GOTRUE_RATE_LIMIT_TOKEN_REFRESH=1000
export GOTRUE_LOG_LEVEL=error

"$cache/auth" migrate >/dev/null
for migration in "$repo"/supabase/migrations/*.sql; do
    run -f "$migration"
done

"$cache/auth" serve >"$folder/auth.log" 2>&1 &
gotrue_pid=$!
for _ in $(seq 50); do
    curl -fs "http://127.0.0.1:$auth_port/health" >/dev/null 2>&1 && break
    sleep 0.2
done

export LOCKIN_TEST_AUTH_URL="http://127.0.0.1:$auth_port"
export LOCKIN_TEST_DATABASE_URL="$database"
export NO_PROXY="127.0.0.1,localhost${NO_PROXY:+,$NO_PROXY}" no_proxy="127.0.0.1,localhost${no_proxy:+,$no_proxy}"

if [ "${1:-}" = "--serve" ]; then
    # For trying the app against it: LOCKIN_AUTH_URL=http://127.0.0.1:9998
    echo "Supabase Auth on $LOCKIN_TEST_AUTH_URL, database $database. Ctrl+C stops."
    echo "Invite code: $(psql "$database" -X -qAt -c "insert into public.invite_code (note) values ('try') returning code")"
    wait "$gotrue_pid"
else
    cd "$repo"
    cargo test -p lockin-core --test account -- --test-threads=1
fi
