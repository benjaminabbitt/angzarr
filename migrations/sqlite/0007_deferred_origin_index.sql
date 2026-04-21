-- Sqlite mirror of postgres 0008_deferred_origin_index.sql. See that
-- file for rationale. Sqlite doesn't support partial indexes with the
-- same semantics, so use a full composite index — the saga-only column
-- predicate is approximated by ordering source columns last so the
-- index is degenerate (single NULL entry) for non-saga events.
CREATE INDEX IF NOT EXISTS idx_events_deferred_origin
    ON events (
        domain,
        edition,
        root,
        source_domain,
        source_edition,
        source_root,
        source_seq
    );
