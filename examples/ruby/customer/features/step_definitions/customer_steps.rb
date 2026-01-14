# frozen_string_literal: true

require_relative '../../lib/customer_logic'
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
end

# --- Given steps ---

Given('no prior events for the aggregate') do
  @prior_events = []
end

Given('a CustomerCreated event with name {string} and email {string}') do |name, email|
  event = Examples::CustomerCreated.new(name: name, email: email)
  @prior_events << Google::Protobuf::Any.pack(event)
end

Given('a LoyaltyPointsAdded event with {int} points and new_balance {int}') do |points, new_balance|
  event = Examples::LoyaltyPointsAdded.new(points: points, new_balance: new_balance)
  @prior_events << Google::Protobuf::Any.pack(event)
end

Given('a LoyaltyPointsRedeemed event with {int} points and new_balance {int}') do |points, new_balance|
  event = Examples::LoyaltyPointsRedeemed.new(points: points, new_balance: new_balance)
  @prior_events << Google::Protobuf::Any.pack(event)
end

# --- When steps ---

When('I handle a CreateCustomer command with name {string} and email {string}') do |name, email|
  event_book = build_event_book
  @state = CustomerLogic.rebuild_state(event_book)
  begin
    @result_event = CustomerLogic.handle_create_customer(@state, name, email)
    @error = nil
  rescue CommandValidationError => e
    @error = e
    @result_event = nil
  end
end

When('I handle an AddLoyaltyPoints command with {int} points and reason {string}') do |points, reason|
  event_book = build_event_book
  @state = CustomerLogic.rebuild_state(event_book)
  begin
    @result_event = CustomerLogic.handle_add_loyalty_points(@state, points, reason)
    @error = nil
  rescue CommandValidationError => e
    @error = e
    @result_event = nil
  end
end

When('I handle a RedeemLoyaltyPoints command with {int} points and type {string}') do |points, redemption_type|
  event_book = build_event_book
  @state = CustomerLogic.rebuild_state(event_book)
  begin
    @result_event = CustomerLogic.handle_redeem_loyalty_points(@state, points, redemption_type)
    @error = nil
  rescue CommandValidationError => e
    @error = e
    @result_event = nil
  end
end

When('I rebuild the customer state') do
  event_book = build_event_book
  @state = CustomerLogic.rebuild_state(event_book)
end

# --- Then steps ---

Then('the result is a CustomerCreated event') do
  expect(@result_event).not_to be_nil, "Expected result but got error: #{@error&.message}"
  expect(@result_event).to be_a(Examples::CustomerCreated)
end

Then('the result is a LoyaltyPointsAdded event') do
  expect(@result_event).not_to be_nil, "Expected result but got error: #{@error&.message}"
  expect(@result_event).to be_a(Examples::LoyaltyPointsAdded)
end

Then('the result is a LoyaltyPointsRedeemed event') do
  expect(@result_event).not_to be_nil, "Expected result but got error: #{@error&.message}"
  expect(@result_event).to be_a(Examples::LoyaltyPointsRedeemed)
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

Then('the event has name {string}') do |name|
  expect(@result_event).to be_a(Examples::CustomerCreated)
  expect(@result_event.name).to eq(name)
end

Then('the event has email {string}') do |email|
  expect(@result_event).to be_a(Examples::CustomerCreated)
  expect(@result_event.email).to eq(email)
end

Then('the event has points {int}') do |points|
  actual_points = case @result_event
                  when Examples::LoyaltyPointsAdded, Examples::LoyaltyPointsRedeemed
                    @result_event.points
                  else
                    raise "Event is not a points event: #{@result_event.class}"
                  end
  expect(actual_points).to eq(points)
end

Then('the event has new_balance {int}') do |new_balance|
  actual_balance = case @result_event
                   when Examples::LoyaltyPointsAdded, Examples::LoyaltyPointsRedeemed
                     @result_event.new_balance
                   else
                     raise "Event is not a points event: #{@result_event.class}"
                   end
  expect(actual_balance).to eq(new_balance)
end

Then('the event has reason {string}') do |reason|
  expect(@result_event).to be_a(Examples::LoyaltyPointsAdded)
  expect(@result_event.reason).to eq(reason)
end

Then('the event has redemption_type {string}') do |redemption_type|
  expect(@result_event).to be_a(Examples::LoyaltyPointsRedeemed)
  expect(@result_event.redemption_type).to eq(redemption_type)
end

Then('the state has name {string}') do |name|
  expect(@state).not_to be_nil
  expect(@state.name).to eq(name)
end

Then('the state has email {string}') do |email|
  expect(@state).not_to be_nil
  expect(@state.email).to eq(email)
end

Then('the state has loyalty_points {int}') do |points|
  expect(@state).not_to be_nil
  expect(@state.loyalty_points).to eq(points)
end

Then('the state has lifetime_points {int}') do |points|
  expect(@state).not_to be_nil
  expect(@state.lifetime_points).to eq(points)
end

# --- Helpers ---

def build_event_book
  return nil if @prior_events.empty?

  pages = @prior_events.each_with_index.map do |event, index|
    Angzarr::EventPage.new(num: index, event: event)
  end

  Angzarr::EventBook.new(pages: pages)
end
