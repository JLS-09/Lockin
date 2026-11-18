# Lockin's cloud (Supabase)

Lockin keeps everything in a local SQLite database and works offline. From phase 5 on it
also syncs to one Supabase project that everyone using Lockin shares: each person logs in
with their own account and only ever sees their own data. Nobody but the project's owner
has to set anything up.

- `migrations/`: the cloud schema, one SQL file per change, run in order.
- `tests/`: a stand-in for Supabase and the schema's tests, for
  `tools/test-cloud-schema.sh`.

## How it's protected

- The app has the project's URL and its **publishable key** built in. Both are public by
  design: they only say which project to talk to. Never put the **secret key** (or the old
  `service_role` key) in the app or the repo.
- Every synced table has **row-level security**: a logged-in user reads, adds and changes
  only rows with their own `user_id`. Without a login nothing is readable at all.
- **Invite codes:** a new account needs a code you made. The "Before User Created" auth hook
  (`public.lockin_check_invite`) refuses a signup without a valid, unused code, so a
  stranger with the app can't make an account and use the project's free quota.
- Feed links hold a personal key, so the app encrypts them before they're uploaded.

## Setting up the project (once)

1. On [supabase.com](https://supabase.com), sign in and make a **new project** on the free
   plan: name `Lockin`, a region in the EU close by (e.g. West EU (Paris) or Central EU
   (Frankfurt)), and a generated database password. Keep that password in your password
   manager; Lockin itself never needs it. Leave the Data API on.
2. **SQL Editor** → New query: paste all of
   `migrations/20261118120000_lockin_schema.sql` and press **Run**. It should end with
   "Success. No rows returned".
3. **Authentication** → Sign In / Providers → **Email**: keep it enabled, turn **Confirm
   email off** and save. Keep "Allow new users to sign up" on: the invite hook does the
   gatekeeping. (The free plan's own mail service only sends to the project's team, so
   with confirmation on, your friends would never get the email.)
4. **Authentication** → **Hooks** → add the **Before User Created** hook: type Postgres,
   schema `public`, function `lockin_check_invite`.
5. Make an invite for yourself (the hook also checks accounts you make yourself). In the
   SQL Editor:

   ```sql
   insert into public.invite_code (note) values ('Jules') returning code;
   ```

6. The **Project URL** and the **publishable key** (`sb_publishable_…`, under Project
   Settings → API Keys) are built into the app, in `crates/lockin-core/src/account.rs`.

## Inviting someone

```sql
insert into public.invite_code (note) values ('Arne') returning code;
```

Send them the code (`K7QM2XWD`; the app accepts it with or without a dash, in any case),
or the link `lockin://invite/K7QM-2XWD`: once Lockin is installed, it opens Settings →
Account with the code filled in. (Chat apps don't always make `lockin://` links
clickable; the code always works.) They choose "Create account" there with their email
and a password. A code works once and expires after 30 days; `expires_at = null` makes
one that never expires. A signup that fails (a taken email, a short password) doesn't use
the code up. `select * from public.invite_code;` shows which codes were used and by whom (the
user id, as under Authentication → Users).

## A forgotten password

The free plan's mail service only sends to the project's team, so a reset email never
reaches a friend. Set a new password for them in the SQL Editor and tell them:

```sql
update auth.users
set encrypted_password = extensions.crypt('their-new-password', extensions.gen_salt('bf'))
where email = 'arne@example.com';
```

Lockin encrypts synced feed links with a key from the password, so after a reset they
paste their TimeEdit and Ufora links again in Settings; everything else stays.

## Changing the schema

Like the SQLite migrations: never edit a file that has been run on the project. Add a new
file (`YYYYMMDDHHMMSS_what.sql`), test it locally, then run it in the SQL Editor.

```bash
tools/test-cloud-schema.sh   # needs Postgres installed (sudo pacman -S postgresql)
tools/test-cloud-auth.sh     # the same with a real Supabase Auth: signing up, logging in
```

`test-cloud-schema.sh` starts a throwaway Postgres in a temporary folder, loads the
stand-in for Supabase, runs every migration and the tests in `tests/schema_test.sql`.
`test-cloud-auth.sh` runs Supabase's own Auth server (GoTrue, downloaded once from
github.com/supabase/auth into `~/.cache/lockin/`) on such a Postgres, with the migrations
and the invite hook, and runs `crates/lockin-core/tests/account.rs` against it. Neither
touches the real project.

## Good to know

- A free project **pauses after a week without any activity**. Lockin keeps working
  offline meanwhile; restore the project in the dashboard and everything syncs again.
- Deleting a user under Authentication → Users deletes all their synced rows too.
