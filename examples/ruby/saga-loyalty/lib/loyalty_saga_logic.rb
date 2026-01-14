# frozen_string_literal: true

require 'time'

# Pure business logic for loyalty saga.
# No gRPC dependencies - can be tested in isolation.
module LoyaltySagaLogic
  CommandResult = Struct.new(:domain, :aggregate_id, :command_type, :points, :reason, keyword_init: true)

  class << self
    def process(event_book, transaction_id: nil)
      return [] if event_book.nil? || event_book.pages.empty?

      commands = []

      event_book.pages.each do |page|
        next unless page.event

        type_url = page.event.type_url

        if type_url.end_with?('TransactionCompleted')
          event = Examples::TransactionCompleted.decode(page.event.value)
          cmd = create_add_points_command(event, transaction_id)
          commands << cmd if cmd
        end
      end

      commands
    end

    private

    def create_add_points_command(event, transaction_id)
      points = event.loyalty_points_earned
      return nil if points <= 0

      # The customer_id needs to come from somewhere - typically from the event book cover
      # or looked up from previous events. For the saga, we use the transaction ID
      # as the reference and the customer domain will handle the actual customer lookup.
      reason = transaction_id ? "transaction:#{transaction_id}" : 'transaction reward'

      CommandResult.new(
        domain: 'customer',
        aggregate_id: nil, # Would be set by caller from event book cover
        command_type: 'AddLoyaltyPoints',
        points: points,
        reason: reason
      )
    end
  end
end
