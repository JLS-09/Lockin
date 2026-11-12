-- Points (phase 4 step 6): every change to your points is one line, so the
-- balance is the sum and the statistics can show where points came from.
-- Step 6 only writes penalties (the emergency button); phase 6 adds points for
-- studying, streaks and the shop.
CREATE TABLE points_ledger (
    id          TEXT PRIMARY KEY,
    at          TEXT NOT NULL,                       -- RFC 3339 in UTC
    points      INTEGER NOT NULL,                    -- earned (+) or lost (-)
    -- Why, e.g. 'emergency'. Plain text without a CHECK, so later phases can
    -- add reasons without rebuilding the table.
    reason      TEXT NOT NULL,
    session_id  TEXT REFERENCES study_session(id),   -- the session it was for, if any
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX points_ledger_at_idx ON points_ledger (at);
CREATE INDEX points_ledger_session_idx ON points_ledger (session_id);
