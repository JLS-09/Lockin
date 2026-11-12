-- Websites on block profiles (phase 4 step 5). While you study, the daemon
-- blocks them through Lockin's own section of /etc/hosts (lockin-helper).

CREATE TABLE block_site (
    id          TEXT PRIMARY KEY,
    profile_id  TEXT NOT NULL REFERENCES block_profile(id),
    domain      TEXT NOT NULL,     -- e.g. "youtube.com" (www. and m. are blocked with it)
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX block_site_profile_idx ON block_site (profile_id);
