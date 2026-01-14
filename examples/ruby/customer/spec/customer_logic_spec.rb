# frozen_string_literal: true

require 'spec_helper'

RSpec.describe CustomerLogic do
  describe '.rebuild_state' do
    it 'returns empty state for nil event_book' do
      state = described_class.rebuild_state(nil)

      expect(state.name).to eq('')
      expect(state.email).to eq('')
      expect(state.loyalty_points).to eq(0)
      expect(state.lifetime_points).to eq(0)
      expect(state.exists?).to be false
    end

    it 'returns empty state for empty event_book' do
      event_book = Angzarr::EventBook.new

      state = described_class.rebuild_state(event_book)

      expect(state.exists?).to be false
    end

    it 'applies CustomerCreated event' do
      event = Examples::CustomerCreated.new(name: 'John Doe', email: 'john@example.com')
      event_any = Google::Protobuf::Any.new(
        type_url: 'type.examples/examples.CustomerCreated',
        value: event.to_proto
      )
      event_book = Angzarr::EventBook.new(
        pages: [Angzarr::EventPage.new(num: 0, event: event_any)]
      )

      state = described_class.rebuild_state(event_book)

      expect(state.exists?).to be true
      expect(state.name).to eq('John Doe')
      expect(state.email).to eq('john@example.com')
    end

    it 'applies LoyaltyPointsAdded event' do
      created = Examples::CustomerCreated.new(name: 'John', email: 'john@example.com')
      added = Examples::LoyaltyPointsAdded.new(points: 100, new_balance: 100)

      event_book = Angzarr::EventBook.new(
        pages: [
          Angzarr::EventPage.new(num: 0, event: pack_event(created)),
          Angzarr::EventPage.new(num: 1, event: pack_event(added))
        ]
      )

      state = described_class.rebuild_state(event_book)

      expect(state.loyalty_points).to eq(100)
      expect(state.lifetime_points).to eq(100)
    end

    it 'applies LoyaltyPointsRedeemed event' do
      created = Examples::CustomerCreated.new(name: 'John', email: 'john@example.com')
      added = Examples::LoyaltyPointsAdded.new(points: 100, new_balance: 100)
      redeemed = Examples::LoyaltyPointsRedeemed.new(points: 30, new_balance: 70)

      event_book = Angzarr::EventBook.new(
        pages: [
          Angzarr::EventPage.new(num: 0, event: pack_event(created)),
          Angzarr::EventPage.new(num: 1, event: pack_event(added)),
          Angzarr::EventPage.new(num: 2, event: pack_event(redeemed))
        ]
      )

      state = described_class.rebuild_state(event_book)

      expect(state.loyalty_points).to eq(70)
      expect(state.lifetime_points).to eq(100) # Lifetime not reduced
    end
  end

  describe '.handle_create_customer' do
    let(:empty_state) { CustomerLogic::CustomerState.new }
    let(:existing_state) { CustomerLogic::CustomerState.new(name: 'Existing', email: 'existing@test.com') }

    it 'returns CustomerCreated event for new customer' do
      event = described_class.handle_create_customer(empty_state, 'Alice', 'alice@example.com')

      expect(event).to be_a(Examples::CustomerCreated)
      expect(event.name).to eq('Alice')
      expect(event.email).to eq('alice@example.com')
    end

    it 'raises failed_precondition for existing customer' do
      expect {
        described_class.handle_create_customer(existing_state, 'New', 'new@test.com')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
        expect(error.message).to include('already exists')
      }
    end

    it 'raises invalid_argument for empty name' do
      expect {
        described_class.handle_create_customer(empty_state, '', 'email@test.com')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::INVALID_ARGUMENT)
      }
    end

    it 'raises invalid_argument for empty email' do
      expect {
        described_class.handle_create_customer(empty_state, 'Name', '')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::INVALID_ARGUMENT)
      }
    end
  end

  describe '.handle_add_loyalty_points' do
    let(:empty_state) { CustomerLogic::CustomerState.new }
    let(:existing_state) { CustomerLogic::CustomerState.new(name: 'John', email: 'john@test.com', loyalty_points: 50) }

    it 'returns LoyaltyPointsAdded event' do
      event = described_class.handle_add_loyalty_points(existing_state, 25, 'purchase')

      expect(event).to be_a(Examples::LoyaltyPointsAdded)
      expect(event.points).to eq(25)
      expect(event.new_balance).to eq(75)
    end

    it 'raises failed_precondition for non-existent customer' do
      expect {
        described_class.handle_add_loyalty_points(empty_state, 25, 'purchase')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
      }
    end

    it 'raises invalid_argument for zero points' do
      expect {
        described_class.handle_add_loyalty_points(existing_state, 0, 'purchase')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::INVALID_ARGUMENT)
      }
    end

    it 'raises invalid_argument for negative points' do
      expect {
        described_class.handle_add_loyalty_points(existing_state, -10, 'purchase')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::INVALID_ARGUMENT)
      }
    end
  end

  describe '.handle_redeem_loyalty_points' do
    let(:empty_state) { CustomerLogic::CustomerState.new }
    let(:existing_state) { CustomerLogic::CustomerState.new(name: 'John', email: 'john@test.com', loyalty_points: 100) }

    it 'returns LoyaltyPointsRedeemed event' do
      event = described_class.handle_redeem_loyalty_points(existing_state, 50, 'discount')

      expect(event).to be_a(Examples::LoyaltyPointsRedeemed)
      expect(event.points).to eq(50)
      expect(event.new_balance).to eq(50)
    end

    it 'raises failed_precondition for non-existent customer' do
      expect {
        described_class.handle_redeem_loyalty_points(empty_state, 50, 'discount')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
      }
    end

    it 'raises failed_precondition for insufficient points' do
      state = CustomerLogic::CustomerState.new(name: 'John', email: 'john@test.com', loyalty_points: 30)

      expect {
        described_class.handle_redeem_loyalty_points(state, 50, 'discount')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
        expect(error.message).to include('Insufficient')
      }
    end

    it 'raises invalid_argument for zero points' do
      expect {
        described_class.handle_redeem_loyalty_points(existing_state, 0, 'discount')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::INVALID_ARGUMENT)
      }
    end
  end

  private

  def pack_event(event)
    type_name = event.class.name.split('::').last
    Google::Protobuf::Any.new(
      type_url: "type.examples/examples.#{type_name}",
      value: event.to_proto
    )
  end
end
