-- Earlier weeks imported from a downloaded TimeEdit file ("Import file…") can be
-- removed again: each import is remembered, and its classes point to it.

CREATE TABLE history_import (
    id           TEXT PRIMARY KEY,
    name         TEXT,                -- the file name; NULL for imports from before this migration
    imported_at  TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0
);

-- The import a class came from; NULL for classes from a feed or added by hand.
-- A class the live feed also has gets a source_id, and then belongs to the feed.
ALTER TABLE class_block ADD COLUMN import_id TEXT REFERENCES history_import(id);

-- Classes imported before this migration (no feed, but an iCal UID) become one
-- "earlier import", so they can be removed too.
INSERT INTO history_import (id, name, imported_at, updated_at)
SELECT lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4'
         || substr(lower(hex(randomblob(2))), 2) || '-'
         || substr('89ab', 1 + (abs(random()) % 4), 1)
         || substr(lower(hex(randomblob(2))), 2) || '-' || lower(hex(randomblob(6))),
       NULL, MIN(updated_at), MIN(updated_at)
FROM class_block
WHERE source_id IS NULL AND ical_uid IS NOT NULL
HAVING COUNT(*) > 0;

UPDATE class_block
SET import_id = (SELECT id FROM history_import LIMIT 1)
WHERE source_id IS NULL AND ical_uid IS NOT NULL;
