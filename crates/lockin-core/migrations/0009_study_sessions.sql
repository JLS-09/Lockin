-- Study sessions (phase 4): planned blocks of study time that the daemon starts
-- and ends on time, plus the log of what happened during each one.
-- Block profiles (which apps and sites a session blocks) follow in a later migration.

-- A weekly repeat, e.g. "every Wednesday 13:00–15:30 until the end of the semester".
-- Its sessions are made right away as rows in study_session (with series_id), so
-- one week can still be changed or removed on its own.
CREATE TABLE session_series (
    id          TEXT PRIMARY KEY,
    course_id   TEXT REFERENCES course(id),  -- NULL = not about one course
    title       TEXT,                        -- NULL = just "Study session"
    until_date  TEXT NOT NULL,               -- last day with a session, YYYY-MM-DD (local)
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE study_session (
    id          TEXT PRIMARY KEY,
    series_id   TEXT REFERENCES session_series(id),  -- NULL = planned once
    course_id   TEXT REFERENCES course(id),          -- NULL = not about one course
    title       TEXT,                                -- NULL = just "Study session"
    start_at    TEXT NOT NULL,                       -- RFC 3339 in UTC, like every time
    end_at      TEXT NOT NULL,
    -- planned: still to come
    -- running: started by the daemon, not over yet
    -- done:    ran until its end
    -- stopped: ended early with the emergency button
    -- missed:  its start passed while the daemon wasn't running (no points, no penalty)
    -- skipped: reserved for later (a session you chose not to do)
    status      TEXT NOT NULL DEFAULT 'planned'
                CHECK (status IN ('planned', 'running', 'done', 'stopped', 'missed', 'skipped')),
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX study_session_start_idx ON study_session (start_at);

-- What happened during a session, in order. The statistics (phase 6) are
-- calculated from this log, and so is the break credit while a session runs.
CREATE TABLE session_event (
    id          TEXT PRIMARY KEY,
    session_id  TEXT NOT NULL REFERENCES study_session(id),
    kind        TEXT NOT NULL
                CHECK (kind IN ('start', 'break', 'resume', 'emergency', 'end')),
    at          TEXT NOT NULL,   -- RFC 3339 in UTC
    detail      TEXT,            -- e.g. 'daemon' on a start
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX session_event_session_idx ON session_event (session_id, at);
