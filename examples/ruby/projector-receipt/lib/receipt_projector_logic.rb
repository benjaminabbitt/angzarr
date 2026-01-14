# frozen_string_literal: true

require 'time'

# Pure business logic for receipt projector.
# No gRPC dependencies - can be tested in isolation.
module ReceiptProjectorLogic
  ReceiptData = Struct.new(
    :customer_id,
    :items,
    :subtotal_cents,
    :discount_cents,
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
      final_total_cents = 0
      payment_method = ''
      loyalty_points_earned = 0
      completed_at = nil
      is_completed = false

      event_book.pages.each do |page|
        next unless page.event

        type_url = page.event.type_url

        if type_url.end_with?('TransactionCreated')
          event = Examples::TransactionCreated.decode(page.event.value)
          customer_id = event.customer_id
          items = event.items.to_a
          subtotal_cents = event.subtotal_cents
        elsif type_url.end_with?('DiscountApplied')
          event = Examples::DiscountApplied.decode(page.event.value)
          discount_cents += event.discount_cents
        elsif type_url.end_with?('TransactionCompleted')
          event = Examples::TransactionCompleted.decode(page.event.value)
          final_total_cents = event.final_total_cents
          payment_method = event.payment_method
          loyalty_points_earned = event.loyalty_points_earned
          completed_at = event.completed_at
          is_completed = true
        end
      end

      return nil unless is_completed

      ReceiptData.new(
        customer_id: customer_id,
        items: items,
        subtotal_cents: subtotal_cents,
        discount_cents: discount_cents,
        final_total_cents: final_total_cents,
        payment_method: payment_method,
        loyalty_points_earned: loyalty_points_earned,
        completed_at: completed_at
      )
    end

    def format_receipt(receipt_data, transaction_id: nil)
      timestamp = receipt_data.completed_at ? Time.at(receipt_data.completed_at.seconds).strftime('%Y-%m-%d %H:%M:%S') : Time.now.strftime('%Y-%m-%d %H:%M:%S')

      lines = []
      lines << '=' * 40
      lines << 'RECEIPT'.center(40)
      lines << '=' * 40
      lines << ''
      lines << "Transaction: #{transaction_id}" if transaction_id
      lines << "Customer: #{receipt_data.customer_id}"
      lines << "Date: #{timestamp}"
      lines << ''
      lines << '-' * 40

      receipt_data.items.each do |item|
        line_total = item.quantity * item.unit_price_cents
        lines << "#{item.name}"
        lines << "  #{item.quantity} x $#{format_cents(item.unit_price_cents)} = $#{format_cents(line_total)}"
      end

      lines << '-' * 40
      lines << "Subtotal: $#{format_cents(receipt_data.subtotal_cents)}".rjust(40)

      if receipt_data.discount_cents > 0
        lines << "Discount: -$#{format_cents(receipt_data.discount_cents)}".rjust(40)
      end

      lines << "TOTAL: $#{format_cents(receipt_data.final_total_cents)}".rjust(40)
      lines << ''
      lines << "Payment: #{receipt_data.payment_method}"

      if receipt_data.loyalty_points_earned > 0
        lines << "Points Earned: #{receipt_data.loyalty_points_earned}"
      end

      lines << ''
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
