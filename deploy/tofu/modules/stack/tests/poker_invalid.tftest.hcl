# Invalid Poker Stack Tests
# Tests that validation correctly catches misconfigurations

# Mock providers to avoid needing real cloud credentials
mock_provider "aws" {}
mock_provider "google" {}

# Common variables for all tests
variables {
  name = "poker-invalid"

  compute = {
    compute_type = "kubernetes"
    namespace    = "poker"
  }

  bus = {
    type           = "rabbit"
    connection_uri = "amqp://guest:guest@rabbitmq:5672"
    provides = {
      # event_bus is required capability
      capabilities  = ["event_bus", "pub_sub", "topic_routing"]
      rust_features = ["amqp"]
    }
  }

  default_storage = {
    event_store = {
      connection_uri = "postgres://postgres:postgres@postgres:5432/poker"
      provides = {
        capabilities  = ["event_store", "position_store", "transactions"]
        rust_features = ["postgres"]
      }
    }
    position_store = {
      connection_uri = "postgres://postgres:postgres@postgres:5432/poker"
      provides = {
        capabilities  = ["position_store", "transactions"]
        rust_features = ["postgres"]
      }
    }
    snapshot_store = null
  }

  coordinator_images = {
    aggregate    = "ghcr.io/angzarr/aggregate:latest"
    saga         = "ghcr.io/angzarr/saga:latest"
    projector    = "ghcr.io/angzarr/projector:latest"
    pm           = "ghcr.io/angzarr/pm:latest"
    grpc_gateway = null
  }

  domains          = {}
  process_managers = {}
}

#------------------------------------------------------------------------------
# Test: Saga targets non-existent domain
#------------------------------------------------------------------------------

run "saga_targets_nonexistent_domain" {
  command = plan

  variables {
    domains = {
      player = {
        aggregate = {
          image = "ghcr.io/poker/player-aggregate:latest"
        }
        sagas = {
          # ERROR: 'betting' domain doesn't exist
          betting = {
            target_domain = "betting"
            image         = "ghcr.io/poker/saga-player-betting:latest"
          }
        }
      }
    }
  }

  expect_failures = [
    check.saga_targets_exist,
  ]
}

#------------------------------------------------------------------------------
# Test: PM subscribes to non-existent domain
#------------------------------------------------------------------------------

run "pm_subscribes_to_nonexistent_domain" {
  command = plan

  variables {
    domains = {
      # entry_point domain (not targeted by PM, marked standalone)
      entry = {
        standalone = true
        aggregate = {
          image = "ghcr.io/poker/entry-aggregate:latest"
        }
      }
      player = {
        aggregate = {
          image = "ghcr.io/poker/player-aggregate:latest"
        }
      }
    }
    process_managers = {
      bad-pm = {
        image = "ghcr.io/poker/pm-bad:latest"
        # ERROR: 'tournament' domain doesn't exist
        subscriptions = ["player", "tournament"]
        targets       = ["player"]
      }
    }
  }

  expect_failures = [
    check.pm_subscriptions_valid,
  ]
}

#------------------------------------------------------------------------------
# Test: PM targets non-existent domain
#------------------------------------------------------------------------------

run "pm_targets_nonexistent_domain" {
  command = plan

  variables {
    domains = {
      player = {
        aggregate = {
          image = "ghcr.io/poker/player-aggregate:latest"
        }
      }
    }
    process_managers = {
      bad-pm = {
        image         = "ghcr.io/poker/pm-bad:latest"
        subscriptions = ["player"]
        # ERROR: 'leaderboard' domain doesn't exist
        targets = ["leaderboard"]
      }
    }
  }

  expect_failures = [
    check.pm_targets_valid,
  ]
}

#------------------------------------------------------------------------------
# Test: PM has no subscriptions
#------------------------------------------------------------------------------

run "pm_missing_subscriptions" {
  command = plan

  variables {
    domains = {
      # entry point domain (not targeted by PM, marked standalone)
      entry = {
        standalone = true
        aggregate = {
          image = "ghcr.io/poker/entry-aggregate:latest"
        }
      }
      player = {
        aggregate = {
          image = "ghcr.io/poker/player-aggregate:latest"
        }
      }
    }
    process_managers = {
      empty-pm = {
        image = "ghcr.io/poker/pm-empty:latest"
        # ERROR: Empty subscriptions
        subscriptions = []
        targets       = ["player"]
      }
    }
  }

  expect_failures = [
    check.pm_has_subscriptions,
  ]
}

#------------------------------------------------------------------------------
# Test: PM has no targets
#------------------------------------------------------------------------------

