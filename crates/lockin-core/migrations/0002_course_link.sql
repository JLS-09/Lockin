-- Link class blocks to courses, detected from the TimeEdit title.
-- A UGent title looks like "E702010A. Signalen en systemen GR02, werkcollege":
-- course code, course name, optional group, then the kind of class.

-- One course per code; courses added by hand may have no code.
CREATE UNIQUE INDEX course_code_idx ON course (code) WHERE code IS NOT NULL;

-- The kind of class: hoorcollege, werkcollege, practicum, ...
-- Activity templates ("after every werkcollege: exercises") will use this.
ALTER TABLE class_block ADD COLUMN kind TEXT;