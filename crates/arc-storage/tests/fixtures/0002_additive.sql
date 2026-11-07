ALTER TABLE vendors ADD COLUMN note TEXT NOT NULL DEFAULT 'preserved';
CREATE TABLE migration_notes (vendor_id TEXT NOT NULL REFERENCES vendors(id), note TEXT NOT NULL);
INSERT INTO migration_notes SELECT id, 'upgraded' FROM vendors;
