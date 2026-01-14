# frozen_string_literal: true

require 'json'
require 'time'

# Pure business logic for transaction log projector.
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
        message: log_entry.is_known ? 'transaction event' : 'unknown event',
        projector: 'projector-log-transaction',
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
      type_url.end_with?('TransactionCreated') ||
        type_url.end_with?('DiscountApplied') ||
        type_url.end_with?('TransactionCompleted') ||
        type_url.end_with?('TransactionCancelled')
    end

    def decode_event(type_url, value)
      if type_url.end_with?('TransactionCreated')
        event = Examples::TransactionCreated.decode(value)
        { customer_id: event.customer_id, subtotal_cents: event.subtotal_cents }
      elsif type_url.end_with?('DiscountApplied')
        event = Examples::DiscountApplied.decode(value)
        { discount_type: event.discount_type, discount_cents: event.discount_cents }
      elsif type_url.end_with?('TransactionCompleted')
        event = Examples::TransactionCompleted.decode(value)
        { final_total_cents: event.final_total_cents, payment_method: event.payment_method }
      elsif type_url.end_with?('TransactionCancelled')
        event = Examples::TransactionCancelled.decode(value)
        { reason: event.reason }
      else
        { raw_type: type_url }
      end
    end
  end
end
