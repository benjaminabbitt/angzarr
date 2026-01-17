<!-- SCM:BEGIN -->
@.scm/context.md
<!-- SCM:END -->


## Tooling
### Helm
Use helm for all deployments.  Do not use kustomize.

### Python's Role
Python is to be used for support files and general scripting.  Things like manage secrets, initializing a registry, and waiting for grpc health checks.  The author prefers python for this role over shell.

### Skaffold
Use skaffold for all deployments. (this uses helm under the hood)

## Examples Projects
Examples for many common languages are provided.  This should encompass the vast majority of general purpose software development.

Each example directory should be largely self sufficient and know how to build and deploy itself.  A few exceptions:
1) They'll all require the angzarr base binaries/images.  They're implementing an angzarr application.
2) The gherkin files themselves are in the examples directory.  They are kept out of the language specific directories because they are applicable to all languages and should be kept DRY.  They're business speak.

## Testing
Three levels of testing:

* Unit tests

Angzarr core and examples.  Run in dev containers, isolated from the rest of the system.

* Integration tests

Angzarr only, run against the real cluster.  Primarily technical to prove out the angzarr system.

* Acceptance tests

Examples only, runs against the real cluster.  Uses cucumber/gherkin language to describe business requirements.  Same gherkin files are used for all languages/examples.
k,