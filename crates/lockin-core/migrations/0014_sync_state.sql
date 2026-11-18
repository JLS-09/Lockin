-- Where the cloud sync stands (phase 5, step 3). Kept on this computer only and
-- not synced itself, like `setting`: one row per name, e.g.
--   account          the account this database syncs with (its user id)
--   push_mark        changes from this time on haven't been sent yet
--   pull:<table>     the newest synced_at fetched from the cloud for a table
--   last_sync_at     when the last sync finished
--   last_error       why the last sync failed (empty when it worked)
CREATE TABLE sync_state (
    name   TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);
