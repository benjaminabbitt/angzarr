-- Add external_id column to events table for idempotency.
-- External_id enables exactly-once delivery: duplicate requests with the same
-- external_id return the original event sequences without persisting duplicates.

ALTER TABLE events ADD COLUMN IF NOT EXISTS external_id TEXT NOT NULL DEFAULT '';

-- Index for fast lookup by external_id within an aggregate.
CREATE INDEX IF NOT EXISTS idx_events_external_id
    ON events (domain, edition, root, external_id)
    WHERE external_id != '';
