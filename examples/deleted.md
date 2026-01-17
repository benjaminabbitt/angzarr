# Deleted Example Implementations

The following example language implementations were removed from the working tree to reduce maintenance burden. History is preserved in git.

## Deleted Languages

- C++ (`cpp/`)
- C# (`csharp/`)
- Elixir (`elixir/`)
- Java (`java/`)
- Kotlin (`kotlin/`)
- Ruby (`ruby/`)
- TypeScript (`typescript/`)

## Active Languages

- Go (`go/`)
- Python (`python/`)
- Rust (`rust/`)

## Recovery

To recover a deleted language implementation:

```bash
# Restore a single directory from the last commit before deletion
git checkout e9eb43f -- examples/<language>/

# Examples:
git checkout e9eb43f -- examples/cpp/
git checkout e9eb43f -- examples/csharp/
git checkout e9eb43f -- examples/elixir/
git checkout e9eb43f -- examples/java/
git checkout e9eb43f -- examples/kotlin/
git checkout e9eb43f -- examples/ruby/
git checkout e9eb43f -- examples/typescript/
```

To view the deleted files without restoring:

```bash
git show e9eb43f:examples/<language>/
```

To restore all deleted examples at once:

```bash
git checkout e9eb43f -- examples/cpp/ examples/csharp/ examples/elixir/ examples/java/ examples/kotlin/ examples/ruby/ examples/typescript/
```
