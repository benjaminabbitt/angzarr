# frozen_string_literal: true

require 'time'
require_relative 'command_validation_error'

# Pure business logic for order aggregate.
# No gRPC dependencies - can be tested in isolation.
module OrderLogic
  OrderState = Struct.new(:customer_id, :items, :subtotal_cents, :discount_cents,
                          :loyalty_points_used, :final_total_cents, :payment_method,
                          :status, keyword_init: true) do
    def initialize(customer_id: '', items: [], subtotal_cents: 0, discount_cents: 0,
                   loyalty_points_used: 0, final_total_cents: 0, payment_method: '', status: :uninitialized)
      super
    end

    def exists?
      status != :uninitialized
    end

    def pending_payment?
      status == :pending_payment
    end

    def payment_submitted?
      status == :payment_submitted
    end
  end

  class << self
    def rebuild_state(event_book)
      state = OrderState.new

      return state if event_book.nil? || event_book.pages.empty?

      # Apply events
      event_book.pages.each do |page|
        next unless page.event

        state = apply_event(state, page.event)
      end

      state
    end

    def handle_create_order(state, customer_id, items, subtotal_cents, discount_cents)
      raise CommandValidationError.failed_precondition('Order already exists') if state.exists?
      raise CommandValidationError.invalid_argument('Customer ID is required') if customer_id.nil? || customer_id.empty?
      raise CommandValidationError.invalid_argument('Order must have items') if items.nil? || items.empty?

      Examples::OrderCreated.new(
        customer_id: customer_id,
        items: items,
        subtotal_cents: subtotal_cents,
        discount_cents: discount_cents,
        created_at: now_timestamp
      )
    end

    def handle_apply_loyalty_discount(state, points_used, discount_cents)
      raise CommandValidationError.failed_precondition('Order does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Order is not pending payment') unless state.pending_payment?
      raise CommandValidationError.invalid_argument('Points must be positive') if points_used <= 0
      raise CommandValidationError.invalid_argument('Discount must be positive') if discount_cents <= 0

      Examples::LoyaltyDiscountApplied.new(
        points_used: points_used,
        discount_cents: discount_cents
      )
    end

    def handle_submit_payment(state, payment_method, amount_cents)
      raise CommandValidationError.failed_precondition('Order does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Order is not pending payment') unless state.pending_payment?
      raise CommandValidationError.invalid_argument('Payment method is required') if payment_method.nil? || payment_method.empty?
      raise CommandValidationError.invalid_argument('Amount must be positive') if amount_cents <= 0

      expected_total = state.subtotal_cents - state.discount_cents
      if amount_cents != expected_total
        raise CommandValidationError.invalid_argument(
          "Payment amount #{amount_cents} does not match expected #{expected_total}"
        )
      end

      Examples::PaymentSubmitted.new(
        payment_method: payment_method,
        amount_cents: amount_cents,
        submitted_at: now_timestamp
      )
    end

    def handle_confirm_payment(state)
      raise CommandValidationError.failed_precondition('Order does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Payment not submitted') unless state.payment_submitted?

      Examples::PaymentConfirmed.new(confirmed_at: now_timestamp)
    end

    def handle_complete_order(state)
      raise CommandValidationError.failed_precondition('Order does not exist') unless state.exists?

      Examples::OrderCompleted.new(completed_at: now_timestamp)
    end

    def handle_cancel_order(state, reason, loyalty_points_used)
      raise CommandValidationError.failed_precondition('Order does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Cannot cancel completed order') if state.status == :completed

      Examples::OrderCancelled.new(
        reason: reason || '',
        loyalty_points_used: loyalty_points_used || state.loyalty_points_used,
        cancelled_at: now_timestamp
      )
    end

    private

    def apply_event(state, event_any)
      type_url = event_any.type_url

      if type_url.end_with?('OrderCreated')
        event = Examples::OrderCreated.decode(event_any.value)
        OrderState.new(
          customer_id: event.customer_id,
          items: event.items.to_a,
          subtotal_cents: event.subtotal_cents,
          discount_cents: event.discount_cents,
          loyalty_points_used: 0,
          final_total_cents: 0,
          payment_method: '',
          status: :pending_payment
        )
      elsif type_url.end_with?('LoyaltyDiscountApplied')
        event = Examples::LoyaltyDiscountApplied.decode(event_any.value)
        OrderState.new(
          customer_id: state.customer_id,
          items: state.items,
          subtotal_cents: state.subtotal_cents,
          discount_cents: state.discount_cents + event.discount_cents,
          loyalty_points_used: event.points_used,
          final_total_cents: state.final_total_cents,
          payment_method: state.payment_method,
          status: state.status
        )
      elsif type_url.end_with?('PaymentSubmitted')
        event = Examples::PaymentSubmitted.decode(event_any.value)
        OrderState.new(
          customer_id: state.customer_id,
          items: state.items,
          subtotal_cents: state.subtotal_cents,
          discount_cents: state.discount_cents,
          loyalty_points_used: state.loyalty_points_used,
          final_total_cents: event.amount_cents,
          payment_method: event.payment_method,
          status: :payment_submitted
        )
      elsif type_url.end_with?('PaymentConfirmed')
        OrderState.new(
          customer_id: state.customer_id,
          items: state.items,
          subtotal_cents: state.subtotal_cents,
          discount_cents: state.discount_cents,
          loyalty_points_used: state.loyalty_points_used,
          final_total_cents: state.final_total_cents,
          payment_method: state.payment_method,
          status: :payment_confirmed
        )
      elsif type_url.end_with?('OrderCompleted')
        OrderState.new(
          customer_id: state.customer_id,
          items: state.items,
          subtotal_cents: state.subtotal_cents,
          discount_cents: state.discount_cents,
          loyalty_points_used: state.loyalty_points_used,
          final_total_cents: state.final_total_cents,
          payment_method: state.payment_method,
          status: :completed
        )
      elsif type_url.end_with?('OrderCancelled')
        OrderState.new(
          customer_id: state.customer_id,
          items: state.items,
          subtotal_cents: state.subtotal_cents,
          discount_cents: state.discount_cents,
          loyalty_points_used: state.loyalty_points_used,
          final_total_cents: state.final_total_cents,
          payment_method: state.payment_method,
          status: :cancelled
        )
      else
        state
      end
    end

    def now_timestamp
      Google::Protobuf::Timestamp.new(seconds: Time.now.to_i)
    end
  end
end
