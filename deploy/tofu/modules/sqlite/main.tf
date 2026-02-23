# SQLite Module - Main
# Local/embedded storage for development and testing
#
# This module does not create any cloud resources - SQLite is an embedded
# database that runs within the application process. This module provides
# the standard interface for configuration consistency.

terraform {
  required_version = ">= 1.0"
}

# No resources - SQLite is embedded in the application
# This module exists purely for interface consistency
