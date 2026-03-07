-- Add source tracking columns for saga-produced events (angzarr_deferred).
-- Source info enables idempotency: if an event exists with matching source,
-- the saga command was already processed.
--
-- Source info is stored with events, making the event log the single source
-- of truth for saga idempotency. Query: "does an event exist in this aggregate
-- with matching source (edition, domain, root, seq)?"

-- Source edition (usually "angzarr" for main timeline)
ALTER TABLE events ADD COLUMN IF NOT EXISTS source_edition TEXT;

-- Source domain (e.g., "order" for saga-order-fulfillment)
ALTER TABLE events ADD COLUMN IF NOT EXISTS source_domain TEXT;

-- Source aggregate root (UUID as hex string)
ALTER TABLE events ADD COLUMN IF NOT EXISTS source_root TEXT;

-- Source event sequence that triggered this command
ALTER TABLE events ADD COLUMN IF NOT EXISTS source_seq INTEGER;

-- Index for fast idempotency lookup by source.
-- Query: find events with matching source info for this aggregate
-- Partial index (WHERE source_edition IS NOT NULL) keeps it small since
-- most events won't have source tracking (only saga-produced ones).
CREATE INDEX IF NOT EXISTS idx_events_source
    ON events (domain, edition, root, source_edition, source_domain, source_root, source_seq)
    WHERE source_edition IS NOT NULL;
