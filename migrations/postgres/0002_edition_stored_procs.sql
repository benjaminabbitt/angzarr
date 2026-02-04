-- Edition-aware event queries with implicit/explicit divergence support.
--
-- Editions fork the main timeline at a divergence point. These stored procedures
-- return composite views: main timeline events up to divergence + edition events.

-- Get events for an aggregate with edition support.
--
-- For main timeline (edition IS NULL or 'angzarr'): returns all events.
-- For editions: returns main timeline up to divergence + edition events.
--
-- Divergence is either:
-- - Explicit: p_explicit_divergence specifies the sequence number
-- - Implicit: derived from the first event written to this edition
CREATE OR REPLACE FUNCTION get_edition_events(
    p_domain TEXT,
    p_edition TEXT,
    p_root UUID,
    p_explicit_divergence INT DEFAULT NULL
) RETURNS TABLE (
    domain TEXT,
    edition TEXT,
    root UUID,
    sequence INT,
    created_at TEXT,
    event_data BYTEA,
    correlation_id TEXT
) AS $$
BEGIN
    -- Main timeline: simple query
    IF p_edition IS NULL OR p_edition = '' OR p_edition = 'angzarr' THEN
        RETURN QUERY
            SELECT e.domain, e.edition, e.root, e.sequence, e.created_at, e.event_data, e.correlation_id
            FROM events e
            WHERE e.domain = p_domain AND e.edition = 'angzarr' AND e.root = p_root
            ORDER BY e.sequence ASC;
        RETURN;
    END IF;

    -- Edition: composite query with divergence
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
    WHERE e.domain = p_domain AND e.edition = 'angzarr' AND e.root = p_root
      AND e.sequence < (SELECT d.seq FROM divergence d)
    UNION ALL
    SELECT ee.domain, ee.edition, ee.root, ee.sequence, ee.created_at, ee.event_data, ee.correlation_id
    FROM edition_events ee
    ORDER BY sequence ASC;
END;
$$ LANGUAGE plpgsql;

-- Get events from a specific sequence onwards with edition support.
CREATE OR REPLACE FUNCTION get_edition_events_from(
    p_domain TEXT,
    p_edition TEXT,
    p_root UUID,
    p_from_seq INT,
    p_explicit_divergence INT DEFAULT NULL
) RETURNS TABLE (
    domain TEXT,
    edition TEXT,
    root UUID,
    sequence INT,
    created_at TEXT,
    event_data BYTEA,
    correlation_id TEXT
) AS $$
BEGIN
    -- Main timeline: simple query with from filter
    IF p_edition IS NULL OR p_edition = '' OR p_edition = 'angzarr' THEN
        RETURN QUERY
            SELECT e.domain, e.edition, e.root, e.sequence, e.created_at, e.event_data, e.correlation_id
            FROM events e
            WHERE e.domain = p_domain AND e.edition = 'angzarr' AND e.root = p_root
              AND e.sequence >= p_from_seq
            ORDER BY e.sequence ASC;
        RETURN;
    END IF;

    -- Edition: composite query with divergence and from filter
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
    WHERE e.domain = p_domain AND e.edition = 'angzarr' AND e.root = p_root
      AND e.sequence < (SELECT d.seq FROM divergence d)
      AND e.sequence >= p_from_seq
    UNION ALL
    SELECT ee.domain, ee.edition, ee.root, ee.sequence, ee.created_at, ee.event_data, ee.correlation_id
    FROM edition_events ee
    WHERE ee.sequence >= p_from_seq
    ORDER BY sequence ASC;
END;
$$ LANGUAGE plpgsql;

-- Delete all events for an edition+domain combination.
-- Returns the count of deleted events.
-- Cannot delete main timeline events ('angzarr' edition).
CREATE OR REPLACE FUNCTION delete_edition_events(
    p_edition TEXT,
    p_domain TEXT
) RETURNS INT AS $$
DECLARE
    deleted_count INT;
BEGIN
    -- Safety: cannot delete main timeline
    IF p_edition IS NULL OR p_edition = '' OR p_edition = 'angzarr' THEN
        RAISE EXCEPTION 'Cannot delete main timeline events';
    END IF;

    DELETE FROM events
    WHERE edition = p_edition AND domain = p_domain;

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;
