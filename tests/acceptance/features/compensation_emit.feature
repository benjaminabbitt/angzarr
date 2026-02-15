@compensation
Feature: Compensation Flow - Framework Emits Notification
  When a saga/PM-issued command is rejected by the target aggregate,
  the framework creates a Notification with RejectionNotification payload
  and routes it to the appropriate components for compensation.

  Background:
    Given the angzarr framework is initialized

  # ============================================================================
  # Scenario: Saga-issued command rejected
  # ============================================================================

  @emit @saga
  Scenario: Framework creates Notification when saga command is rejected
    Given a Player aggregate with FundsReserved event
    And a PaymentSaga that reacts to FundsReserved by issuing ProcessPayment
    And the Payment aggregate rejects ProcessPayment with "insufficient_balance"
    When the framework processes the rejection
    Then a Notification is created
    And the notification has issuer_name "saga-payment"
    And the notification has rejection_reason "insufficient_balance"
    And the notification contains the rejected ProcessPayment command

  @emit @saga
  Scenario: Framework routes Notification to source aggregate after saga rejection
    Given a Player aggregate with FundsReserved event
    And a PaymentSaga that reacts to FundsReserved by issuing ProcessPayment
    And the Payment aggregate rejects ProcessPayment
    When the framework routes the rejection
    Then the Player aggregate receives the Notification
    And the notification has source_event_type "FundsReserved"

  @emit @saga
  Scenario: Notification carries full rejected command context
    Given a saga command with:
      | domain       | player         |
      | root         | player-123     |
      | correlation  | corr-456       |
      | command_type | ProcessPayment |
      | amount       | 100            |
    When the command is rejected with reason "card_declined"
    Then the Notification rejected_command contains:
      | field            | value          |
      | cover.domain     | player         |
      | cover.root       | player-123     |
      | command_type     | ProcessPayment |
    And the rejection_reason is "card_declined"

  # ============================================================================
  # Scenario: PM-issued command rejected
  # ============================================================================

  @emit @pm
  Scenario: Framework creates Notification when PM command is rejected
    Given an Order aggregate with OrderCreated event
    And an OrderWorkflowPM that reacts to OrderCreated by issuing ReserveInventory
    And the Inventory aggregate rejects ReserveInventory with "out_of_stock"
    When the framework processes the rejection
    Then a Notification is created
    And the notification has issuer_name "pmg-order-workflow"
    And the notification has rejection_reason "out_of_stock"

  @emit @pm
  Scenario: Framework routes Notification to PM first then source aggregate
    Given an Order aggregate with OrderCreated event
    And an OrderWorkflowPM that reacts to OrderCreated by issuing ReserveInventory
    And the Inventory aggregate rejects ReserveInventory
    When the framework routes the rejection
    Then the OrderWorkflowPM receives the Notification first
    And then the Order aggregate receives the Notification

  @emit @pm
  Scenario: PM rejection includes process manager context
    Given an OrderWorkflowPM in state:
      | order_id     | order-789            |
      | step         | awaiting_inventory   |
      | attempts     | 1                    |
    And the PM issues ReserveInventory which is rejected
    When the framework creates the Notification
    Then the notification can be matched to the PM's correlation_id
    And the PM can access its state when handling rejection

  # ============================================================================
  # Scenario: Rejection details propagation
  # ============================================================================

  @emit
  Scenario: FAILED_PRECONDITION status triggers compensation flow
    Given a command sent to an aggregate
    When the aggregate returns gRPC status FAILED_PRECONDITION
    Then the framework initiates compensation flow
    And a Notification is created

  @emit
  Scenario: Other gRPC errors do not trigger compensation
    Given a command sent to an aggregate
    When the aggregate returns gRPC status INVALID_ARGUMENT
    Then the framework does not create a Notification
    And the error is returned to the caller

  @emit
  Scenario: Notification preserves full provenance chain
    Given an event chain:
      | step | component      | action              | domain    |
      | 1    | OrderAggregate | emits OrderCreated  | order     |
      | 2    | OrderWorkflowPM| issues ReserveInventory | inventory |
      | 3    | InventoryAgg   | rejects command     | inventory |
    When the framework creates the Notification
    Then the notification includes:
      | field               | value               |
      | source_event_type   | OrderCreated        |
      | rejected_command    | ReserveInventory    |
      | issuer_type         | process_manager     |
      | issuer_name         | pmg-order-workflow  |

  # ============================================================================
  # Scenario: Edge cases
  # ============================================================================

  @emit @edge
  Scenario: Multiple commands in saga step - first rejection stops chain
    Given a saga that issues multiple commands:
      | command           | target_domain |
      | ReserveInventory  | inventory     |
      | ChargeCreditCard  | payment       |
    When ReserveInventory is rejected
    Then ChargeCreditCard is not sent
    And only one Notification is created

  @emit @edge
  Scenario: Nested PM chain rejection bubbles up
    Given a PM chain:
      | pm                | issues           | to_domain   |
      | FulfillmentPM     | ShipOrder        | shipping    |
      | ShippingPM        | BookCarrier      | carrier     |
    When BookCarrier is rejected by Carrier aggregate
    Then ShippingPM receives Notification
    And then FulfillmentPM receives Notification
    And finally source aggregate receives Notification
