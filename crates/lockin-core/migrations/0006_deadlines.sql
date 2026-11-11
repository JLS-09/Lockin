-- Deadlines: added by hand now, from the Ufora feed later in phase 3.
-- Like class blocks, a feed deadline is matched on its iCal UID, so a changed
-- due date becomes "moved" and one that leaves the feed becomes "cancelled",
-- never silently deleted.

CREATE TABLE deadline (
    id           TEXT PRIMARY KEY,
    source_id    TEXT REFERENCES calendar_source(id),  -- NULL = added by hand
    ical_uid     TEXT UNIQUE,                          -- NULL = added by hand
    course_id    TEXT REFERENCES course(id),           -- NULL = not linked to a course
    title        TEXT NOT NULL,
    description  TEXT,
    due_at       TEXT NOT NULL,                        -- RFC 3339 in UTC, like every time
    done         INTEGER NOT NULL DEFAULT 0,
    done_at      TEXT,
    status       TEXT NOT NULL DEFAULT 'normal' CHECK (status IN ('normal', 'moved', 'cancelled')),
    moved_from   TEXT,                                 -- due time before the first move
    change_seen  INTEGER NOT NULL DEFAULT 0,           -- 1 after "Got it", as on class_block
    updated_at   TEXT NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX deadline_due_idx ON deadline (due_at);
