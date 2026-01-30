# Go service template - set SERVICE before importing
#
# Required variables (set in importing justfile):
#   SERVICE - service/binary name (e.g., "cart")

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`

default:
    @just --list

proto:
    mkdir -p proto/angzarr proto/examples
    cp -r "{{TOP}}/generated/go/angzarr/"* proto/angzarr/ 2>/dev/null || true
    cp -r "{{TOP}}/generated/go/examples/"* proto/examples/ 2>/dev/null || true

build: proto
    go mod tidy
    go build -o {{SERVICE}} .

run: build
    ./{{SERVICE}}

run-port port:
    PORT={{port}} ./{{SERVICE}}

clean:
    rm -f {{SERVICE}}
    rm -rf proto/angzarr/*.go proto/examples/*.go

copy-features:
    rm -rf features
    cp -r {{TOP}}/examples/features .

unit: proto
    go test -v ./logic/...

acceptance: proto copy-features
    ANGZARR_TEST_MODE=container go test -v ./acceptance/...

test: unit acceptance

test-unit: unit

fmt:
    go fmt ./...

lint:
    golangci-lint run
