---
sidebar_position: 6
---

# immudb

immudb provides **cryptographic verification** for audit-critical event sourcing where tamper-proof guarantees are essential.

---

## Why immudb

| Strength | Benefit |
|----------|---------|
| **Immutable by design** | Events cannot be altered post-write |
| **Cryptographic proof** | Merkle tree verification |
| **Tamper detection** | Any modification breaks proofs |
| **Audit compliance** | SOC 2, HIPAA, financial regulations |

---

## Trade-offs

| Concern | Consideration |
|---------|---------------|
| **Performance** | Slower than non-verified writes |
| **Complexity** | Proof verification adds overhead |
| **Storage growth** | Merkle proofs increase storage |

---

## Implementation Note

Angzarr connects to immudb via its **PostgreSQL wire protocol compatibility** layer. This means:
- Uses the standard `sqlx` PostgreSQL driver
- Connection strings follow PostgreSQL format
- Query syntax is PostgreSQL-compatible

---

## Configuration

```toml
[storage]
backend = "immudb"

[storage.immudb]
host = "localhost"
port = 3322
database = "angzarr"
username = "immudb"
password = "immudb"
```

### Environment Variables

```bash
export IMMUDB_HOST="localhost"
export IMMUDB_PORT="3322"
export IMMUDB_DATABASE="angzarr"
export IMMUDB_USERNAME="immudb"
export IMMUDB_PASSWORD="immudb"
export STORAGE_BACKEND="immudb"
```

---

## Cryptographic Verification

Every event write returns a proof:

```rust
// Write event
let tx = store.add_event(&event).await?;

// Verify event wasn't tampered
let verified = store.verify_event(&event, &tx.proof).await?;
assert!(verified);
```

---

## Merkle Tree Structure

```
            Root Hash
           /         \
      Hash(0-1)    Hash(2-3)
      /     \      /     \
   H(e0)  H(e1)  H(e2)  H(e3)
     |      |      |      |
   Event0 Event1 Event2 Event3
```

Modifying any event changes its hash, which propagates up, changing the root. Old root hashes become invalid.

---

## Audit Trail

Query historical state with cryptographic proof:

```rust
// Get event at specific transaction
let event = store.get_event_at_tx(&cover, sequence, tx_id).await?;

// Verify it matches the state at that time
let proof = store.get_proof_at_tx(tx_id).await?;
```

---

## Docker Setup

```bash
# Start immudb
docker run -d \
  --name immudb \
  -p 3322:3322 \
  -p 9497:9497 \
  codenotary/immudb:latest

# Web console at http://localhost:9497
```

---

## When to Use immudb

- **Financial services** — Regulatory compliance
- **Healthcare** — HIPAA audit trails
- **Legal** — Tamper-proof records
- **Supply chain** — Provenance tracking
- **Government** — Public records integrity

---

## Testing

```bash
# Run immudb tests (requires testcontainers)
cargo test --test storage_immudb --features immudb
```

---

## Next Steps

- **[PostgreSQL](/tooling/databases/postgres)** — Standard alternative
- **[Testing](/operations/testing)** — Verification in tests
