-- Make `edition` genuinely nullable.
--
-- Rationale: Cover.edition is optional on the wire. SQL NULL is the
-- faithful representation of "no edition set / main timeline" — not an
-- "angzarr" literal, not an empty string. Postgres 15+ supports
-- `NULLS NOT DISTINCT` on unique constraints, which gives us the
-- composite-key semantics we need even when edition is NULL.

-- 1. Normalize existing rows (either sentinel) to NULL so the primary
--    key conversion below sees a single canonical representation.
UPDATE events     SET edition = NULL WHERE edition IN ('angzarr', '');
UPDATE snapshots  SET edition = NULL WHERE edition IN ('angzarr', '');
UPDATE positions  SET edition = NULL WHERE edition IN ('angzarr', '');

-- 2. Drop the current composite PKs (which included edition as NOT NULL).
-- positions DOES have a PK from 0001; the prior comment here was wrong
-- and caused step 3's DROP NOT NULL to fail on positions.edition.
ALTER TABLE events     DROP CONSTRAINT events_pkey;
ALTER TABLE snapshots  DROP CONSTRAINT snapshots_pkey;
ALTER TABLE positions  DROP CONSTRAINT positions_pkey;

-- 3. Allow NULL in the edition column.
ALTER TABLE events     ALTER COLUMN edition DROP NOT NULL;
ALTER TABLE snapshots  ALTER COLUMN edition DROP NOT NULL;
ALTER TABLE positions  ALTER COLUMN edition DROP NOT NULL;

-- 4. Re-add uniqueness with NULLS NOT DISTINCT so two rows with NULL
--    edition + same (domain, root, seq) are still treated as duplicates.
ALTER TABLE events
    ADD CONSTRAINT events_pkey
    UNIQUE NULLS NOT DISTINCT (domain, edition, root, sequence);

ALTER TABLE snapshots
    ADD CONSTRAINT snapshots_pkey
    UNIQUE NULLS NOT DISTINCT (domain, edition, root, sequence);

-- 5. Rebuild the composite read index (it was implicitly covered by the
--    PK before; now we want it explicit for edition-filtered lookups).
DROP INDEX IF EXISTS idx_events_domain_edition_root;
CREATE INDEX idx_events_domain_edition_root
    ON events (domain, edition, root);

DROP INDEX IF EXISTS idx_events_domain_edition_root_created_at;
CREATE INDEX idx_events_domain_edition_root_created_at
    ON events (domain, edition, root, created_at);

-- 6. Rewrite the composite-read stored procs so they filter main-timeline
--    rows by `edition IS NULL` instead of `edition = 'angzarr'`. Behavior
--    otherwise unchanged.
CREATE OR REPLACE FUNCTION get_edition_events(
    p_domain TEXT,
    p_edition TEXT,
    p_root TEXT,
    p_explicit_divergence INT DEFAULT NULL
) RETURNS TABLE (
    domain TEXT,
    edition TEXT,
    root TEXT,
    sequence INT,
    created_at TEXT,
    event_data BYTEA,
    correlation_id TEXT
) AS $$
BEGIN
    IF p_edition IS NULL OR p_edition = '' THEN
        RETURN QUERY
            SELECT e.domain, e.edition, e.root, e.sequence, e.created_at, e.event_data, e.correlation_id
            FROM events e
            WHERE e.domain = p_domain AND e.edition IS NULL AND e.root = p_root
            ORDER BY e.sequence ASC;
        RETURN;
    END IF;

    RETURN QUERY
    WITH edition_events AS (
        SELECT e.domain, e.edition, e.root, e.sequence, e.created_at, e.event_data, e.correlation_id
        FROM events e
        WHERE e.domain = p_domain AND e.edition = p_edition AND e.root = p_root
    ),
    divergence AS (
        SELECT COALESCE(p_explicit_divergence, MIN(ee.sequence), 0) as seq
        FROM edition_events ee
    )
    SELECT e.domain, e.edition, e.root, e.sequence, e.created_at, e.event_data, e.correlation_id
    FROM events e
    WHERE e.domain = p_domain AND e.edition IS NULL AND e.root = p_root
      AND e.sequence < (SELECT d.seq FROM divergence d)
    UNION ALL
    SELECT ee.domain, ee.edition, ee.root, ee.sequence, ee.created_at, ee.event_data, ee.correlation_id
    FROM edition_events ee
    ORDER BY sequence ASC;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION get_edition_events_from(
    p_domain TEXT,
    p_edition TEXT,
    p_root TEXT,
    p_from_seq INT,
    p_explicit_divergence INT DEFAULT NULL
) RETURNS TABLE (
    domain TEXT,
    edition TEXT,
    root TEXT,
    sequence INT,
    created_at TEXT,
    event_data BYTEA,
    correlation_id TEXT
) AS $$
BEGIN
    IF p_edition IS NULL OR p_edition = '' THEN
        RETURN QUERY
            SELECT e.domain, e.edition, e.root, e.sequence, e.created_at, e.event_data, e.correlation_id
            FROM events e
            WHERE e.domain = p_domain AND e.edition IS NULL AND e.root = p_root
              AND e.sequence >= p_from_seq
            ORDER BY e.sequence ASC;
        RETURN;
    END IF;

    RETURN QUERY
    WITH edition_events AS (
        SELECT e.domain, e.edition, e.root, e.sequence, e.created_at, e.event_data, e.correlation_id
        FROM events e
        WHERE e.domain = p_domain AND e.edition = p_edition AND e.root = p_root
    ),
    divergence AS (
        SELECT COALESCE(p_explicit_divergence, MIN(ee.sequence), 0) as seq
        FROM edition_events ee
    )
    SELECT e.domain, e.edition, e.root, e.sequence, e.created_at, e.event_data, e.correlation_id
    FROM events e
    WHERE e.domain = p_domain AND e.edition IS NULL AND e.root = p_root
      AND e.sequence < (SELECT d.seq FROM divergence d)
      AND e.sequence >= p_from_seq
    UNION ALL
    SELECT ee.domain, ee.edition, ee.root, ee.sequence, ee.created_at, ee.event_data, ee.correlation_id
    FROM edition_events ee
    WHERE ee.sequence >= p_from_seq
    ORDER BY sequence ASC;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION delete_edition_events(
    p_edition TEXT,
    p_domain TEXT
) RETURNS INT AS $$
DECLARE
    deleted_count INT;
BEGIN
    IF p_edition IS NULL OR p_edition = '' THEN
        RAISE EXCEPTION 'Cannot delete main timeline events';
    END IF;
    DELETE FROM events WHERE edition = p_edition AND domain = p_domain;
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;
