---
sidebar_position: 5
keywords: [makefile, docker, container, build, pattern, overlay, mount]
---

# Container Overlay Pattern for Makefiles

A technique for running the same `make` commands on host and inside containers without conditionals or duplicate interfaces.

Works with [GNU Make](https://www.gnu.org/software/make/), [just](https://github.com/casey/just), or any file-based command runner.

---

## The Problem

Containerized builds ensure consistent environments, but create friction:

| Approach | Drawback |
|----------|----------|
| Dual files (`Makefile`, `Makefile.docker`) | Must know which to invoke |
| Conditional detection (`ifeq ($(IN_DOCKER),1)`) | Clutters Makefile with branches |
| Different commands per context | Cognitive overhead, documentation burden |

Ideally: same command, same interface, different implementation based on context.

---

## The Pattern

Mount a container-specific Makefile **over** the host Makefile inside the container.

```
Host filesystem:
├── Makefile              # Delegates to container
└── Makefile.container    # Runs commands directly

Inside container (after mount):
└── Makefile              # IS Makefile.container (mounted over)
```

The file swap is the detection mechanism. No conditionals needed.

---

## How It Works

### Host Makefile

Delegates all work to container execution:

```make
# Makefile (host version)
.PHONY: build test lint

build:
	docker run --rm \
		-v ./:/workspace \
		-v ./Makefile.container:/workspace/Makefile:ro \
		-w /workspace \
		myimage make build

test:
	docker run --rm \
		-v ./:/workspace \
		-v ./Makefile.container:/workspace/Makefile:ro \
		-w /workspace \
		myimage make test
```

Key: `Makefile.container` is mounted **over** `Makefile` inside the container.

### Container Makefile

Runs commands directly:

```make
# Makefile.container (becomes Makefile inside container)
.PHONY: build test lint

build:
	cargo build

test:
	cargo test

lint:
	cargo clippy
```

### User Experience

```bash
# On host
$ make build
# → Starts container, mounts overlay, runs `make build` inside

# Inside container
$ make build
# → Runs cargo build directly
```

Same command. Same interface. Context determines implementation.

---

## Advantages

### No Conditionals

Common pattern (cluttered):

```make
IN_DOCKER := $(shell test -f /.dockerenv && echo 1 || echo 0)
ifeq ($(IN_DOCKER),1)
build:
	cargo build
else
build:
	docker run ... make build
endif
```

Overlay pattern (clean):

```make
# Host Makefile: delegates
build:
	docker run -v ./Makefile.container:/workspace/Makefile:ro ... make build

# Container Makefile: executes
build:
	cargo build
```

### Single Interface

No need to remember:
- `make build` vs `make docker-build`
- `make test` vs `make container-test`
- Which file to edit for which context

### Clear Separation of Concerns

- **Host concerns** (container orchestration, mounts, networking) stay in host Makefile
- **Build concerns** (compilation, testing, linting) stay in container Makefile
- No mixing of responsibilities

### DRY Container Configuration

Extract container invocation to a variable:

```make
DOCKER_RUN := docker run --rm \
	-v ./:/workspace \
	-v ./Makefile.container:/workspace/Makefile:ro \
	-w /workspace \
	myimage

build:
	$(DOCKER_RUN) make build

test:
	$(DOCKER_RUN) make test
```

---

## Optional: Escape Hatch for Nested Containers

When running inside an IDE devcontainer, you may want to skip container nesting:

```make
ifdef DEVCONTAINER
DOCKER_RUN :=
else
DOCKER_RUN := docker run --rm -v ... myimage
endif

build:
	$(DOCKER_RUN) make -f Makefile.container build
```

This is optional—only needed if your workflow involves pre-existing container environments.

---

## Using with just

The same pattern works with [just](https://github.com/casey/just)—and the code is cleaner:

```
Host filesystem:
├── justfile              # Delegates to container
└── justfile.container    # Runs commands directly
```

**Host justfile:**
```just
_run +ARGS:
    podman run --rm \
        -v ./:/workspace:Z \
        -v ./justfile.container:/workspace/justfile:ro \
        -w /workspace \
        myimage just {{ARGS}}

build:
    just _run build

test:
    just _run test
```

**Container justfile:**
```just
build:
    cargo build

test:
    cargo test
```

### Why just is cleaner

| Make | just |
|------|------|
| Requires `.PHONY` declarations | No declarations needed |
| `$$(hostname)` escaping | `$(hostname)` works naturally |
| Verbose variable syntax | Clean argument passing (`{{ARGS}}`) |
| gnumake vs BSD make differences | Cross-platform consistency |

just's module system also composes naturally with this pattern—module commands route through the container transparently.

---

## Comparison with Prior Art

| Pattern | Detection | Files | Conditionals |
|---------|-----------|-------|--------------|
| Dual Makefiles | User chooses file | 2 separate interfaces | None |
| `/.dockerenv` check | Runtime file test | 1 | Yes |
| Env var check | `$IN_DOCKER` | 1 | Yes |
| **Mount overlay** | Mount replaces file | 2 files, 1 interface | None |

The mount overlay pattern uses two files but presents a single interface. Detection is implicit in the mount configuration, not explicit in code.

---

## When to Use

**Good fit:**
- Containerized CI/CD with local development parity
- Teams with mixed host environments (Linux, macOS, WSL)
- Projects where build tooling differs from runtime
- Polyglot projects with language-specific containers

**Not needed:**
- Simple projects without containerized builds
- When all developers use identical environments
- Single-platform deployments

---

## Working Example

A working example demonstrating this pattern:

```bash
cd docs/examples/container-overlay
make demo
```

## Implementation Checklist

1. Create host `Makefile` with container delegation
2. Create `Makefile.container` with direct commands
3. Configure container to mount overlay: `-v ./Makefile.container:/workspace/Makefile:ro`
4. Ensure same target names in both files
5. Test: `make <target>` should work identically in both contexts

---

## Next Steps

- **[GNU Make Manual](https://www.gnu.org/software/make/manual/)** — Make documentation
- **[just](https://github.com/casey/just)** — Modern command runner alternative
- **[just in Angzarr](/tooling/just)** — Project-specific just commands
