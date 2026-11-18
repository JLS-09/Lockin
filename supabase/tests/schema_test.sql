-- Tests for the cloud schema, run by tools/test-cloud-schema.sh on a throwaway
-- local Postgres after supabase_stub.sql and the migrations. Every check raises
-- an error when it fails, which stops the script.

\set ON_ERROR_STOP on
\set QUIET on
-- Only the notices ("ok: …") and errors show, not the empty result of each check.
\o /dev/null

insert into auth.users (id, email) values
    ('00000000-0000-0000-0000-00000000000a', 'alice@example.test'),
    ('00000000-0000-0000-0000-00000000000b', 'bob@example.test');

-- Act as a logged-in user, as Supabase does for a request with their token.
create function pg_temp.login(who text) returns void language sql as $$
    select set_config('request.jwt.claim.sub',
        case who when 'alice' then '00000000-0000-0000-0000-00000000000a'
                 when 'bob'   then '00000000-0000-0000-0000-00000000000b' end,
        false);
$$;

create function pg_temp.check(ok boolean, what text) returns void language plpgsql as $$
begin
    if not coalesce(ok, false) then
        raise exception 'FAILED: %', what;
    end if;
    raise notice 'ok: %', what;
end;
$$;
grant execute on function pg_temp.login(text) to authenticated, anon;
grant execute on function pg_temp.check(boolean, text) to authenticated, anon, supabase_auth_admin;


-- ---------------------------------------------------------------------------
-- Rows belong to whoever added them
-- ---------------------------------------------------------------------------

select pg_temp.login('alice');
set role authenticated;

insert into public.course (id, name, code, updated_at, synced_at)
values ('c1', 'Signalen en systemen', 'E702010A', '2099-11-09T10:00:00Z', '2000-01-01');

select pg_temp.check(
    (select user_id = '00000000-0000-0000-0000-00000000000a' from public.course where id = 'c1'),
    'a new row gets the logged-in user');
select pg_temp.check(
    (select synced_at > now() - interval '1 minute' from public.course where id = 'c1'),
    'synced_at comes from the server, not from the app');

-- The sync engine's upsert: insert, or update the row with the same id.
insert into public.course (id, name, updated_at, deleted)
values ('c1', 'Signalen en systemen (renamed)', '2099-11-09T11:00:00Z', 0)
on conflict (user_id, id) do update
    set name = excluded.name, updated_at = excluded.updated_at, deleted = excluded.deleted;
select pg_temp.check(
    (select name = 'Signalen en systemen (renamed)' from public.course where id = 'c1'),
    'an upsert updates your own row');

-- A soft delete is an update.
update public.course set deleted = 1, updated_at = '2099-11-09T12:00:00Z' where id = 'c1';
select pg_temp.check((select deleted = 1 from public.course where id = 'c1'),
    'a soft delete works');

do $$
begin
    delete from public.course where id = 'c1';
    raise exception 'FAILED: a hard delete went through';
exception when insufficient_privilege then
    raise notice 'ok: no hard deletes from the app';
end;
$$;

reset role;


select pg_temp.login('bob');
set role authenticated;

select pg_temp.check((select count(*) = 0 from public.course), 'bob sees none of alice''s rows');

update public.course set name = 'hacked' where id = 'c1';
reset role;
select pg_temp.check(
    (select name = 'Signalen en systemen (renamed)' from public.course where id = 'c1'),
    'bob can''t change alice''s row');
set role authenticated;

do $$
begin
    insert into public.course (user_id, id, name, updated_at)
    values ('00000000-0000-0000-0000-00000000000a', 'c2', 'planted', '2099-11-09T10:00:00Z');
    raise exception 'FAILED: bob added a row for alice';
exception when insufficient_privilege then
    raise notice 'ok: bob can''t add rows for alice';
end;
$$;

do $$
begin
    update public.course set user_id = '00000000-0000-0000-0000-00000000000a';
    -- Bob has no rows yet, so nothing happened; try with one of his own.
    insert into public.course (id, name, updated_at) values ('b1', 'mine', '2099-11-09T10:00:00Z');
    update public.course set user_id = '00000000-0000-0000-0000-00000000000a' where id = 'b1';
    raise exception 'FAILED: bob gave a row to alice';
exception when insufficient_privilege then
    raise notice 'ok: a row can''t be moved to another user';
end;
$$;

-- The same id for two people is fine: ids are only unique per user.
insert into public.course (id, name, updated_at) values ('c1', 'Bob''s own c1', '2099-11-09T10:00:00Z');
select pg_temp.check((select count(*) = 1 from public.course where id = 'c1'),
    'bob can use an id alice also has');

reset role;


-- ---------------------------------------------------------------------------
-- The newer change wins (20261118190000_keep_newer.sql)
-- ---------------------------------------------------------------------------

select pg_temp.login('alice');
set role authenticated;

insert into public.course (id, name, updated_at) values ('k1', 'first', '2099-11-09T10:00:00Z');

