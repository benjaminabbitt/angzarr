# frozen_string_literal: true

require 'spec_helper'

RSpec.describe LogProjectorLogic do
  describe '.process_event' do
    it 'returns nil for nil event' do
      result = described_class.process_event(nil)
      expect(result).to be_nil
    end

    it 'processes TransactionCreated event' do
      event = Examples::TransactionCreated.new(customer_id: 'cust-001', subtotal_cents: 2000)
      event_any = pack_event(event)

      result = described_class.process_event(event_any)

      expect(result.is_known).to be true
      expect(result.event_type).to eq('examples.TransactionCreated')
      expect(result.event_data[:customer_id]).to eq('cust-001')
      expect(result.event_data[:subtotal_cents]).to eq(2000)
    end

    it 'processes DiscountApplied event' do
      event = Examples::DiscountApplied.new(discount_type: 'percentage', discount_cents: 200)
      event_any = pack_event(event)

      result = described_class.process_event(event_any)

      expect(result.is_known).to be true
      expect(result.event_data[:discount_type]).to eq('percentage')
      expect(result.event_data[:discount_cents]).to eq(200)
    end

    it 'processes TransactionCompleted event' do
      event = Examples::TransactionCompleted.new(final_total_cents: 2000, payment_method: 'card')
      event_any = pack_event(event)

      result = described_class.process_event(event_any)

      expect(result.is_known).to be true
      expect(result.event_data[:final_total_cents]).to eq(2000)
      expect(result.event_data[:payment_method]).to eq('card')
    end

    it 'processes TransactionCancelled event' do
      event = Examples::TransactionCancelled.new(reason: 'customer request')
      event_any = pack_event(event)

      result = described_class.process_event(event_any)

      expect(result.is_known).to be true
      expect(result.event_data[:reason]).to eq('customer request')
    end

    it 'handles unknown event type' do
      event_any = Google::Protobuf::Any.new(
        type_url: 'type.examples/examples.UnknownEvent',
        value: ''
      )

      result = described_class.process_event(event_any)

      expect(result.is_known).to be false
      expect(result.event_type).to eq('examples.UnknownEvent')
    end
  end

  describe '.format_log' do
    it 'formats known event log entry' do
      log_entry = LogProjectorLogic::LogEntry.new(
        event_type: 'examples.TransactionCreated',
        event_data: { customer_id: 'cust-001' },
        is_known: true
      )

      result = described_class.format_log(
        log_entry,
        domain: 'transaction',
        aggregate_id: 'txn-001',
        sequence: 0
      )

      expect(result[:level]).to eq('info')
      expect(result[:message]).to eq('transaction event')
      expect(result[:event_type]).to eq('examples.TransactionCreated')
      expect(result[:domain]).to eq('transaction')
      expect(result[:aggregate_id]).to eq('txn-001')
      expect(result[:sequence]).to eq(0)
    end

    it 'formats unknown event log entry' do
      log_entry = LogProjectorLogic::LogEntry.new(
        event_type: 'examples.Unknown',
        event_data: { raw_type: 'unknown' },
        is_known: false
      )

      result = described_class.format_log(log_entry)

      expect(result[:message]).to eq('unknown event')
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
