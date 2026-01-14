# frozen_string_literal: true

require 'json'
require 'time'

# Pure business logic for customer log projector.
# No gRPC dependencies - can be tested in isolation.
module LogProjectorLogic
  LogEntry = Struct.new(:event_type, :event_data, :is_known, keyword_init: true)

  class << self
    def process_event(event_any)
      return nil unless event_any

      type_url = event_any.type_url
      event_type = type_url.split('/').last

      if known_event?(type_url)
        event_data = decode_event(type_url, event_any.value)
        LogEntry.new(event_type: event_type, event_data: event_data, is_known: true)
      else
        LogEntry.new(event_type: event_type, event_data: { raw_type: type_url }, is_known: false)
      end
    end

    def format_log(log_entry, domain: nil, aggregate_id: nil, sequence: nil, created_at: nil)
      {
        level: 'info',
        message: log_entry.is_known ? 'customer event' : 'unknown event',
        projector: 'projector-log-customer',
        event_type: log_entry.event_type,
        sequence: sequence,
        domain: domain,
        aggregate_id: aggregate_id,
        created_at: created_at,
        event_data: log_entry.event_data,
        timestamp: Time.now.iso8601
      }
    end

    private

    def known_event?(type_url)
      type_url.end_with?('CustomerCreated') ||
        type_url.end_with?('LoyaltyPointsAdded') ||
        type_url.end_with?('LoyaltyPointsRedeemed')
    end

    def decode_event(type_url, value)
      if type_url.end_with?('CustomerCreated')
        event = Examples::CustomerCreated.decode(value)
        { name: event.name, email: event.email }
      elsif type_url.end_with?('LoyaltyPointsAdded')
        event = Examples::LoyaltyPointsAdded.decode(value)
        { points: event.points, new_balance: event.new_balance, reason: event.reason }
      elsif type_url.end_with?('LoyaltyPointsRedeemed')
        event = Examples::LoyaltyPointsRedeemed.decode(value)
        { points: event.points, new_balance: event.new_balance, redemption_type: event.redemption_type }
      else
        { raw_type: type_url }
      end
    end
  end
end
