# Rust service template - define variables before importing:
#   PKG     - cargo package name (e.g., "cart")
#   BIN_PKG - cargo package for build/run binary (usually same as PKG)
#   BIN     - binary name (e.g., "cart-server")

set shell := ["bash", "-c"]

TOP := `git rev-parse --show-toplevel`

default:
    @just --list

build:
    cargo build -p {{BIN_PKG}} --bin {{BIN}}

build-release:
    cargo build -p {{BIN_PKG}} --bin {{BIN}} --release

run: build
    cargo run -p {{BIN_PKG}} --bin {{BIN}}

run-port port:
    PORT={{port}} cargo run -p {{BIN_PKG}} --bin {{BIN}}

copy-features:
    rm -rf tests/features
    cp -r {{TOP}}/examples/features tests/

unit:
    cargo test -p {{PKG}} --lib

acceptance: copy-features
    cargo test -p {{PKG}} --test acceptance

test: unit acceptance

test-unit: unit

clean:
    cargo clean -p {{PKG}}

check:
    cargo check -p {{PKG}}

lint:
    cargo clippy -p {{PKG}} -- -D warnings