run "pm_missing_targets" {
  command = plan

  variables {
    domains = {
      player = {
        aggregate = {
          image = "ghcr.io/poker/player-aggregate:latest"
        }
      }
    }
    process_managers = {
      empty-pm = {
        image         = "ghcr.io/poker/pm-empty:latest"
        subscriptions = ["player"]
        # ERROR: Empty targets
        targets = []
      }
    }
  }

  expect_failures = [
    check.pm_has_targets,
  ]
}

#------------------------------------------------------------------------------
# Test: Orphan domain (not connected and not standalone)
#------------------------------------------------------------------------------

run "orphan_domain_not_connected" {
  command = plan

  variables {
    domains = {
      # Player is an entry point
      player = {
        aggregate = {
          image = "ghcr.io/poker/player-aggregate:latest"
        }
      }
      # ERROR: 'audit' is not connected to anything and not marked standalone
      audit = {
        aggregate = {
          image = "ghcr.io/poker/audit-aggregate:latest"
        }
      }
    }
  }

  expect_failures = [
    check.no_orphan_domains,
  ]
}

#------------------------------------------------------------------------------
# Test: Domain/PM name collision
#------------------------------------------------------------------------------

run "domain_pm_name_collision" {
  command = plan

  variables {
    domains = {
      # entry point (not targeted by anything)
      entry = {
        aggregate = {
          image = "ghcr.io/poker/entry-aggregate:latest"
        }
        sagas = {
          table = {
            target_domain = "table"
            image         = "ghcr.io/poker/saga-entry-table:latest"
          }
        }
      }
      table = {
        aggregate = {
          image = "ghcr.io/poker/table-aggregate:latest"
        }
      }
    }
    process_managers = {
      # ERROR: 'table' collides with domain name
      table = {
        image         = "ghcr.io/poker/pm-table:latest"
        subscriptions = ["entry"]
        targets       = ["table"]
      }
    }
  }

  expect_failures = [
    check.no_domain_pm_name_collision,
  ]
}

#------------------------------------------------------------------------------
# Test: No entry points (fully circular)
#------------------------------------------------------------------------------

run "no_entry_points_circular" {
  command = plan

  variables {
    domains = {
      # Every domain has inbound sagas - no external entry point
      a = {
        aggregate = {
          image = "ghcr.io/poker/a-aggregate:latest"
        }
        sagas = {
          to-b = {
            target_domain = "b"
            image         = "ghcr.io/poker/saga-a-b:latest"
          }
        }
      }
      b = {
        aggregate = {
          image = "ghcr.io/poker/b-aggregate:latest"
        }
        sagas = {
          to-c = {
            target_domain = "c"
            image         = "ghcr.io/poker/saga-b-c:latest"
          }
        }
      }
      c = {
        aggregate = {
          image = "ghcr.io/poker/c-aggregate:latest"
        }
        sagas = {
          to-a = {
            target_domain = "a"
            image         = "ghcr.io/poker/saga-c-a:latest"
          }
        }
      }
    }
  }

  expect_failures = [
    check.has_entry_points,
  ]
}

#------------------------------------------------------------------------------
# Test: Invalid event store capability
#------------------------------------------------------------------------------

run "invalid_event_store_capability" {
  command = plan

  variables {
    default_storage = {
      event_store = {
        connection_uri = "redis://redis:6379"
        provides = {
          # ERROR: Missing 'event_store' capability
          capabilities  = ["caching", "fast_reads"]
          rust_features = ["redis"]
        }
      }
      position_store = {
        connection_uri = "postgres://postgres:postgres@postgres:5432/poker"
        provides = {
          capabilities  = ["position_store", "transactions"]
          rust_features = ["postgres"]
        }
      }
      snapshot_store = null
    }
    domains = {
      player = {
        standalone = true
        aggregate = {
          image = "ghcr.io/poker/player-aggregate:latest"
        }
      }
    }
  }

  # Variable validation catches this before check block
  expect_failures = [
    var.default_storage,
  ]
}

#------------------------------------------------------------------------------
# Test: Invalid position store capability
#------------------------------------------------------------------------------

run "invalid_position_store_capability" {
  command = plan

  variables {
    default_storage = {
      event_store = {
        connection_uri = "postgres://postgres:postgres@postgres:5432/poker"
        provides = {
          capabilities  = ["event_store", "transactions"]
          rust_features = ["postgres"]
        }
      }
      position_store = {
        connection_uri = "redis://redis:6379"
        provides = {
          # ERROR: Missing 'position_store' capability
          capabilities  = ["caching"]
          rust_features = ["redis"]
        }
      }
      snapshot_store = null
    }
    domains = {
      player = {
        standalone = true
        aggregate = {
          image = "ghcr.io/poker/player-aggregate:latest"
        }
      }
    }
  }

  # Variable validation catches this before check block
  expect_failures = [
    var.default_storage,
  ]
}
