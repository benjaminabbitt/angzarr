-- Positions table: tracks last-processed sequence per handler/domain/edition/root.

CREATE TABLE IF NOT EXISTS positions (
    handler TEXT NOT NULL,
    domain TEXT NOT NULL,
    edition TEXT NOT NULL,
    root BLOB NOT NULL,
    sequence INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (handler, domain, edition, root)
);
