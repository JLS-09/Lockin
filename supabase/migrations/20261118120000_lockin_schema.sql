-- Lockin's cloud database (phase 5, step 1).
--
-- One Supabase project shared by everyone who uses Lockin: each person signs
-- up with an invite code and only ever sees their own rows.
--
-- How to run it: paste this whole file into the SQL Editor of the Supabase
-- project and press Run (see supabase/README.md). Like the SQLite migrations,
-- a file that has been run is never edited: a change is a new file.
--
-- The tables mirror the local SQLite tables that sync (crates/lockin-core/
-- migrations), with the same column names and types, so a row goes up and
-- comes back unchanged:
--   - text stays text (ids are UUIDs as text, times RFC 3339 text in UTC),
--   - SQLite INTEGER (also the 0/1 flags like done and deleted) is integer.
-- Each table adds two columns:
--   - user_id:   whose row it is, filled in from the login (auth.uid()).
--   - synced_at: when the row last reached the cloud, set here by a trigger
--                with the server's clock, so each computer can fetch "what's
--                new since I last looked" without trusting other clocks.
-- There are no CHECKs or foreign keys between these tables: the app checks
-- its data locally, and rows can arrive in any order.
--
-- Not synced: `setting` and `notification_sent` stay on each computer.
-- `calendar_source.url` holds the feed link encrypted (it contains a personal
-- key); the app encrypts it before it leaves the computer.


-- ---------------------------------------------------------------------------
-- The synced tables
-- ---------------------------------------------------------------------------

