# Go service template - set SERVICE before importing
#
# Required variables (set in importing justfile):
#   SERVICE - service/binary name (e.g., "cart")

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`

default:
    @just --list

proto:
    mkdir -p proto/examples
    for f in "{{TOP}}/generated/go/examples/"*.go; do case "$f" in */domains.pb.go) ;; *) cp "$f" proto/examples/ ;; esac; done

build: proto
    go mod tidy
    go build -o {{SERVICE}} .

run: build
    ./{{SERVICE}}

run-port port:
    PORT={{port}} ./{{SERVICE}}

clean:
    rm -f {{SERVICE}}
    rm -rf proto/examples/*.go

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
