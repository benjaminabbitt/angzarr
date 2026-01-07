# Tasks

## In Progress

## To Do

- Set up AMQP/RabbitMQ event bus as alternative to direct gRPC calls
- Implement multi-language business logic support:
  - Python: PyO3 in-process (feature flag: `python`) - in progress
  - Go: FFI via C-shared library + libloading (feature flag: `go-ffi`)
  - Any language: gRPC out-of-process (default, already implemented)
- Evaluate database strategy: add PostgresEventStore implementation, consider EventStoreDB or other purpose-built event stores
- Containerize the entire system (Dockerfile, docker-compose)
- Build Helm charts and Terraform for k8s deployment
- Terraform for AWS: EKS + MemoryDB for Redis (event store + cache)
- Terraform for GCP: GKE + Bigtable (event store + cache)
- Implement RedisEventStore (MemoryDB) using sorted sets for events, hashes for snapshots
- Implement BigtableEventStore for GCP

## Backlog

## Done

- Research optimal event-sourcing storage on AWS (Aurora vs DynamoDB vs Timestream vs QLDB) -> MemoryDB recommended
- Research optimal event-sourcing storage on GCP (Cloud Spanner vs Cloud SQL vs Firestore vs Bigtable) -> Bigtable recommended
- Fix SQLite schema: composite PK (domain, root, sequence), removed surrogate id
- Fix SQL injection in SqliteSnapshotStore::put - now uses sea-query OnConflict
- Remove unused hex dependency

## Reminders

## Notes
