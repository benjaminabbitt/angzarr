<!-- SCM:BEGIN -->
@.scm/context.md
<!-- SCM:END -->

## Technical Parity
The in process execution is intended to ease development by making it run in a single binary.  It is not intended to be used in production.

## Interface Parity
The FFI (Go) and Python client interfaces must maintain parity with the gRPC interface. All three should expose identical operations with equivalent semantics. Changes to the gRPC service definitions must be reflected in FFI and Python bindings.
