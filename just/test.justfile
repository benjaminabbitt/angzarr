# Test commands

# Run unit tests (no infrastructure required)
unit:
    cargo test --lib

# Run unit tests with fast-dev profile
unit-fast:
    cargo test --lib --profile fast-dev
