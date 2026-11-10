-- First version of the schema: only what phase 1 needs.
-- Every table has a UUID key (stored as TEXT), updated_at and deleted,
-- so the later sync with Supabase stays simple.
-- Timestamps are stored as RFC 3339 text in UTC, e.g. 2026-11-10T08:15:00Z.

CREATE TABLE course (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    code        TEXT,
    color       TEXT,
    semester    TEXT,
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE calendar_source (
    id               TEXT PRIMARY KEY,
    kind             TEXT NOT NULL CHECK (kind IN ('timeedit', 'ufora')),
    url              TEXT NOT NULL UNIQUE,
    last_fetched_at  TEXT,
    updated_at       TEXT NOT NULL,
    deleted          INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE filter_rule (
    id          TEXT PRIMARY KEY,
    source_id   TEXT REFERENCES calendar_source(id),  -- NULL = applies to every source
    field       TEXT NOT NULL CHECK (field IN ('title', 'location', 'description')),
    pattern     TEXT NOT NULL,                        -- case-insensitive "contains"
    action      TEXT NOT NULL DEFAULT 'ignore' CHECK (action IN ('ignore')),
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE class_block (
    id                TEXT PRIMARY KEY,
    source_id         TEXT REFERENCES calendar_source(id),  -- NULL = added manually
    ical_uid          TEXT UNIQUE,                          -- NULL = added manually
    course_id         TEXT REFERENCES course(id),
    title             TEXT NOT NULL,
    location          TEXT,
    description       TEXT,
    start_at          TEXT NOT NULL,
    end_at            TEXT NOT NULL,
    -- Ignoring: the rules decide, unless you set a manual exception.
    ignored_by_rule   INTEGER NOT NULL DEFAULT 0,
    ignored_override  INTEGER,                              -- NULL = follow the rules, 0/1 = manual
    status            TEXT NOT NULL DEFAULT 'normal' CHECK (status IN ('normal', 'moved', 'cancelled')),
    updated_at        TEXT NOT NULL,
    deleted           INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX class_block_start_idx ON class_block (start_at);
