-- Activities to check off, and templates that create them per course.

-- "After every werkcollege of Signalen en systemen: do the exercises."
CREATE TABLE activity_template (
    id          TEXT PRIMARY KEY,
    course_id   TEXT NOT NULL REFERENCES course(id),
    kind        TEXT,               -- NULL = every block of the course, else e.g. 'werkcollege'
    title       TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0
);

-- Something to check off. Either linked to a class block,
-- or on its own (then course and week are filled in by hand).
CREATE TABLE activity (
    id           TEXT PRIMARY KEY,
    block_id     TEXT REFERENCES class_block(id),        -- NULL = not linked to a block
    template_id  TEXT REFERENCES activity_template(id),  -- NULL = added by hand
    course_id    TEXT REFERENCES course(id),
    title        TEXT NOT NULL,
    week         TEXT,               -- Monday of the week, e.g. '2026-11-09'; only for activities without a block
    done         INTEGER NOT NULL DEFAULT 0,
    done_at      TEXT,
    updated_at   TEXT NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0
);

-- A template creates at most one activity per block, ever:
-- an activity you deleted (soft delete) doesn't come back on the next sync.
CREATE UNIQUE INDEX activity_template_block_idx ON activity (template_id, block_id)
    WHERE template_id IS NOT NULL;

CREATE INDEX activity_block_idx ON activity (block_id);
CREATE INDEX activity_week_idx ON activity (week);

-- Give every existing course a color from the default palette (same list as
-- COURSE_COLORS in activity.rs). New courses get one when they are created.
UPDATE course SET color = CASE (rowid % 8)
    WHEN 0 THEN '#6b9bd1'
    WHEN 1 THEN '#a685d6'
    WHEN 2 THEN '#d47fae'
    WHEN 3 THEN '#4fb3bf'
    WHEN 4 THEN '#c8b45a'
    WHEN 5 THEN '#8b93e8'
    WHEN 6 THEN '#c98b6b'
    ELSE        '#7fa88f'
END
WHERE color IS NULL;

-- Activities with the week they belong to, as the app shows them.
-- An activity on a block belongs to the week of that block, so it follows the
-- block when the class is moved. date(start, '-6 days', 'weekday 1') is the
-- Monday on or before the start. It works on the UTC time, which only gives the
-- wrong week for a class between 00:00 and 02:00 on a Monday night.
--
-- On an ignored block, only the open activities a template made are hidden:
-- what you added yourself or already checked off always stays visible.
CREATE VIEW activity_with_week AS
SELECT a.id, a.block_id, a.template_id, a.course_id, a.title, a.done, a.done_at,
       COALESCE(date(b.start_at, '-6 days', 'weekday 1'), a.week) AS week
FROM activity a
LEFT JOIN class_block b ON b.id = a.block_id
WHERE a.deleted = 0
  AND (b.id IS NULL
       OR (b.deleted = 0
           AND (COALESCE(b.ignored_override, b.ignored_by_rule) = 0
                OR a.template_id IS NULL
                OR a.done = 1)));
