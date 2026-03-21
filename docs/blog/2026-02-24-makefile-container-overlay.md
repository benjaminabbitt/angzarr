---
slug: makefile-container-overlay-pattern
title: "The Container Overlay Pattern: Same Makefile Command, Different Context"
authors: [angzarr]
tags: [patterns, docker, makefile, devops]
keywords: [makefile, docker, container, build, pattern, overlay, mount, ci-cd]
---

import BlogHeader from '@site/src/components/BlogHeader';

<BlogHeader />

How we eliminated conditionals from our Makefile while supporting both host and containerized builds with a single command interface.

<!-- truncate -->

## The Problem We Faced

We wanted containerized builds for consistency across developer machines and CI. But every approach we tried had friction:

**Dual Makefiles** (`Makefile` and `Makefile.docker`): Works, but now everyone has to remember which file to use. Documentation says "run `make -f Makefile.docker build`" and someone inevitably runs `make build` instead.

**Conditional detection**: Check for `/.dockerenv` or an environment variable:

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

This works but clutters the Makefile. Every target needs the conditional. The file becomes a maze of `ifeq`/`else`/`endif` blocks.

**Different commands**: `make build` on host, `make container-build` for Docker. Now you have parallel target names, duplicate documentation, and cognitive overhead.

We wanted something simpler: **same command, different behavior based on context**.

## The Insight

Docker bind mounts can replace individual files inside the container. The [Docker documentation](https://docs.docker.com/engine/storage/bind-mounts/) even mentions this—if you mount over an existing file, the original is "obscured."

What if we mount a *different* Makefile over the host's Makefile inside the container?

## The Pattern

Two files, one interface:

```
project/
├── Makefile              # Host version: delegates to container
└── Makefile.container    # Container version: runs commands directly
```

**Host Makefile** starts the container and mounts the overlay:

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

**Container Makefile** runs commands directly:

```make
build:
    cargo build

test:
    cargo test
```

The key line: `-v ./Makefile.container:/workspace/Makefile:ro`

This mounts `Makefile.container` **over** `Makefile` inside the container. When the container runs `make build`, it sees `Makefile.container` as `Makefile`.

## Why This Works

The file swap *is* the detection mechanism.

- **On host**: `make build` → runs Docker → mounts overlay → runs `make build` inside container
- **In container**: `make build` → runs `cargo build` directly (because `Makefile` is now `Makefile.container`)

No conditionals. No environment variable checks. No remembering which command to run. The mount handles everything.

## Advantages Over Alternatives

### Cleaner Than Conditionals

Before (conditional detection):
```make
IN_DOCKER := $(shell test -f /.dockerenv && echo 1 || echo 0)
ifeq ($(IN_DOCKER),1)
build:
    cargo build
test:
    cargo test
lint:
    cargo clippy
else
build:
    docker run ... make build
test:
    docker run ... make test
lint:
    docker run ... make lint
endif
```

After (overlay pattern):
```make
# Host Makefile - just delegation
build:
    $(DOCKER_RUN) make build
test:
    $(DOCKER_RUN) make test
lint:
    $(DOCKER_RUN) make lint
```

```make
# Container Makefile - just execution
build:
    cargo build
test:
    cargo test
lint:
    cargo clippy
```

Same number of lines, but separated by concern. Host file handles orchestration. Container file handles execution. No mixing.

### Simpler Than Dual Files

With separate `Makefile` and `Makefile.docker`, users must know which to invoke. CI scripts use one, developers might use another. Documentation has to explain both.

With the overlay pattern, there's one command: `make build`. It works everywhere. The context determines the implementation.

### Clear Separation of Concerns

**Host Makefile responsibilities:**
- Container image selection
- Volume mounts
- Network configuration
- Environment variables

**Container Makefile responsibilities:**
- Compilation
- Testing
- Linting
- Any actual build logic

These concerns don't mix. When build logic changes, edit the container file. When container orchestration changes, edit the host file.

## Edge Cases

### Already in a Container?

If you're using VS Code devcontainers or similar, you might already be inside a container. Running Docker-in-Docker works but adds overhead.

Optional escape hatch:

```make
ifdef DEVCONTAINER
DOCKER_RUN :=
else
DOCKER_RUN := docker run --rm -v ... myimage
endif

build:
    $(DOCKER_RUN) make build
```

When `DEVCONTAINER` is set, `DOCKER_RUN` becomes empty and commands run directly. This is the one conditional we allow—and it's optional.

### Podman?

Same pattern, swap `docker` for `podman`. We use Podman with the `:Z` SELinux flag:

```make
PODMAN_RUN := podman run --rm \
    -v ./:/workspace:Z \
    -v ./Makefile.container:/workspace/Makefile:ro \
    -w /workspace \
    myimage
```

### What About just?

The pattern works identically with [just](https://github.com/casey/just)—and the code is cleaner:

```just
# justfile (host)
_run +ARGS:
    podman run -v ./justfile.container:/workspace/justfile:ro ... just {{ARGS}}

build:
    just _run build

# justfile.container
build:
    cargo build
```

just's module system (`mod examples "examples/justfile"`) composes naturally—module commands route through the container transparently.

Compared to Make, just:
- No `.PHONY` declarations
- Shell variables work naturally (`$(hostname)` vs `$$(hostname)`)
- Recipes can take arguments (`just _run build`)
- Better error messages
- Cross-platform without gnumake vs BSD make quirks

The Make version works. The just version... just works.

*(No affiliation with just—just a happy user.)*

## Honest Assessment

This pattern is better than the alternatives, but let's not oversell it. There's still duplication:

- Target names repeated in both files
- Two files to maintain instead of one
- Container orchestration logic repeated per-target (though DRY-able with variables)

It's not perfect. It's just... less bad. The duplication is mechanical rather than logical—you're not mixing concerns, just listing the same names twice. That's easier to maintain than conditional spaghetti, but it's still more than ideal.

That said, mechanical duplication is exactly the kind of work AI assistants handle well. "Add a `lint` target that runs `cargo clippy`" is a constrained, rule-following task: add it to the container file with the actual command, add a delegation stub to the host file. No judgment calls, no architectural decisions—just pattern application. If you're already using AI-assisted development, this maintenance overhead largely disappears.

If someone invents a cleaner approach, we're all ears.

## When Not to Use This

- **Simple projects**: If you don't need containerized builds, don't add complexity.
- **Uniform environments**: If all developers run the same OS with the same toolchain, containers may be overkill.
- **Single-target deployments**: If you only deploy to one platform, you might not need the isolation.

The pattern shines for polyglot projects, mixed dev teams (Linux/macOS/WSL), and CI/CD pipelines where consistency matters.

## Try It

1. Create `Makefile` with container delegation
2. Create `Makefile.container` with direct commands
3. Add the mount: `-v ./Makefile.container:/workspace/Makefile:ro`
4. Run `make build` on host and in container—same command, appropriate behavior

For full implementation details, see our [technical documentation](/tooling/container-overlay).

**[Download working example (tar.gz)](/container-overlay.tar.gz)** — includes both Make and just versions.
