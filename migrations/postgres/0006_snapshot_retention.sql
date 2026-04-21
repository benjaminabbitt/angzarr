-- Add retention support to snapshots table.
-- This allows keeping multiple snapshots per aggregate with different retention policies.

-- Step 1: Create new table with correct schema
CREATE TABLE IF NOT EXISTS snapshots_new (
    domain TEXT NOT NULL,
    edition TEXT NOT NULL,
    root TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    state_data BYTEA NOT NULL,
    retention INTEGER NOT NULL DEFAULT 0,  -- 0=DEFAULT, 1=PERSIST, 2=TRANSIENT
    created_at TEXT NOT NULL,
    PRIMARY KEY (domain, edition, root, sequence)
);

-- Step 2: Migrate existing data (treat all as DEFAULT retention)
INSERT INTO snapshots_new (domain, edition, root, sequence, state_data, retention, created_at)
SELECT domain, edition, root, sequence, state_data, 0, created_at
FROM snapshots
ON CONFLICT (domain, edition, root, sequence) DO NOTHING;

-- Step 3: Drop old table and rename new. Postgres preserves constraint
-- names through RENAME TABLE, so the rebuilt `snapshots` table still
-- carries `snapshots_new_pkey` — explicitly rename it back to
-- `snapshots_pkey` so later migrations (notably 0007) can drop it by
-- the conventional name.
DROP TABLE IF EXISTS snapshots;
ALTER TABLE snapshots_new RENAME TO snapshots;
ALTER TABLE snapshots RENAME CONSTRAINT snapshots_new_pkey TO snapshots_pkey;

-- Step 4: Create index for efficient latest snapshot queries
CREATE INDEX IF NOT EXISTS idx_snapshots_latest
    ON snapshots (domain, edition, root, sequence DESC);

-- Step 5: Create index for retention-based cleanup queries
CREATE INDEX IF NOT EXISTS idx_snapshots_retention
    ON snapshots (domain, edition, root, retention, sequence);
