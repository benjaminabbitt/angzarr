@csharp @cpp
Feature: CompensationContext
  When a saga or process manager issues a command that gets rejected, the framework
  sends a Notification containing rejection details. CompensationContext extracts
  this information into a developer-friendly structure.

  **Why this matters:**
  - **Debugging**: Understand which component issued the failing command
  - **Compensation logic**: Decide whether to retry, rollback, or escalate
  - **Observability**: Log structured rejection data for monitoring
  - **Business rules**: Different compensation for different rejection reasons

  Without CompensationContext, developers must manually unpack nested protobuf
  messages (Notification -> Any -> RejectionNotification -> fields), which is
  error-prone and obscures the business logic in boilerplate.

  Scenario: Extract all rejection details from notification
    # The core use case: a saga command was rejected, and the aggregate's
    # rejection handler needs to understand what happened and respond.
    Given a Notification containing a RejectionNotification with:
      | field                 | value              |
      | issuer_name           | saga-order-fulfill |
      | issuer_type           | saga               |
      | source_event_sequence | 7                  |
      | rejection_reason      | out of stock       |
    When I create a CompensationContext from the Notification
    Then the CompensationContext should have:
      | field                 | value              |
      | issuer_name           | saga-order-fulfill |
      | issuer_type           | saga               |
      | source_event_sequence | 7                  |
      | rejection_reason      | out of stock       |

  Scenario: Extract rejected command type for dispatch routing
    # Compensation handlers are often keyed by command type.
    # "If ReserveStock was rejected, release the hold."
    # "If CreateShipment was rejected, refund the payment."
    # rejected_command_type() enables this dispatch pattern.
    Given a Notification with a rejected command of type "ReserveStock"
    When I create a CompensationContext from the Notification
    Then the rejected_command_type should end with "ReserveStock"

  Scenario: Extract source aggregate for context
    # The source_aggregate tells you which aggregate triggered the saga flow.
    # Useful for:
    # - Correlating back to the original request
    # - Including context in compensation events
    # - Logging and debugging distributed flows
    Given a Notification with source_aggregate cover for domain "inventory"
    When I create a CompensationContext from the Notification
    Then the source_aggregate should have domain "inventory"

  Scenario: Handle notification without rejected command gracefully
    # Some notifications may lack the rejected command (e.g., timeout-based
    # rejections or system errors). The context must handle missing data
    # without throwing, returning null/None for optional fields.
    Given a Notification without a rejected command
    When I create a CompensationContext from the Notification
    Then rejected_command should be null
    And rejected_command_type should return null

  Scenario: Handle empty notification payload gracefully
    # Edge case: malformed or minimal notifications should not crash.
    # All fields default to empty/zero values, enabling null-safe access.
    Given a Notification with empty payload
    When I create a CompensationContext from the Notification
    Then all fields should have default/empty values
