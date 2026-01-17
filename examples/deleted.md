# Deleted Example Implementations

The following example language implementations were removed from the working tree to reduce maintenance burden. History is preserved in git.  Note that these are still planned to be supported in the future and there's no explicit reason why they wouldn't work at the time of this writing, but focus on the core languages first.

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

## Related Helm Values Files

The following Helm values files were also removed (they configured deployments for deleted languages):

```bash
# Restore helm values if needed
git checkout e9eb43f -- deploy/helm/angzarr/values-cpp.yaml
git checkout e9eb43f -- deploy/helm/angzarr/values-csharp.yaml
git checkout e9eb43f -- deploy/helm/angzarr/values-elixir.yaml
git checkout e9eb43f -- deploy/helm/angzarr/values-java.yaml
git checkout e9eb43f -- deploy/helm/angzarr/values-kotlin.yaml
git checkout e9eb43f -- deploy/helm/angzarr/values-ruby.yaml
git checkout e9eb43f -- deploy/helm/angzarr/values-typescript.yaml
```
