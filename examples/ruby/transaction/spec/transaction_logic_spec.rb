# frozen_string_literal: true

require 'spec_helper'

RSpec.describe TransactionLogic do
  describe '.rebuild_state' do
    it 'returns empty state for nil event_book' do
      state = described_class.rebuild_state(nil)

      expect(state.customer_id).to eq('')
      expect(state.items).to eq([])
      expect(state.subtotal_cents).to eq(0)
      expect(state.discount_cents).to eq(0)
      expect(state.status).to eq('none')
      expect(state.exists?).to be false
    end

    it 'returns empty state for empty event_book' do
      event_book = Angzarr::EventBook.new

      state = described_class.rebuild_state(event_book)

      expect(state.exists?).to be false
    end

    it 'applies TransactionCreated event' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000)]
      event = Examples::TransactionCreated.new(customer_id: 'cust-001', items: items, subtotal_cents: 2000)
      event_book = Angzarr::EventBook.new(
        pages: [Angzarr::EventPage.new(num: 0, event: pack_event(event))]
      )

      state = described_class.rebuild_state(event_book)

      expect(state.exists?).to be true
      expect(state.pending?).to be true
      expect(state.customer_id).to eq('cust-001')
      expect(state.subtotal_cents).to eq(2000)
    end

    it 'applies DiscountApplied event' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000)]
      created = Examples::TransactionCreated.new(customer_id: 'cust-001', items: items, subtotal_cents: 2000)
      discount = Examples::DiscountApplied.new(discount_type: 'percentage', value: 10, discount_cents: 200)

      event_book = Angzarr::EventBook.new(
        pages: [
          Angzarr::EventPage.new(num: 0, event: pack_event(created)),
          Angzarr::EventPage.new(num: 1, event: pack_event(discount))
        ]
      )

      state = described_class.rebuild_state(event_book)

      expect(state.discount_cents).to eq(200)
      expect(state.final_total_cents).to eq(1800)
    end

    it 'applies TransactionCompleted event' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000)]
      created = Examples::TransactionCreated.new(customer_id: 'cust-001', items: items, subtotal_cents: 2000)
      completed = Examples::TransactionCompleted.new(final_total_cents: 2000, payment_method: 'card')

      event_book = Angzarr::EventBook.new(
        pages: [
          Angzarr::EventPage.new(num: 0, event: pack_event(created)),
          Angzarr::EventPage.new(num: 1, event: pack_event(completed))
        ]
      )

      state = described_class.rebuild_state(event_book)

      expect(state.completed?).to be true
      expect(state.pending?).to be false
    end

    it 'applies TransactionCancelled event' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 1, unit_price_cents: 1000)]
      created = Examples::TransactionCreated.new(customer_id: 'cust-001', items: items, subtotal_cents: 1000)
      cancelled = Examples::TransactionCancelled.new(reason: 'customer request')

      event_book = Angzarr::EventBook.new(
        pages: [
          Angzarr::EventPage.new(num: 0, event: pack_event(created)),
          Angzarr::EventPage.new(num: 1, event: pack_event(cancelled))
        ]
      )

      state = described_class.rebuild_state(event_book)

      expect(state.cancelled?).to be true
      expect(state.pending?).to be false
    end
  end

  describe '.handle_create_transaction' do
    let(:empty_state) { TransactionLogic::TransactionState.new }
    let(:existing_state) { TransactionLogic::TransactionState.new(customer_id: 'cust-001', status: 'pending') }

    it 'returns TransactionCreated event for new transaction' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000)]
      event = described_class.handle_create_transaction(empty_state, 'cust-001', items)

      expect(event).to be_a(Examples::TransactionCreated)
      expect(event.customer_id).to eq('cust-001')
      expect(event.subtotal_cents).to eq(2000)
    end

    it 'calculates subtotal correctly for multiple items' do
      items = [
        Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000),
        Examples::LineItem.new(product_id: 'SKU-002', name: 'Gadget', quantity: 1, unit_price_cents: 2500)
      ]
      event = described_class.handle_create_transaction(empty_state, 'cust-001', items)

      expect(event.subtotal_cents).to eq(4500)
    end

    it 'raises failed_precondition for existing transaction' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 1, unit_price_cents: 1000)]

      expect {
        described_class.handle_create_transaction(existing_state, 'cust-002', items)
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
      }
    end

    it 'raises invalid_argument for empty customer_id' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 1, unit_price_cents: 1000)]

      expect {
        described_class.handle_create_transaction(empty_state, '', items)
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::INVALID_ARGUMENT)
      }
    end

    it 'raises invalid_argument for empty items' do
      expect {
        described_class.handle_create_transaction(empty_state, 'cust-001', [])
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::INVALID_ARGUMENT)
      }
    end
  end

  describe '.handle_apply_discount' do
    let(:empty_state) { TransactionLogic::TransactionState.new }
    let(:pending_state) { TransactionLogic::TransactionState.new(customer_id: 'cust-001', subtotal_cents: 2000, status: 'pending') }
    let(:completed_state) { TransactionLogic::TransactionState.new(customer_id: 'cust-001', subtotal_cents: 2000, status: 'completed') }

    it 'returns DiscountApplied event for percentage discount' do
      event = described_class.handle_apply_discount(pending_state, 'percentage', 10)

      expect(event).to be_a(Examples::DiscountApplied)
      expect(event.discount_type).to eq('percentage')
      expect(event.value).to eq(10)
      expect(event.discount_cents).to eq(200)
    end

    it 'returns DiscountApplied event for fixed discount' do
      event = described_class.handle_apply_discount(pending_state, 'fixed', 500)

      expect(event).to be_a(Examples::DiscountApplied)
      expect(event.discount_cents).to eq(500)
    end

    it 'caps fixed discount at subtotal' do
      event = described_class.handle_apply_discount(pending_state, 'fixed', 5000)

      expect(event.discount_cents).to eq(2000)
    end

    it 'raises failed_precondition for non-existent transaction' do
      expect {
        described_class.handle_apply_discount(empty_state, 'percentage', 10)
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
      }
    end

    it 'raises failed_precondition for completed transaction' do
      expect {
        described_class.handle_apply_discount(completed_state, 'percentage', 10)
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
      }
    end

    it 'raises invalid_argument for empty discount type' do
      expect {
        described_class.handle_apply_discount(pending_state, '', 10)
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::INVALID_ARGUMENT)
      }
    end

    it 'raises invalid_argument for zero value' do
      expect {
        described_class.handle_apply_discount(pending_state, 'percentage', 0)
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::INVALID_ARGUMENT)
      }
    end
  end

  describe '.handle_complete_transaction' do
    let(:empty_state) { TransactionLogic::TransactionState.new }
    let(:pending_state) { TransactionLogic::TransactionState.new(customer_id: 'cust-001', subtotal_cents: 2000, status: 'pending') }
    let(:pending_state_with_discount) { TransactionLogic::TransactionState.new(customer_id: 'cust-001', subtotal_cents: 2000, discount_cents: 200, status: 'pending') }
    let(:completed_state) { TransactionLogic::TransactionState.new(customer_id: 'cust-001', subtotal_cents: 2000, status: 'completed') }

    it 'returns TransactionCompleted event' do
      event = described_class.handle_complete_transaction(pending_state, 'card')

      expect(event).to be_a(Examples::TransactionCompleted)
      expect(event.final_total_cents).to eq(2000)
      expect(event.payment_method).to eq('card')
      expect(event.loyalty_points_earned).to eq(20)
    end

    it 'calculates final total with discount' do
      event = described_class.handle_complete_transaction(pending_state_with_discount, 'cash')

      expect(event.final_total_cents).to eq(1800)
      expect(event.loyalty_points_earned).to eq(18)
    end

    it 'raises failed_precondition for non-existent transaction' do
      expect {
        described_class.handle_complete_transaction(empty_state, 'card')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
      }
    end

    it 'raises failed_precondition for already completed transaction' do
      expect {
        described_class.handle_complete_transaction(completed_state, 'card')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
      }
    end
  end

  describe '.handle_cancel_transaction' do
    let(:empty_state) { TransactionLogic::TransactionState.new }
    let(:pending_state) { TransactionLogic::TransactionState.new(customer_id: 'cust-001', subtotal_cents: 2000, status: 'pending') }
    let(:completed_state) { TransactionLogic::TransactionState.new(customer_id: 'cust-001', subtotal_cents: 2000, status: 'completed') }

    it 'returns TransactionCancelled event' do
      event = described_class.handle_cancel_transaction(pending_state, 'customer request')

      expect(event).to be_a(Examples::TransactionCancelled)
      expect(event.reason).to eq('customer request')
    end

    it 'raises failed_precondition for non-existent transaction' do
      expect {
        described_class.handle_cancel_transaction(empty_state, 'reason')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
      }
    end

    it 'raises failed_precondition for completed transaction' do
      expect {
        described_class.handle_cancel_transaction(completed_state, 'too late')
      }.to raise_error(CommandValidationError) { |error|
        expect(error.status_code).to eq(CommandValidationError::FAILED_PRECONDITION)
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