create table public.course (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    name        text not null,
    code        text,
    color       text,
    semester    text,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.calendar_source (
    user_id          uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id               text not null,
    kind             text not null,
    url              text not null,   -- encrypted by the app, never the plain link
    last_fetched_at  text,            -- not synced: each computer keeps its own
    updated_at       text not null,
    deleted          integer not null default 0,
    synced_at        timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.filter_rule (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    source_id   text,
    field       text not null,
    pattern     text not null,
    action      text not null,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.history_import (
    user_id      uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id           text not null,
    name         text,
    imported_at  text not null,
    updated_at   text not null,
    deleted      integer not null default 0,
    synced_at    timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.class_block (
    user_id           uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id                text not null,
    source_id         text,
    ical_uid          text,
    course_id         text,
    title             text not null,
    location          text,
    description       text,
    start_at          text not null,
    end_at            text not null,
    ignored_by_rule   integer not null default 0,
    ignored_override  integer,
    status            text not null default 'normal',
    kind              text,
    moved_from        text,
    change_seen       integer not null default 0,
    import_id         text,
    updated_at        text not null,
    deleted           integer not null default 0,
    synced_at         timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.activity_template (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    course_id   text not null,
    kind        text,
    title       text not null,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.activity (
    user_id      uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id           text not null,
    block_id     text,
    template_id  text,
    course_id    text,
    title        text not null,
    week         text,
    done         integer not null default 0,
    done_at      text,
    updated_at   text not null,
    deleted      integer not null default 0,
    synced_at    timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.deadline (
    user_id      uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id           text not null,
    source_id    text,
    ical_uid     text,
    course_id    text,
    title        text not null,
    description  text,
    due_at       text not null,
    done         integer not null default 0,
    done_at      text,
    status       text not null default 'normal',
    moved_from   text,
    change_seen  integer not null default 0,
    url          text,
    updated_at   text not null,
    deleted      integer not null default 0,
    synced_at    timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.block_profile (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    name        text not null,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.block_app (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    profile_id  text not null,
    app_id      text not null,
    name        text not null,
    exec_names  text not null,
    flatpak_id  text,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.block_site (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    profile_id  text not null,
    domain      text not null,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.session_series (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    course_id   text,
    title       text,
    until_date  text not null,
    profile_id  text,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.study_session (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    series_id   text,
    course_id   text,
    title       text,
    start_at    text not null,
    end_at      text not null,
    status      text not null default 'planned',
    profile_id  text,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.session_event (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    session_id  text not null,
    kind        text not null,
    at          text not null,
    detail      text,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.app_closed (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    session_id  text not null,
    app_id      text not null,
    name        text not null,
    at          text not null,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);

create table public.points_ledger (
    user_id     uuid not null default auth.uid() references auth.users (id) on delete cascade,
    id          text not null,
    at          text not null,
    points      integer not null,
    reason      text not null,
    session_id  text,
    updated_at  text not null,
    deleted     integer not null default 0,
    synced_at   timestamptz not null default now(),
    primary key (user_id, id)
);


-- synced_at always comes from the server's clock, whatever the app sends.
create function public.lockin_set_synced_at()
returns trigger
language plpgsql
set search_path = ''
as $$
begin
    new.synced_at := clock_timestamp();
    return new;
end;
$$;


-- The same rules for every synced table:
--   - row-level security: you read, add and change only your own rows;
--   - no hard deletes from the app (rows are soft-deleted with `deleted`);
--     deleting your account removes your rows (on delete cascade);
--   - nothing at all for anon (not logged in);
--   - the synced_at trigger, and an index to fetch what's new.
do $$
declare
    t text;
begin
    foreach t in array array[
        'course', 'calendar_source', 'filter_rule', 'history_import', 'class_block',
        'activity_template', 'activity', 'deadline', 'block_profile', 'block_app',
        'block_site', 'session_series', 'study_session', 'session_event',
        'app_closed', 'points_ledger'
    ] loop
        execute format('alter table public.%I enable row level security', t);
        execute format('revoke all on public.%I from anon, authenticated, public', t);
        execute format('grant select, insert, update on public.%I to authenticated', t);

        execute format(
            'create policy "read own rows" on public.%I for select to authenticated
             using (user_id = (select auth.uid()))', t);
        execute format(
            'create policy "add own rows" on public.%I for insert to authenticated
             with check (user_id = (select auth.uid()))', t);
        execute format(
            'create policy "change own rows" on public.%I for update to authenticated
             using (user_id = (select auth.uid()))
             with check (user_id = (select auth.uid()))', t);

        execute format(
            'create trigger set_synced_at before insert or update on public.%I
             for each row execute function public.lockin_set_synced_at()', t);
        execute format('create index %I on public.%I (user_id, synced_at)', t || '_pull_idx', t);
    end loop;
end;
$$;


-- ---------------------------------------------------------------------------
-- Invite codes: only people with a code can make an account
-- ---------------------------------------------------------------------------
--
-- Make one in the SQL Editor (the note is just for you):
--   insert into public.invite_code (note) values ('Arne') returning code;
-- or add a row in the Table Editor: the code fills itself in.
--
-- The app sends the code with the signup (as user metadata `invite_code`);
-- the "Before User Created" auth hook below checks it and marks it used.
-- Each code works once and expires after 30 days.

-- A random code of 8 characters without look-alikes (no 0/O, 1/I/L), like
-- "K7QM2XWD". The app shows it as "K7QM-2XWD"; dashes and spaces are ignored.
create function public.lockin_new_invite_code()
returns text
language sql
volatile
set search_path = ''
as $$
    select string_agg(
        substr('ABCDEFGHJKMNPQRSTUVWXYZ23456789', get_byte(bytes, i) % 31 + 1, 1),
        '' order by i)
    from (select extensions.gen_random_bytes(8) as bytes) as random,
         generate_series(0, 7) as i;
$$;

create table public.invite_code (
    code        text primary key default public.lockin_new_invite_code(),
    note        text,                                    -- who it's for
    created_at  timestamptz not null default now(),
    expires_at  timestamptz default now() + interval '30 days',   -- null = never
    used_at     timestamptz,
    used_by     uuid      -- the new user's id; no foreign key, as the hook runs
                          -- just before the user exists
);

-- Nobody reaches this table through the app's key; only the auth hook (and
-- you, in the dashboard) can read and use codes.
alter table public.invite_code enable row level security;
revoke all on public.invite_code from anon, authenticated, public;
grant usage on schema public to supabase_auth_admin;
grant select, update on public.invite_code to supabase_auth_admin;
create policy "auth hook uses codes" on public.invite_code
    for all to supabase_auth_admin using (true) with check (true);


-- The "Before User Created" auth hook. Turn it on in the dashboard:
-- Authentication → Hooks → Before User Created → Postgres function
-- public.lockin_check_invite. It runs for every new account, also ones you
-- make in the dashboard, so make an invite for yourself first.
create function public.lockin_check_invite(event jsonb)
returns jsonb
language plpgsql
set search_path = ''
as $$
declare
    typed  text := event #>> '{user,user_metadata,invite_code}';
    wanted text := regexp_replace(upper(coalesce(typed, '')), '[^A-Z0-9]', '', 'g');
    used   text;
begin
    update public.invite_code as invite
    set used_at = now(),
        used_by = (event #>> '{user,id}')::uuid
    where invite.code = wanted
      and invite.used_at is null
      and (invite.expires_at is null or invite.expires_at > now())
    returning invite.code into used;

    if used is null then
        -- The app recognises this message and shows its own text.
        return jsonb_build_object('error', jsonb_build_object(
            'http_code', 403,
            'message', 'lockin: invalid invite code'));
    end if;
    return '{}'::jsonb;
end;
$$;

grant execute on function public.lockin_check_invite(jsonb) to supabase_auth_admin;
revoke execute on function public.lockin_check_invite(jsonb) from anon, authenticated, public;
revoke execute on function public.lockin_new_invite_code() from anon, authenticated, public;
grant execute on function public.lockin_new_invite_code() to supabase_auth_admin;
