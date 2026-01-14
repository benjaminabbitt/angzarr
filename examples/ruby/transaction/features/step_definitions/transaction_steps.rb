# frozen_string_literal: true

require_relative '../../lib/transaction_logic'
require_relative '../../lib/command_validation_error'

# Load proto files - these are generated from the proto definitions
$LOAD_PATH.unshift(File.expand_path('../../gen', __dir__))
require 'angzarr/angzarr_pb'
require 'examples/domains_pb'

Before do
  @prior_events = []
  @result_event = nil
  @error = nil
  @state = nil
  @pending_items = []
end

# --- Given steps ---

Given('no prior events for the aggregate') do
  @prior_events = []
end

Given('a TransactionCreated event with customer {string} and subtotal {int}') do |customer_id, subtotal|
  event = Examples::TransactionCreated.new(
    customer_id: customer_id,
    items: [],
    subtotal_cents: subtotal
  )
  @prior_events << Google::Protobuf::Any.pack(event)
end

Given('a TransactionCreated event with customer {string} and items:') do |customer_id, table|
  items = table.hashes.map do |row|
    Examples::LineItem.new(
      product_id: row['product_id'],
      name: row['name'],
      quantity: row['quantity'].to_i,
      unit_price_cents: row['unit_price_cents'].to_i
    )
  end
  subtotal = items.sum { |item| item.quantity * item.unit_price_cents }
  event = Examples::TransactionCreated.new(
    customer_id: customer_id,
    items: items,
    subtotal_cents: subtotal
  )
  @prior_events << Google::Protobuf::Any.pack(event)
end

Given('a TransactionCompleted event') do
  event = Examples::TransactionCompleted.new(
    final_total_cents: 0,
    payment_method: 'card'
  )
  @prior_events << Google::Protobuf::Any.pack(event)
end

Given('a DiscountApplied event with {int} cents discount') do |discount_cents|
  event = Examples::DiscountApplied.new(
    discount_type: 'fixed',
    value: discount_cents,
    discount_cents: discount_cents
  )
  @prior_events << Google::Protobuf::Any.pack(event)
end

# --- When steps ---

When('I handle a CreateTransaction command with customer {string} and items:') do |customer_id, table|
  items = table.hashes.map do |row|
    Examples::LineItem.new(
      product_id: row['product_id'],
      name: row['name'],
      quantity: row['quantity'].to_i,
      unit_price_cents: row['unit_price_cents'].to_i
    )
  end

  event_book = build_event_book
  @state = TransactionLogic.rebuild_state(event_book)
  begin
    @result_event = TransactionLogic.handle_create_transaction(@state, customer_id, items)
    @error = nil
  rescue CommandValidationError => e
    @error = e
    @result_event = nil
  end
end

When('I handle a CreateTransaction command with customer {string} and no items') do |customer_id|
  event_book = build_event_book
  @state = TransactionLogic.rebuild_state(event_book)
  begin
    @result_event = TransactionLogic.handle_create_transaction(@state, customer_id, [])
    @error = nil
  rescue CommandValidationError => e
    @error = e
    @result_event = nil
  end
end

When('I handle an ApplyDiscount command with type {string} and value {int}') do |discount_type, value|
  event_book = build_event_book
  @state = TransactionLogic.rebuild_state(event_book)
  begin
    @result_event = TransactionLogic.handle_apply_discount(@state, discount_type, value)
    @error = nil
  rescue CommandValidationError => e
    @error = e
    @result_event = nil
  end
end

When('I handle a CompleteTransaction command with payment method {string}') do |payment_method|
  event_book = build_event_book
  @state = TransactionLogic.rebuild_state(event_book)
  begin
    @result_event = TransactionLogic.handle_complete_transaction(@state, payment_method)
    @error = nil
  rescue CommandValidationError => e
    @error = e
    @result_event = nil
  end
end

When('I handle a CancelTransaction command with reason {string}') do |reason|
  event_book = build_event_book
  @state = TransactionLogic.rebuild_state(event_book)
  begin
    @result_event = TransactionLogic.handle_cancel_transaction(@state, reason)
    @error = nil
  rescue CommandValidationError => e
    @error = e
    @result_event = nil
  end
end

When('I rebuild the transaction state') do
  event_book = build_event_book
  @state = TransactionLogic.rebuild_state(event_book)
end

# --- Then steps ---

Then('the result is a TransactionCreated event') do
  expect(@result_event).not_to be_nil, "Expected result but got error: #{@error&.message}"
  expect(@result_event).to be_a(Examples::TransactionCreated)
end

Then('the result is a DiscountApplied event') do
  expect(@result_event).not_to be_nil, "Expected result but got error: #{@error&.message}"
  expect(@result_event).to be_a(Examples::DiscountApplied)
end

Then('the result is a TransactionCompleted event') do
  expect(@result_event).not_to be_nil, "Expected result but got error: #{@error&.message}"
  expect(@result_event).to be_a(Examples::TransactionCompleted)
end

Then('the result is a TransactionCancelled event') do
  expect(@result_event).not_to be_nil, "Expected result but got error: #{@error&.message}"
  expect(@result_event).to be_a(Examples::TransactionCancelled)
end

Then('the command fails with status {string}') do |status_name|
  expect(@error).not_to be_nil, 'Expected command to fail but it succeeded'
  expected_code = status_name.downcase.to_sym
  expect(@error.status_code).to eq(expected_code), "Expected status #{status_name}, got #{@error.status_code}"
end

Then('the error message contains {string}') do |substring|
  expect(@error).not_to be_nil, 'Expected error but command succeeded'
  expect(@error.message.downcase).to include(substring.downcase)
end

Then('the event has customer_id {string}') do |customer_id|
  expect(@result_event.customer_id).to eq(customer_id)
end

Then('the event has subtotal_cents {int}') do |subtotal|
  expect(@result_event.subtotal_cents).to eq(subtotal)
end

Then('the event has discount_cents {int}') do |discount_cents|
  expect(@result_event.discount_cents).to eq(discount_cents)
end

Then('the event has final_total_cents {int}') do |final_total|
  expect(@result_event.final_total_cents).to eq(final_total)
end

Then('the event has payment_method {string}') do |payment_method|
  expect(@result_event.payment_method).to eq(payment_method)
end

Then('the event has loyalty_points_earned {int}') do |points|
  expect(@result_event.loyalty_points_earned).to eq(points)
end

Then('the event has reason {string}') do |reason|
  expect(@result_event.reason).to eq(reason)
end

Then('the state has customer_id {string}') do |customer_id|
  expect(@state).not_to be_nil
  expect(@state.customer_id).to eq(customer_id)
end

Then('the state has subtotal_cents {int}') do |subtotal|
  expect(@state).not_to be_nil
  expect(@state.subtotal_cents).to eq(subtotal)
end

Then('the state has status {string}') do |status|
  expect(@state).not_to be_nil
  expect(@state.status).to eq(status)
end

# --- Helpers ---

def build_event_book
  return nil if @prior_events.empty?

  pages = @prior_events.each_with_index.map do |event, index|
    Angzarr::EventPage.new(num: index, event: event)
  end

  Angzarr::EventBook.new(pages: pages)
end