-- An older change arriving late: ignored, also by an upsert.
insert into public.course (id, name, updated_at) values ('k1', 'older', '2099-11-09T09:00:00Z')
on conflict (user_id, id) do update set name = excluded.name, updated_at = excluded.updated_at;
select pg_temp.check((select name = 'first' from public.course where id = 'k1'),
    'an older change doesn''t overwrite a newer one');

-- A newer one goes through; the same time again too (sending twice is harmless).
insert into public.course (id, name, updated_at) values ('k1', 'newer', '2099-11-09T11:00:00Z')
on conflict (user_id, id) do update set name = excluded.name, updated_at = excluded.updated_at;
select pg_temp.check((select name = 'newer' from public.course where id = 'k1'),
    'a newer change goes through');
update public.course set name = 'same time' where id = 'k1';
select pg_temp.check((select name = 'same time' from public.course where id = 'k1'),
    'a change with the same time goes through');

reset role;


-- ---------------------------------------------------------------------------
-- Not logged in: nothing
-- ---------------------------------------------------------------------------

set role anon;
do $$
begin
    perform * from public.course;
    raise exception 'FAILED: anon could read course';
exception when insufficient_privilege then
    raise notice 'ok: anon can''t read the synced tables';
end;
$$;
do $$
begin
    perform * from public.invite_code;
    raise exception 'FAILED: anon could read invite codes';
exception when insufficient_privilege then
    raise notice 'ok: anon can''t read invite codes';
end;
$$;
reset role;

set role authenticated;
do $$
begin
    perform * from public.invite_code;
    raise exception 'FAILED: a user could read invite codes';
exception when insufficient_privilege then
    raise notice 'ok: users can''t read invite codes';
end;
$$;
reset role;


-- Every synced table has row-level security on.
select pg_temp.check(
    (select bool_and(c.relrowsecurity) and count(*) = 17
     from pg_class c join pg_namespace n on n.oid = c.relnamespace
     where n.nspname = 'public' and c.relkind = 'r'),
    'row-level security is on for all 17 tables');


-- ---------------------------------------------------------------------------
-- Invite codes
-- ---------------------------------------------------------------------------

select pg_temp.check(
    (select bool_and(code ~ '^[ABCDEFGHJKMNPQRSTUVWXYZ2-9]{8}$')
     from (select public.lockin_new_invite_code() as code from generate_series(1, 200)) as codes),
    'new codes are 8 characters without look-alikes');

insert into public.invite_code (code, note) values ('K7QM2XWD', 'Carol');
insert into public.invite_code (code, note, expires_at) values ('OLDCODE2', 'Dave', now() - interval '1 day');
insert into public.invite_code (note) values ('Eve');
select pg_temp.check((select length(code) = 8 from public.invite_code where note = 'Eve'),
    'a code fills itself in');

-- The hook, called the way Supabase Auth calls it.
create function pg_temp.signup(code text, user_id text) returns jsonb language sql as $$
    select public.lockin_check_invite(jsonb_build_object(
        'metadata', jsonb_build_object('name', 'before-user-created'),
        'user', jsonb_build_object(
            'id', user_id,
            'email', 'carol@example.test',
            'user_metadata', case when code is null then '{}'::jsonb
                                  else jsonb_build_object('invite_code', code) end)));
$$;
grant execute on function pg_temp.signup(text, text) to supabase_auth_admin;

set role supabase_auth_admin;

select pg_temp.check(
    pg_temp.signup('k7qm-2xwd ', '00000000-0000-0000-0000-00000000000c') = '{}'::jsonb,
    'a valid code (any case, with a dash) lets the signup through');
select pg_temp.check(
    pg_temp.signup('K7QM2XWD', '00000000-0000-0000-0000-00000000000d') ? 'error',
    'a used code is refused');
select pg_temp.check(
    pg_temp.signup('OLDCODE2', '00000000-0000-0000-0000-00000000000d') ? 'error',
    'an expired code is refused');
select pg_temp.check(
    pg_temp.signup('NOPENOPE', '00000000-0000-0000-0000-00000000000d') ? 'error',
    'an unknown code is refused');
select pg_temp.check(
    pg_temp.signup(null, '00000000-0000-0000-0000-00000000000d') ? 'error',
    'a signup without a code is refused');
select pg_temp.check(
    (pg_temp.signup('nope', '00000000-0000-0000-0000-00000000000d')
        #>> '{error,http_code}') = '403',
    'a refusal is a 403');

reset role;

select pg_temp.check(
    (select used_by = '00000000-0000-0000-0000-00000000000c' and used_at is not null
     from public.invite_code where code = 'K7QM2XWD'),
    'a used code remembers who used it');

set role authenticated;
do $$
begin
    perform public.lockin_check_invite('{}'::jsonb);
    raise exception 'FAILED: a user could call the hook';
exception when insufficient_privilege then
    raise notice 'ok: only Supabase Auth can call the hook';
end;
$$;
reset role;

\echo 'All cloud schema tests passed.'
