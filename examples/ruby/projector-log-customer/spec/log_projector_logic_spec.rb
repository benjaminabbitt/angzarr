# frozen_string_literal: true

require 'spec_helper'

RSpec.describe LogProjectorLogic do
  describe '.process_event' do
    it 'returns nil for nil event' do
      result = described_class.process_event(nil)
      expect(result).to be_nil
    end

    it 'processes CustomerCreated event' do
      event = Examples::CustomerCreated.new(name: 'Alice', email: 'alice@example.com')
      event_any = pack_event(event)

      result = described_class.process_event(event_any)

      expect(result.is_known).to be true
      expect(result.event_type).to eq('examples.CustomerCreated')
      expect(result.event_data[:name]).to eq('Alice')
      expect(result.event_data[:email]).to eq('alice@example.com')
    end

    it 'processes LoyaltyPointsAdded event' do
      event = Examples::LoyaltyPointsAdded.new(points: 100, new_balance: 100, reason: 'purchase')
      event_any = pack_event(event)

      result = described_class.process_event(event_any)

      expect(result.is_known).to be true
      expect(result.event_type).to eq('examples.LoyaltyPointsAdded')
      expect(result.event_data[:points]).to eq(100)
      expect(result.event_data[:new_balance]).to eq(100)
    end

    it 'processes LoyaltyPointsRedeemed event' do
      event = Examples::LoyaltyPointsRedeemed.new(points: 50, new_balance: 50, redemption_type: 'discount')
      event_any = pack_event(event)

      result = described_class.process_event(event_any)

      expect(result.is_known).to be true
      expect(result.event_data[:points]).to eq(50)
      expect(result.event_data[:redemption_type]).to eq('discount')
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
        event_type: 'examples.CustomerCreated',
        event_data: { name: 'Alice' },
        is_known: true
      )

      result = described_class.format_log(
        log_entry,
        domain: 'customer',
        aggregate_id: 'cust-001',
        sequence: 0
      )

      expect(result[:level]).to eq('info')
      expect(result[:message]).to eq('customer event')
      expect(result[:event_type]).to eq('examples.CustomerCreated')
      expect(result[:domain]).to eq('customer')
      expect(result[:aggregate_id]).to eq('cust-001')
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
