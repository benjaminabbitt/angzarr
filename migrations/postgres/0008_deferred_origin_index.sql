-- Index to support `EventStore::find_by_source` lookups used by
-- `AggregateContext::check_deferred_idempotency`. Saga-produced commands
-- carry an `AngzarrDeferredSequence` page header; on delivery the
-- pipeline looks up `(domain, edition, root, source_*)` to detect AMQP
-- redeliveries before the destination handler is ever invoked.
--
-- Partial index: only saga-produced events carry source info, and
-- aggregate handlers' own emissions never need this lookup. Keeps the
-- write path on direct commands cheap.
CREATE INDEX IF NOT EXISTS idx_events_deferred_origin
    ON events (
        domain,
        edition,
        root,
        source_domain,
        source_edition,
        source_root,
        source_seq
    )
    WHERE source_domain IS NOT NULL;
