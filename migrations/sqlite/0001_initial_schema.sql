-- Initial schema: events and snapshots tables for event sourcing.

CREATE TABLE IF NOT EXISTS events (
    domain TEXT NOT NULL,
    edition TEXT NOT NULL,
    root TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    event_data BLOB NOT NULL,
    correlation_id TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (domain, edition, root, sequence)
);

CREATE INDEX IF NOT EXISTS idx_events_domain_edition_root
    ON events (domain, edition, root);

CREATE INDEX IF NOT EXISTS idx_events_correlation_id
    ON events (correlation_id);

CREATE INDEX IF NOT EXISTS idx_events_domain_edition_root_created_at
    ON events (domain, edition, root, created_at);

CREATE TABLE IF NOT EXISTS snapshots (
    domain TEXT NOT NULL,
    edition TEXT NOT NULL,
    root TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    state_data BLOB NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (domain, edition, root)
);
