# frozen_string_literal: true

require 'time'
require_relative 'command_validation_error'

# Pure business logic for transaction aggregate.
# No gRPC dependencies - can be tested in isolation.
module TransactionLogic
  POINTS_PER_CENT = 0.01

  TransactionState = Struct.new(:customer_id, :items, :subtotal_cents, :discount_cents, :status, keyword_init: true) do
    def initialize(customer_id: '', items: [], subtotal_cents: 0, discount_cents: 0, status: 'none')
      super
    end

    def exists?
      status != 'none'
    end

    def pending?
      status == 'pending'
    end

    def completed?
      status == 'completed'
    end

    def cancelled?
      status == 'cancelled'
    end

    def final_total_cents
      [subtotal_cents - discount_cents, 0].max
    end
  end

  class << self
    def rebuild_state(event_book)
      state = TransactionState.new

      return state if event_book.nil? || event_book.pages.empty?

      # Start from snapshot if present
      if event_book.snapshot&.state
        snap = event_book.snapshot.state
        if snap.type_url.end_with?('TransactionState')
          snap_state = Examples::TransactionState.decode(snap.value)
          state = TransactionState.new(
            customer_id: snap_state.customer_id,
            items: snap_state.items.to_a,
            subtotal_cents: snap_state.subtotal_cents,
            discount_cents: snap_state.discount_cents,
            status: snap_state.status
          )
        end
      end

      # Apply events
      event_book.pages.each do |page|
        next unless page.event

        state = apply_event(state, page.event)
      end

      state
    end

    def handle_create_transaction(state, customer_id, items)
      raise CommandValidationError.failed_precondition('Transaction already exists') if state.exists?
      raise CommandValidationError.invalid_argument('Customer ID is required') if customer_id.nil? || customer_id.empty?
      raise CommandValidationError.invalid_argument('Items are required') if items.nil? || items.empty?

      subtotal = calculate_subtotal(items)

      Examples::TransactionCreated.new(
        customer_id: customer_id,
        items: items,
        subtotal_cents: subtotal,
        created_at: now_timestamp
      )
    end

    def handle_apply_discount(state, discount_type, value)
      raise CommandValidationError.failed_precondition('Transaction does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Transaction is not pending') unless state.pending?
      raise CommandValidationError.invalid_argument('Discount type is required') if discount_type.nil? || discount_type.empty?
      raise CommandValidationError.invalid_argument('Discount value must be positive') if value <= 0

      discount_cents = calculate_discount(state.subtotal_cents, discount_type, value)

      Examples::DiscountApplied.new(
        discount_type: discount_type,
        value: value,
        discount_cents: discount_cents
      )
    end

    def handle_complete_transaction(state, payment_method)
      raise CommandValidationError.failed_precondition('Transaction does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Transaction is not pending') unless state.pending?

      final_total = state.final_total_cents
      points_earned = calculate_loyalty_points(final_total)

      Examples::TransactionCompleted.new(
        final_total_cents: final_total,
        payment_method: payment_method || '',
        loyalty_points_earned: points_earned,
        completed_at: now_timestamp
      )
    end

    def handle_cancel_transaction(state, reason)
      raise CommandValidationError.failed_precondition('Transaction does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Transaction is not pending') unless state.pending?

      Examples::TransactionCancelled.new(
        reason: reason || '',
        cancelled_at: now_timestamp
      )
    end

    private

    def apply_event(state, event_any)
      type_url = event_any.type_url

      if type_url.end_with?('TransactionCreated')
        event = Examples::TransactionCreated.decode(event_any.value)
        TransactionState.new(
          customer_id: event.customer_id,
          items: event.items.to_a,
          subtotal_cents: event.subtotal_cents,
          discount_cents: 0,
          status: 'pending'
        )
      elsif type_url.end_with?('DiscountApplied')
        event = Examples::DiscountApplied.decode(event_any.value)
        TransactionState.new(
          customer_id: state.customer_id,
          items: state.items,
          subtotal_cents: state.subtotal_cents,
          discount_cents: state.discount_cents + event.discount_cents,
          status: state.status
        )
      elsif type_url.end_with?('TransactionCompleted')
        TransactionState.new(
          customer_id: state.customer_id,
          items: state.items,
          subtotal_cents: state.subtotal_cents,
          discount_cents: state.discount_cents,
          status: 'completed'
        )
      elsif type_url.end_with?('TransactionCancelled')
        TransactionState.new(
          customer_id: state.customer_id,
          items: state.items,
          subtotal_cents: state.subtotal_cents,
          discount_cents: state.discount_cents,
          status: 'cancelled'
        )
      else
        state
      end
    end

    def calculate_subtotal(items)
      items.sum { |item| item.quantity * item.unit_price_cents }
    end

    def calculate_discount(subtotal_cents, discount_type, value)
      case discount_type
      when 'percentage'
        (subtotal_cents * value / 100.0).to_i
      when 'fixed'
        [value, subtotal_cents].min
      else
        0
      end
    end

    def calculate_loyalty_points(final_total_cents)
      (final_total_cents * POINTS_PER_CENT).to_i
    end

    def now_timestamp
      Google::Protobuf::Timestamp.new(seconds: Time.now.to_i)
    end
  end
end
