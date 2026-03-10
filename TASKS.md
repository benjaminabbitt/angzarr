# Tasks - TASKS

## In Progress

- [ ] Write acceptance tests (Gherkin) for SyncMode/CascadeErrorMode against deployed services

## To Do

- [ ] Implement distributed mode CASCADE - SagaCoordinatorService.Execute (proto defined but not implemented)
- [ ] Implement distributed mode CASCADE - ProcessManagerCoordinatorService.Handle (missing from proto)
- [ ] Add saga/PM discovery to ServiceDiscovery trait (get_sagas_for_domain, get_pms_for_domain)
- [ ] Extend GrpcAggregateContext.post_persist() to call sync sagas/PMs in CASCADE mode
- [ ] Update saga/PM binaries to expose gRPC coordinator services for sync calls

## Backlog


## Done

- [x] Standalone SyncMode (ASYNC/SIMPLE/CASCADE) implementation
- [x] Standalone CascadeErrorMode (FAIL_FAST/CONTINUE/COMPENSATE/DEAD_LETTER) implementation
- [x] Integration tests for CascadeErrorMode in standalone mode
