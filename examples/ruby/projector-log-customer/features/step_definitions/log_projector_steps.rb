# frozen_string_literal: true

require_relative '../../lib/log_projector_logic'

# Load proto files - these are generated from the proto definitions
$LOAD_PATH.unshift(File.expand_path('../../gen', __dir__))
require 'angzarr/angzarr_pb'
require 'examples/domains_pb'

Before do
  @event = nil
  @log_entry = nil
  @processed = false
end

# --- Given steps ---

Given('a CustomerCreated event with name {string} and email {string}') do |name, email|
  event = Examples::CustomerCreated.new(name: name, email: email)
  @event = pack_event(event)
end

Given('a LoyaltyPointsAdded event with {int} points and new_balance {int}') do |points, new_balance|
  event = Examples::LoyaltyPointsAdded.new(points: points, new_balance: new_balance)
  @event = pack_event(event)
end

Given('a TransactionCreated event with customer {string} and subtotal {int}') do |customer_id, subtotal|
  # This is a transaction event - unknown to customer log projector
  event = Examples::TransactionCreated.new(customer_id: customer_id, subtotal_cents: subtotal)
  @event = pack_event(event)
end

Given('a TransactionCompleted event with total {int} and payment {string}') do |total, payment|
  event = Examples::TransactionCompleted.new(final_total_cents: total, payment_method: payment)
  @event = pack_event(event)
end

Given('an unknown event type') do
  @event = Google::Protobuf::Any.new(
    type_url: 'type.examples/examples.CompletelyUnknown',
    value: ''
  )
end

# --- When steps ---

When('I process the log projector') do
  @log_entry = LogProjectorLogic.process_event(@event)
  @processed = true
end

# --- Then steps ---

Then('the event is logged successfully') do
  expect(@log_entry).not_to be_nil
  expect(@log_entry.is_known).to be true
end

Then('the event is logged as unknown') do
  expect(@log_entry).not_to be_nil
  expect(@log_entry.is_known).to be false
end

# --- Helpers ---

def pack_event(event)
  type_name = event.class.name.split('::').last
  Google::Protobuf::Any.new(
    type_url: "type.examples/examples.#{type_name}",
    value: event.to_proto
  )
end
