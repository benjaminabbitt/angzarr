# Tasks - Todos

## In Progress

- [ ] build dev container for angzaar

## To Do

- [ ] Evaluate database strategy: add PostgresEventStore implementation, consider EventStoreDB or other purpose-built event stores
- [ ] Build Helm charts and Terraform for k8s deployment
- [ ] Terraform for AWS: EKS + MemoryDB for Redis (event store + cache)
- [ ] Terraform for GCP: GKE + Bigtable (event store + cache)
- [ ] Implement RedisEventStore (MemoryDB) using sorted sets for events, hashes for snapshots
- [ ] Implement BigtableEventStore for GCP

## Backlog


## Done

- [x] Research optimal event-sourcing storage on AWS (Aurora vs DynamoDB vs Timestream vs QLDB) -> MemoryDB recommended
- [x] Research optimal event-sourcing storage on GCP (Cloud Spanner vs Cloud SQL vs Firestore vs Bigtable) -> Bigtable recommended
- [x] Fix SQLite schema: composite PK (domain, root, sequence), removed surrogate id
- [x] Fix SQL injection in SqliteSnapshotStore::put - now uses sea-query OnConflict
- [x] Remove unused hex dependency
- [x] Rust example: integration tests running
- [x] Go example: integration tests running
- [x] Python example: integration tests running
- [x] Java example: get integration tests running
- [x] Build dev container for Ruby examples (with test deps)
- [x] Ruby example: get integration tests running
- [x] Build dev container for Go examples (with test deps)
- [x] Build dev container for Python examples (with test deps)
- [x] Build dev container for Rust examples (with test deps)
- [x] Build dev container for TypeScript examples (with test deps)
- [x] Build dev container for Java examples (with test deps)
- [x] Build dev container for Kotlin examples (with test deps)
- [x] Build dev container for C# examples (with test deps)
- [x] TypeScript example: get integration tests running
- [x] C# example: get integration tests running
- [x] Kotlin: fix TLS/networking issue in dev container (gradle can't reach maven central)
- [x] Kotlin example: get integration tests running

## Reminders

- Each example self-contained: symlinks to shared features/, own justfile, only depends on angzarr framework

## Notes

- Dev containers include: proto bindings, test deps, mount source code, run tests via justfile
