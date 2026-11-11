-- A link to open a feed deadline in Ufora: the assignment's page when the
-- event links to one, else the event itself. NULL for deadlines added by hand.
ALTER TABLE deadline ADD COLUMN url TEXT;
