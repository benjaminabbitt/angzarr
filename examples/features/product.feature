Feature: Product Catalog Business Logic
  Tests product aggregate behavior independent of transport.
  These scenarios verify pure business logic for product lifecycle and pricing.

  # --- CreateProduct scenarios ---

  Scenario: Create a new product
    Given no prior events for the product aggregate
    When I handle a CreateProduct command with sku "SKU-001" name "Widget" description "A useful widget" and price_cents 1999
    Then the result is a ProductCreated event
    And the product event has sku "SKU-001"
    And the product event has name "Widget"
    And the product event has description "A useful widget"
    And the product event has price_cents 1999

  Scenario: Cannot create product twice
    Given a ProductCreated event with sku "SKU-002" name "Gadget" and price_cents 2999
    When I handle a CreateProduct command with sku "SKU-002" name "Gadget2" description "Another" and price_cents 3999
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already exists"

  Scenario: Creating product requires SKU
    Given no prior events for the product aggregate
    When I handle a CreateProduct command with sku "" name "NoSku" description "Missing SKU" and price_cents 1000
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "sku"

  Scenario: Creating product requires name
    Given no prior events for the product aggregate
    When I handle a CreateProduct command with sku "SKU-003" name "" description "No name" and price_cents 1000
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "name"

  Scenario: Creating product requires positive price
    Given no prior events for the product aggregate
    When I handle a CreateProduct command with sku "SKU-004" name "FreeItem" description "Zero price" and price_cents 0
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "price"

  Scenario: Cannot create product with negative price
    Given no prior events for the product aggregate
    When I handle a CreateProduct command with sku "SKU-005" name "NegativePrice" description "Invalid" and price_cents -100
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "price"

  # --- UpdateProduct scenarios ---

  Scenario: Update product name and description
    Given a ProductCreated event with sku "SKU-010" name "OldName" and price_cents 1000
    When I handle an UpdateProduct command with name "NewName" and description "New description"
    Then the result is a ProductUpdated event
    And the product event has name "NewName"
    And the product event has description "New description"

  Scenario: Cannot update non-existent product
    Given no prior events for the product aggregate
    When I handle an UpdateProduct command with name "Ghost" and description "Does not exist"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "does not exist"

  # --- SetPrice scenarios ---

  Scenario: Set new price for product
    Given a ProductCreated event with sku "SKU-020" name "Priceable" and price_cents 1000
    When I handle a SetPrice command with price_cents 1500
    Then the result is a PriceSet event
    And the product event has price_cents 1500
    And the product event has previous_price_cents 1000

  Scenario: Cannot set negative price
    Given a ProductCreated event with sku "SKU-021" name "GoodProduct" and price_cents 1000
    When I handle a SetPrice command with price_cents -500
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "price"

  Scenario: Cannot set price on non-existent product
    Given no prior events for the product aggregate
    When I handle a SetPrice command with price_cents 2000
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "does not exist"

  Scenario: Cannot set price on discontinued product
    Given a ProductCreated event with sku "SKU-022" name "Discontinued" and price_cents 1000
    And a ProductDiscontinued event
    When I handle a SetPrice command with price_cents 2000
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "discontinued"

  # --- Discontinue scenarios ---

  Scenario: Discontinue an active product
    Given a ProductCreated event with sku "SKU-030" name "ToDiscontinue" and price_cents 1000
    When I handle a Discontinue command with reason "End of life"
    Then the result is a ProductDiscontinued event
    And the product event has reason "End of life"

  Scenario: Cannot discontinue non-existent product
    Given no prior events for the product aggregate
    When I handle a Discontinue command with reason "Does not exist"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "does not exist"

  Scenario: Cannot discontinue already discontinued product
    Given a ProductCreated event with sku "SKU-031" name "AlreadyDiscontinued" and price_cents 1000
    And a ProductDiscontinued event
    When I handle a Discontinue command with reason "Already gone"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already discontinued"

  # --- State reconstruction scenarios ---

  Scenario: Rebuild state from creation and price change
    Given a ProductCreated event with sku "SKU-040" name "Stateful" and price_cents 1000
    And a PriceSet event with price_cents 1200
    When I rebuild the product state
    Then the product state has sku "SKU-040"
    And the product state has name "Stateful"
    And the product state has price_cents 1200
    And the product state has status "active"

  Scenario: Rebuild state with discontinuation
    Given a ProductCreated event with sku "SKU-041" name "ToBeDiscontinued" and price_cents 1500
    And a ProductDiscontinued event
    When I rebuild the product state
    Then the product state has status "discontinued"

  Scenario: Rebuild state with name update
    Given a ProductCreated event with sku "SKU-042" name "OldName" and price_cents 800
    And a ProductUpdated event with name "NewName" and description "Updated description"
    When I rebuild the product state
    Then the product state has name "NewName"
    And the product state has description "Updated description"
