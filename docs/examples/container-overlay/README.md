# Container Overlay Pattern Demo

Demonstrates the container overlay pattern where the same command works on both host and inside containers.

Includes both **Make** and **just** examples.

## How It Works

```
Host:      make build → starts container → mounts overlay → runs make build inside
Container: make build → runs commands directly (Makefile.container IS Makefile)
```

The key is this mount: `-v ./Makefile.container:/workspace/Makefile:ro`

## Files

**Make version:**
- `Makefile` - Host version, delegates to container
- `Makefile.container` - Container version, runs commands directly
- `Containerfile` - Alpine + make (~9MB)

**just version:**
- `justfile` - Host version, delegates to container
- `justfile.container` - Container version, runs commands directly
- `Containerfile.just` - Alpine + just (~12MB)

## Usage

**Make:**
```bash
make demo
make build
make test
make info
make clean
```

**just:**
```bash
just demo
just build
just test
just info
just clean
```

## Output

```
=== Host Environment ===
Hostname: my-laptop
This justfile: justfile (host version)

=== Running 'just info' (delegates to container) ===

=== Container Environment ===
This justfile is: justfile.container (mounted as justfile)
Hostname: a1b2c3d4e5f6
NAME="Alpine Linux"
```

Same command, different context. No conditionals. Just works.

## Why just?

The [just](https://github.com/casey/just) examples are notably cleaner:

- No `.PHONY` declarations required
- Shell variables work naturally (`$(hostname)` vs `$$(hostname)`)
- Recipes can take arguments (`just _run build`)
- Better error messages
- Cross-platform without gnumake vs BSD make issues

The Make examples work, but just... just works.

*(No affiliation with just—just a happy user.)*
