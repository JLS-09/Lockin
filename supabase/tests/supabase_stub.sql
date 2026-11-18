-- A small stand-in for what a Supabase project already has, so the cloud
-- schema can be tested on a plain local Postgres (tools/test-cloud-schema.sh).
-- Never run this on the real project.

create role anon nologin;
create role authenticated nologin;
create role service_role nologin bypassrls;
create role supabase_auth_admin nologin;

create schema auth;
create schema extensions;
create extension pgcrypto schema extensions;

grant usage on schema auth to anon, authenticated, supabase_auth_admin;
grant usage on schema extensions to anon, authenticated, supabase_auth_admin;
grant usage on schema public to anon, authenticated;

create table auth.users (
    id     uuid primary key,
    email  text
);

-- Like Supabase: the logged-in user's id from the request's JWT claims.
create function auth.uid()
returns uuid
language sql
stable
as $$
    select nullif(current_setting('request.jwt.claim.sub', true), '')::uuid
$$;
grant execute on function auth.uid() to anon, authenticated, supabase_auth_admin;
