-- Block profiles (phase 4 step 4): your own lists of apps to close during a
-- study session, e.g. "Strict" and "Light". Lockin ships none: you pick the apps.
-- Websites join the profiles in step 5.

CREATE TABLE block_profile (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

-- One app on a profile. What to look for is copied from the app's .desktop
-- file when you add it, so the daemon never has to read those files.
CREATE TABLE block_app (
    id          TEXT PRIMARY KEY,
    profile_id  TEXT NOT NULL REFERENCES block_profile(id),
    -- The desktop file id ("discord", "com.discordapp.Discord"), or "program:<name>"
    -- for a program added by name.
    app_id      TEXT NOT NULL,
    name        TEXT NOT NULL,      -- as shown, e.g. "Discord"
    exec_names  TEXT NOT NULL,      -- program names to match, one per line
    flatpak_id  TEXT,               -- the Flatpak app id, for Flatpak apps
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX block_app_profile_idx ON block_app (profile_id);

-- Every time the daemon closed a blocked app during a session
-- ("Closed Steam twice this session"); one line per app per close, however
-- many processes it had.
CREATE TABLE app_closed (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES study_session(id),
    app_id      TEXT NOT NULL,
    name        TEXT NOT NULL,
    at          TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX app_closed_session_idx ON app_closed (session_id);

-- The profile a session blocks with; NULL = blocks nothing.
ALTER TABLE study_session ADD COLUMN profile_id TEXT REFERENCES block_profile(id);
ALTER TABLE session_series ADD COLUMN profile_id TEXT REFERENCES block_profile(id);
