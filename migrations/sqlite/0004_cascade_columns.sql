-- Add cascade tracking columns for 2PC (Phase 5: Timeout & Recovery).
-- These columns enable efficient queries for stale cascade detection.
--
-- committed: false = pending 2PC (needs Confirmation), true = immediately committed
-- cascade_id: groups related pending events for atomic commit/rollback
--
-- Query patterns:
-- 1. Find stale cascades: SELECT DISTINCT cascade_id WHERE committed = 0
--    AND created_at < threshold AND cascade_id NOT IN (confirmed/revoked cascades)
-- 2. Find cascade participants: SELECT domain, root, sequence WHERE cascade_id = ?

-- Committed flag: false = pending 2PC, true = committed (default for existing events)
ALTER TABLE events ADD COLUMN committed INTEGER DEFAULT 1;

-- Cascade ID: groups related events for atomic commit/rollback (NULL = not in cascade)
ALTER TABLE events ADD COLUMN cascade_id TEXT;

-- Index for finding uncommitted events by cascade_id.
-- Partial index covers only uncommitted events (small subset of total).
CREATE INDEX IF NOT EXISTS idx_events_cascade
    ON events (cascade_id, domain, root, sequence)
    WHERE committed = 0;

-- Index for timeout detection: find uncommitted events by age.
-- Partial index on uncommitted events only.
CREATE INDEX IF NOT EXISTS idx_events_uncommitted_age
    ON events (created_at, cascade_id)
    WHERE committed = 0;
