-- Fact injection idempotency tracking.
-- Ensures external_id is only claimed once per aggregate root.

CREATE TABLE IF NOT EXISTS fact_idempotency (
    domain TEXT NOT NULL,
    edition TEXT NOT NULL,
    root TEXT NOT NULL,
    external_id TEXT NOT NULL,
    first_sequence INTEGER NOT NULL,
    last_sequence INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (domain, edition, root, external_id)
);

-- Index for lookups by external_id across all roots in a domain
CREATE INDEX IF NOT EXISTS idx_fact_idempotency_domain_external_id
    ON fact_idempotency (domain, external_id);
