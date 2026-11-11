-- Activities in the order they were added, instead of by title, so a list
-- doesn't reshuffle after a reload. The view from 0003 gets one more column:
-- `created_order`, SQLite's rowid, which grows with every insert.

DROP VIEW activity_with_week;

-- Same as in 0003, plus created_order.
CREATE VIEW activity_with_week AS
SELECT a.id, a.block_id, a.template_id, a.course_id, a.title, a.done, a.done_at,
       COALESCE(date(b.start_at, '-6 days', 'weekday 1'), a.week) AS week,
       a.rowid AS created_order
FROM activity a
LEFT JOIN class_block b ON b.id = a.block_id
WHERE a.deleted = 0
  AND (b.id IS NULL
       OR (b.deleted = 0
           AND (COALESCE(b.ignored_override, b.ignored_by_rule) = 0
                OR a.template_id IS NULL
                OR a.done = 1)));
