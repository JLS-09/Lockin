-- Schedule changes, as the week view shows them:
-- "Moved · was Fri 13:00" needs the original time, and the banner
-- "TimeEdit changed 2 blocks … Got it" needs to know what you've already seen.

-- Start time before the block was first moved. NULL = never moved
-- (or moved before this column existed).
ALTER TABLE class_block ADD COLUMN moved_from TEXT;

-- 1 once you've clicked "Got it" on the change; back to 0 when it changes again.
ALTER TABLE class_block ADD COLUMN change_seen INTEGER NOT NULL DEFAULT 0;

-- Course colors from the UI mockups (same list as COURSE_COLORS in activity.rs).
-- Courses that still have a default color from migration 0003 get the matching new one.
UPDATE course SET color = CASE color
    WHEN '#6b9bd1' THEN '#3987e5'
    WHEN '#a685d6' THEN '#9085e9'
    WHEN '#d47fae' THEN '#d55181'
    WHEN '#4fb3bf' THEN '#199e70'
    WHEN '#c8b45a' THEN '#2aa5b8'
    WHEN '#8b93e8' THEN '#c56cc8'
    WHEN '#c98b6b' THEN '#9aa43c'
    WHEN '#7fa88f' THEN '#6b8fb3'
    ELSE color
END;
