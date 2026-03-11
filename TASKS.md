# Tasks - TASKS

## In Progress


## To Do


## Backlog

- [ ] Phase 2.2: Extract Python client to angzarr-client-python repo
- [ ] Phase 3.2: Extract Python examples to angzarr-examples-python repo
- [ ] Phase 4: Extract remaining languages (Go, Rust, Java, C#, C++)
- [ ] Phase 5: Clean up core repo - remove client/ and examples/

## Done

- [x] Standalone SyncMode (ASYNC/SIMPLE/CASCADE) implementation
- [x] Standalone CascadeErrorMode (FAIL_FAST/CONTINUE/COMPENSATE/DEAD_LETTER) implementation
- [x] Integration tests for CascadeErrorMode in standalone mode
- [x] Write acceptance tests (Gherkin) for SyncMode/CascadeErrorMode against deployed services
- [x] Implement distributed mode CASCADE - SagaCoordinatorService.Execute (proto defined but not implemented)
- [x] Implement distributed mode CASCADE - ProcessManagerCoordinatorService.Handle (missing from proto)
- [x] Add saga/PM discovery to ServiceDiscovery trait (get_sagas_for_domain, get_pms_for_domain)
- [x] Extend GrpcAggregateContext.post_persist() to call sync sagas/PMs in CASCADE mode
- [x] Update saga/PM binaries to expose gRPC coordinator services for sync calls
- [x] Phase 1.1: Move feature files to features/ directory in core
- [x] Phase 1.2: Add buf-publish.yml workflow and update proto/buf.yaml
