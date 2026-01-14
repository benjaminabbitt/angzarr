# frozen_string_literal: true

require_relative '../../lib/loyalty_saga_logic'

# Load proto files - these are generated from the proto definitions
$LOAD_PATH.unshift(File.expand_path('../../gen', __dir__))
require 'angzarr/angzarr_pb'
require 'examples/domains_pb'

Before do
  @prior_events = []
  @commands = []
end

# --- Given steps ---

Given('a TransactionCreated event with customer {string} and subtotal {int}') do |customer_id, subtotal|
  event = Examples::TransactionCreated.new(
    customer_id: customer_id,
    subtotal_cents: subtotal
  )
  @prior_events << Angzarr::EventPage.new(num: @prior_events.size, event: pack_event(event))
end

Given('a TransactionCompleted event with {int} loyalty points earned') do |points|
  event = Examples::TransactionCompleted.new(
    final_total_cents: points * 100, # Approximate original total
    payment_method: 'card',
    loyalty_points_earned: points
  )
  @prior_events << Angzarr::EventPage.new(num: @prior_events.size, event: pack_event(event))
end

# --- When steps ---

When('I process the saga') do
  event_book = Angzarr::EventBook.new(
    cover: Angzarr::Cover.new(domain: 'transaction', id: 'txn-test-001'),
    pages: @prior_events
  )
  @commands = LoyaltySagaLogic.process(event_book, transaction_id: 'txn-test-001')
end

# --- Then steps ---

Then('no commands are generated') do
  expect(@commands).to be_empty
end

Then('an AddLoyaltyPoints command is generated') do
  expect(@commands.size).to eq(1)
  expect(@commands[0].command_type).to eq('AddLoyaltyPoints')
end

Then('the command has points {int}') do |points|
  expect(@commands[0].points).to eq(points)
end

Then('the command has domain {string}') do |domain|
  expect(@commands[0].domain).to eq(domain)
end

Then('the command reason contains {string}') do |substring|
  expect(@commands[0].reason.downcase).to include(substring.downcase)
end

# --- Helpers ---

def pack_event(event)
  type_name = event.class.name.split('::').last
  Google::Protobuf::Any.new(
    type_url: "type.examples/examples.#{type_name}",
    value: event.to_proto
  )
end
