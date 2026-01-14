# frozen_string_literal: true

require_relative '../../lib/receipt_projector_logic'

# Load proto files - these are generated from the proto definitions
$LOAD_PATH.unshift(File.expand_path('../../gen', __dir__))
require 'angzarr/angzarr_pb'
require 'examples/domains_pb'

Before do
  @prior_events = []
  @receipt = nil
  @formatted_text = nil
end

# --- Given steps ---

Given('a TransactionCreated event with customer {string} and subtotal {int}') do |customer_id, subtotal|
  event = Examples::TransactionCreated.new(
    customer_id: customer_id,
    items: [],
    subtotal_cents: subtotal
  )
  @prior_events << Angzarr::EventPage.new(num: @prior_events.size, event: pack_event(event))
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
  @prior_events << Angzarr::EventPage.new(num: @prior_events.size, event: pack_event(event))
end

Given('a DiscountApplied event with {int} cents discount') do |discount_cents|
  event = Examples::DiscountApplied.new(
    discount_type: 'fixed',
    value: discount_cents,
    discount_cents: discount_cents
  )
  @prior_events << Angzarr::EventPage.new(num: @prior_events.size, event: pack_event(event))
end

Given('a TransactionCompleted event with total {int} and payment {string}') do |total, payment|
  event = Examples::TransactionCompleted.new(
    final_total_cents: total,
    payment_method: payment,
    loyalty_points_earned: (total * 0.01).to_i
  )
  @prior_events << Angzarr::EventPage.new(num: @prior_events.size, event: pack_event(event))
end

Given('a TransactionCompleted event with total {int} and payment {string} earning {int} points') do |total, payment, points|
  event = Examples::TransactionCompleted.new(
    final_total_cents: total,
    payment_method: payment,
    loyalty_points_earned: points
  )
  @prior_events << Angzarr::EventPage.new(num: @prior_events.size, event: pack_event(event))
end

# --- When steps ---

When('I project the events') do
  event_book = Angzarr::EventBook.new(pages: @prior_events)
  @receipt = ReceiptProjectorLogic.project(event_book)
  @formatted_text = ReceiptProjectorLogic.format_receipt(@receipt) if @receipt
end

# --- Then steps ---

Then('no projection is generated') do
  expect(@receipt).to be_nil
end

Then('a Receipt projection is generated') do
  expect(@receipt).not_to be_nil
end

Then('the receipt has customer_id {string}') do |customer_id|
  expect(@receipt.customer_id).to eq(customer_id)
end

Then('the receipt has subtotal_cents {int}') do |subtotal|
  expect(@receipt.subtotal_cents).to eq(subtotal)
end

Then('the receipt has discount_cents {int}') do |discount|
  expect(@receipt.discount_cents).to eq(discount)
end

Then('the receipt has final_total_cents {int}') do |final_total|
  expect(@receipt.final_total_cents).to eq(final_total)
end

Then('the receipt has payment_method {string}') do |payment_method|
  expect(@receipt.payment_method).to eq(payment_method)
end

Then('the receipt has loyalty_points_earned {int}') do |points|
  expect(@receipt.loyalty_points_earned).to eq(points)
end

Then('the receipt formatted_text contains {string}') do |substring|
  expect(@formatted_text).to include(substring)
end

# --- Helpers ---

def pack_event(event)
  type_name = event.class.name.split('::').last
  Google::Protobuf::Any.new(
    type_url: "type.examples/examples.#{type_name}",
    value: event.to_proto
  )
end
