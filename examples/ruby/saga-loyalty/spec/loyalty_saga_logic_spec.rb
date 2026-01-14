# frozen_string_literal: true

require 'spec_helper'

RSpec.describe LoyaltySagaLogic do
  describe '.process' do
    it 'returns empty array for nil event_book' do
      result = described_class.process(nil)
      expect(result).to eq([])
    end

    it 'returns empty array for empty event_book' do
      event_book = Angzarr::EventBook.new
      result = described_class.process(event_book)
      expect(result).to eq([])
    end

    it 'returns empty array for non-completed transaction' do
      created = Examples::TransactionCreated.new(customer_id: 'cust-001', subtotal_cents: 2000)

      event_book = Angzarr::EventBook.new(
        pages: [Angzarr::EventPage.new(num: 0, event: pack_event(created))]
      )

      result = described_class.process(event_book)

      expect(result).to eq([])
    end

    it 'generates AddLoyaltyPoints command for completed transaction' do
      completed = Examples::TransactionCompleted.new(
        final_total_cents: 2000,
        payment_method: 'card',
        loyalty_points_earned: 20
      )

      event_book = Angzarr::EventBook.new(
        pages: [Angzarr::EventPage.new(num: 0, event: pack_event(completed))]
      )

      result = described_class.process(event_book, transaction_id: 'txn-001')

      expect(result.size).to eq(1)
      expect(result[0].domain).to eq('customer')
      expect(result[0].command_type).to eq('AddLoyaltyPoints')
      expect(result[0].points).to eq(20)
      expect(result[0].reason).to include('transaction')
    end

    it 'returns empty array for zero points' do
      completed = Examples::TransactionCompleted.new(
        final_total_cents: 0,
        payment_method: 'card',
        loyalty_points_earned: 0
      )

      event_book = Angzarr::EventBook.new(
        pages: [Angzarr::EventPage.new(num: 0, event: pack_event(completed))]
      )

      result = described_class.process(event_book)

      expect(result).to eq([])
    end

    it 'includes transaction ID in reason' do
      completed = Examples::TransactionCompleted.new(
        final_total_cents: 5000,
        payment_method: 'card',
        loyalty_points_earned: 50
      )

      event_book = Angzarr::EventBook.new(
        pages: [Angzarr::EventPage.new(num: 0, event: pack_event(completed))]
      )

      result = described_class.process(event_book, transaction_id: 'txn-12345')

      expect(result[0].reason).to include('txn-12345')
    end

    it 'handles multiple TransactionCompleted events' do
      completed1 = Examples::TransactionCompleted.new(loyalty_points_earned: 10)
      completed2 = Examples::TransactionCompleted.new(loyalty_points_earned: 20)

      event_book = Angzarr::EventBook.new(
        pages: [
          Angzarr::EventPage.new(num: 0, event: pack_event(completed1)),
          Angzarr::EventPage.new(num: 1, event: pack_event(completed2))
        ]
      )

      result = described_class.process(event_book)

      expect(result.size).to eq(2)
      expect(result[0].points).to eq(10)
      expect(result[1].points).to eq(20)
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
