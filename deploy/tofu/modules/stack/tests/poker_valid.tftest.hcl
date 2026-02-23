# Valid Poker Stack Test
# Tests a correctly configured poker domain topology

variables {
  name = "poker"

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

  # Poker domains: player, table, hand
  # player is an ENTRY POINT (no inbound sagas) - receives external commands
  # table receives saga from hand, is also an entry point for CreateTable
  # hand receives saga from table
  domains = {
    # Player is a pure entry point - no other domain sends commands to it via saga
    # External API calls: RegisterPlayer, DepositFunds, WithdrawFunds
    player = {
      aggregate = {
        image = "ghcr.io/poker/player-aggregate:latest"
      }
      # Player emits events that table saga consumes (but no inbound sagas to player)
    }

    # Table receives commands from hand saga when hand completes
    table = {
      aggregate = {
        image = "ghcr.io/poker/table-aggregate:latest"
      }
      sagas = {
        # Table events trigger hand creation
        hand = {
          target_domain = "hand"
          image         = "ghcr.io/poker/saga-table-hand:latest"
        }
      }
    }

    # Hand receives commands from table saga to start hands
    # Hand emits events consumed by PM for payouts
    hand = {
      aggregate = {
        image = "ghcr.io/poker/hand-aggregate:latest"
      }
      sagas = {
        # Hand completion triggers table state update
        table = {
          target_domain = "table"
          image         = "ghcr.io/poker/saga-hand-table:latest"
        }
      }
    }
  }

  # Process manager for cross-domain orchestration
  # Subscribes to player events for seat management
  # Targets table to update seating state
  process_managers = {
    seating = {
      image         = "ghcr.io/poker/pm-seating:latest"
      subscriptions = ["player", "table"]
      targets       = ["table"]
    }
  }
}

# Test that the valid configuration passes all checks
run "valid_poker_stack" {
  command = plan

  # Verify entry points - player has no inbound sagas so it's an entry point
  assert {
    condition     = length(output.entry_points) > 0
    error_message = "Should have at least one entry point"
  }

  assert {
    condition     = contains(output.entry_points, "player")
    error_message = "Player should be an entry point (no inbound sagas)"
  }

  # Verify rust features are aggregated from storage and bus
  assert {
    condition     = length(output.rust_features) > 0
    error_message = "Should have rust features from storage/bus"
  }

  assert {
    condition     = contains(output.rust_features, "postgres")
    error_message = "Should require postgres rust feature"
  }

  assert {
    condition     = contains(output.rust_features, "amqp")
    error_message = "Should require amqp rust feature"
  }

  # Verify all domains are tracked
  assert {
    condition     = length(keys(output.domains)) == 3
    error_message = "Should have 3 domains: player, table, hand"
  }

  # Verify process manager is tracked
  assert {
    condition     = length(keys(output.process_managers)) == 1
    error_message = "Should have 1 process manager: seating"
  }
}

# Test the mermaid diagram is generated
run "generates_mermaid" {
  command = plan

  assert {
    condition     = output.topology_mermaid != ""
    error_message = "Should generate mermaid diagram"
  }

  assert {
    condition     = can(regex("flowchart LR", output.topology_mermaid))
    error_message = "Mermaid should start with 'flowchart LR'"
  }

  assert {
    condition     = can(regex("player\\[", output.topology_mermaid))
    error_message = "Mermaid should include player domain"
  }

  assert {
    condition     = can(regex("table\\[", output.topology_mermaid))
    error_message = "Mermaid should include table domain"
  }

  assert {
    condition     = can(regex("hand\\[", output.topology_mermaid))
    error_message = "Mermaid should include hand domain"
  }
}
