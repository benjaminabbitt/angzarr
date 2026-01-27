# Infrastructure shortcuts (backing services only)
# These targets deploy ONLY backing services (databases, messaging).
# Application services are deployed via Skaffold in examples/ directories.
# For OpenTofu primitives, use: just tofu <command>

# Deploy local backing services (PostgreSQL, RabbitMQ via Helm charts)
local:
    just tofu init local
    just tofu apply-auto local

# Destroy local infrastructure
local-destroy:
    just tofu destroy-auto local

# Deploy staging infrastructure
staging:
    just tofu init staging
    just tofu apply staging

# Destroy staging infrastructure
staging-destroy:
    just tofu destroy staging

# Deploy production infrastructure
prod:
    just tofu init prod
    just tofu apply prod

# Destroy production infrastructure (requires confirmation)
prod-destroy:
    just tofu destroy prod
