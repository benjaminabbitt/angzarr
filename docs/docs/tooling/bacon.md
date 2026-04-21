---
sidebar_position: 2
---

# bacon

[bacon](https://github.com/Canop/bacon) is a background Rust code checker. It watches for file changes and continuously runs `cargo check`, `cargo clippy`, or `cargo test` in a TUI.

---

## Installation

```bash
# Via cargo
cargo install --locked bacon

# macOS
brew install bacon
```

---

## Usage

```bash
# Start bacon with default job (build)
just watch

# Or run directly with a specific job
bacon check    # Fast type checking
bacon clippy   # Lints
bacon test     # Run tests
bacon doc      # Build docs
```

---

## Keybindings

| Key | Action |
|-----|--------|
| `c` | Switch to check job |
| `b` | Switch to build job |
| `t` | Switch to test job |
| `l` | Switch to clippy job |
| `d` | Switch to doc job |
| `f` | Switch to fmt job |
| `p` | Switch to proto generation |
| `q` | Quit |
| `w` | Toggle wrap |
| `Esc` | Back / clear |

---

## Available Jobs

### Core Jobs

| Job | Command |
|-----|---------|
| `check` | `cargo check --lib --bins` |
| `build` | `cargo build --lib --bins --release` |
| `test` | `cargo test --lib` |
| `clippy` | `cargo clippy --lib --bins -- -D warnings` |
| `doc` | `cargo doc --lib --no-deps` |
| `fmt` | `cargo fmt` |

### Example-Specific Jobs

Watch individual example crates:

| Job | Package |
|-----|---------|
| `ex-cart` | cart aggregate |
| `ex-customer` | customer aggregate |
| `ex-fulfillment` | fulfillment aggregate |
| `ex-inventory` | inventory-svc aggregate |
| `ex-order` | order aggregate |
| `ex-product` | product aggregate |
| `ex-proj-accounting` | accounting projector |
| `ex-proj-web` | web projector |
| `ex-proj-logging` | logging projector |
| `ex-saga-cancel` | cancellation saga |
| `ex-saga-fulfill` | fulfillment saga |
| `ex-saga-loyalty` | loyalty-earn saga |
| `ex-e2e` | e2e tests |

### Language-Specific Jobs

| Job | Description |
|-----|-------------|
| `ex-rust` | Full Rust workspace check |
| `ex-go` | Go examples build |
| `ex-python` | Python examples lint |

### Proto Generation Jobs

| Job | Description |
|-----|-------------|
| `proto` | Generate all language protos |
| `proto-rust` | Generate Rust protos only |
| `proto-python` | Generate Python protos only |
| `proto-go` | Generate Go protos only |

### Binary Build Jobs

| Job | Binary |
|-----|--------|
| `bin-aggregate` | angzarr-aggregate |
| `bin-saga` | angzarr-saga |
| `bin-projector` | angzarr-projector |
| `bin-gateway` | angzarr-gateway |
| `bin-stream` | angzarr-stream |
| `bin-event-projector` | angzarr-event-projector |

---

## Configuration

Configuration is in `bacon.toml` at the project root. Jobs define:

- `command` - The command to run
- `need_stdout` - Whether to capture stdout (true for tests)
- `on_change_strategy` - How to handle file changes (`kill_then_restart`)
- `watch` - Optional custom watch paths (defaults to Rust sources)

---

## Next Steps

- **[just](/tooling/just)** — Command runner integration
- **[Getting Started](/getting-started)** — Full development workflow
