-- Expand-only Codex attribution. Existing vendor constraints and history stay intact.
-- Exactly one vendor column is set by the application in the job transaction.
ALTER TABLE jobs ADD COLUMN vendor_codex TEXT
  CHECK(vendor_codex IS NULL OR (vendor_codex='codex' AND vendor IS NULL));
