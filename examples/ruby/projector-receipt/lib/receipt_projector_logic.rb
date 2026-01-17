# frozen_string_literal: true

require 'time'

# Pure business logic for receipt projector.
# No gRPC dependencies - can be tested in isolation.
module ReceiptProjectorLogic
  POINTS_PER_DOLLAR = 10

  ReceiptData = Struct.new(
    :customer_id,
    :items,
    :subtotal_cents,
    :discount_cents,
    :loyalty_points_used,
    :final_total_cents,
    :payment_method,
    :loyalty_points_earned,
    :completed_at,
    keyword_init: true
  )

  class << self
    def project(event_book)
      return nil if event_book.nil? || event_book.pages.empty?

      customer_id = ''
      items = []
      subtotal_cents = 0
      discount_cents = 0
      loyalty_points_used = 0
      final_total_cents = 0
      payment_method = ''
      completed_at = nil
      is_completed = false

      event_book.pages.each do |page|
        next unless page.event

        type_url = page.event.type_url

        if type_url.end_with?('OrderCreated')
          event = Examples::OrderCreated.decode(page.event.value)
          customer_id = event.customer_id
          items = event.items.to_a
          subtotal_cents = event.subtotal_cents
          discount_cents = event.discount_cents
        elsif type_url.end_with?('LoyaltyDiscountApplied')
          event = Examples::LoyaltyDiscountApplied.decode(page.event.value)
          loyalty_points_used = event.points_used
          discount_cents += event.discount_cents
        elsif type_url.end_with?('PaymentSubmitted')
          event = Examples::PaymentSubmitted.decode(page.event.value)
          final_total_cents = event.amount_cents
          payment_method = event.payment_method
        elsif type_url.end_with?('OrderCompleted')
          event = Examples::OrderCompleted.decode(page.event.value)
          completed_at = event.completed_at
          is_completed = true
        end
      end

      return nil unless is_completed

      loyalty_points_earned = (final_total_cents / 100) * POINTS_PER_DOLLAR

      ReceiptData.new(
        customer_id: customer_id,
        items: items,
        subtotal_cents: subtotal_cents,
        discount_cents: discount_cents,
        loyalty_points_used: loyalty_points_used,
        final_total_cents: final_total_cents,
        payment_method: payment_method,
        loyalty_points_earned: loyalty_points_earned,
        completed_at: completed_at
      )
    end

    def format_receipt(receipt_data, order_id: nil)
      timestamp = receipt_data.completed_at ? Time.at(receipt_data.completed_at.seconds).strftime('%Y-%m-%d %H:%M:%S') : Time.now.strftime('%Y-%m-%d %H:%M:%S')

      short_order_id = order_id && order_id.length > 16 ? order_id[0...16] : order_id
      short_customer_id = receipt_data.customer_id.length > 16 ? receipt_data.customer_id[0...16] : receipt_data.customer_id

      lines = []
      lines << '=' * 40
      lines << 'RECEIPT'.center(40)
      lines << '=' * 40
      lines << ''
      lines << "Order: #{short_order_id}..." if short_order_id
      lines << "Customer: #{short_customer_id.empty? ? 'N/A' : "#{short_customer_id}..."}"
      lines << "Date: #{timestamp}"
      lines << ''
      lines << '-' * 40

      receipt_data.items.each do |item|
        line_total = item.quantity * item.unit_price_cents
        lines << "#{item.quantity} x #{item.name} @ $#{format_cents(item.unit_price_cents)} = $#{format_cents(line_total)}"
      end

      lines << '-' * 40
      lines << "Subtotal: $#{format_cents(receipt_data.subtotal_cents)}".rjust(40)

      if receipt_data.discount_cents > 0
        discount_type = receipt_data.loyalty_points_used > 0 ? 'loyalty' : 'coupon'
        lines << "Discount (#{discount_type}): -$#{format_cents(receipt_data.discount_cents)}".rjust(40)
      end

      lines << '-' * 40
      lines << "TOTAL: $#{format_cents(receipt_data.final_total_cents)}".rjust(40)
      lines << "Payment: #{receipt_data.payment_method}"
      lines << '-' * 40
      lines << "Loyalty Points Earned: #{receipt_data.loyalty_points_earned}"
      lines << '=' * 40
      lines << 'Thank you for your purchase!'.center(40)
      lines << '=' * 40

      lines.join("\n")
    end

    private

    def format_cents(cents)
      format('%.2f', cents / 100.0)
    end
  end
end
