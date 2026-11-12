-- Settings and sent notifications (phase 4 step 7).
--
-- Both tables belong to this computer and are never synced, so they don't
-- follow the usual id / updated_at / deleted pattern.

-- Simple settings the app and the daemon share, e.g. language = 'nl' or
-- notify.deadlines = 'off'. A missing key means its default.
CREATE TABLE setting (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Each notification the daemon sent, by a key that names what it was about
-- (e.g. 'session-soon:<id>:<start>'), so it's never sent twice, also not
-- after a restart.
CREATE TABLE notification_sent (
    key         TEXT PRIMARY KEY,
    sent_at     TEXT NOT NULL
);
