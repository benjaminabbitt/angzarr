# frozen_string_literal: true

require 'spec_helper'

RSpec.describe ReceiptProjectorLogic do
  describe '.project' do
    it 'returns nil for nil event_book' do
      result = described_class.project(nil)
      expect(result).to be_nil
    end

    it 'returns nil for empty event_book' do
      event_book = Angzarr::EventBook.new
      result = described_class.project(event_book)
      expect(result).to be_nil
    end

    it 'returns nil for incomplete transaction' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000)]
      created = Examples::TransactionCreated.new(customer_id: 'cust-001', items: items, subtotal_cents: 2000)

      event_book = Angzarr::EventBook.new(
        pages: [Angzarr::EventPage.new(num: 0, event: pack_event(created))]
      )

      result = described_class.project(event_book)

      expect(result).to be_nil
    end

    it 'generates receipt for completed transaction' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000)]
      created = Examples::TransactionCreated.new(customer_id: 'cust-001', items: items, subtotal_cents: 2000)
      completed = Examples::TransactionCompleted.new(final_total_cents: 2000, payment_method: 'card', loyalty_points_earned: 20)

      event_book = Angzarr::EventBook.new(
        pages: [
          Angzarr::EventPage.new(num: 0, event: pack_event(created)),
          Angzarr::EventPage.new(num: 1, event: pack_event(completed))
        ]
      )

      result = described_class.project(event_book)

      expect(result).not_to be_nil
      expect(result.customer_id).to eq('cust-001')
      expect(result.subtotal_cents).to eq(2000)
      expect(result.final_total_cents).to eq(2000)
      expect(result.payment_method).to eq('card')
      expect(result.loyalty_points_earned).to eq(20)
    end

    it 'includes discount in receipt' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000)]
      created = Examples::TransactionCreated.new(customer_id: 'cust-002', items: items, subtotal_cents: 2000)
      discount = Examples::DiscountApplied.new(discount_type: 'percentage', discount_cents: 200)
      completed = Examples::TransactionCompleted.new(final_total_cents: 1800, payment_method: 'cash', loyalty_points_earned: 18)

      event_book = Angzarr::EventBook.new(
        pages: [
          Angzarr::EventPage.new(num: 0, event: pack_event(created)),
          Angzarr::EventPage.new(num: 1, event: pack_event(discount)),
          Angzarr::EventPage.new(num: 2, event: pack_event(completed))
        ]
      )

      result = described_class.project(event_book)

      expect(result.subtotal_cents).to eq(2000)
      expect(result.discount_cents).to eq(200)
      expect(result.final_total_cents).to eq(1800)
    end

    it 'accumulates multiple discounts' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000)]
      created = Examples::TransactionCreated.new(customer_id: 'cust-003', items: items, subtotal_cents: 2000)
      discount1 = Examples::DiscountApplied.new(discount_type: 'percentage', discount_cents: 100)
      discount2 = Examples::DiscountApplied.new(discount_type: 'fixed', discount_cents: 50)
      completed = Examples::TransactionCompleted.new(final_total_cents: 1850, payment_method: 'card')

      event_book = Angzarr::EventBook.new(
        pages: [
          Angzarr::EventPage.new(num: 0, event: pack_event(created)),
          Angzarr::EventPage.new(num: 1, event: pack_event(discount1)),
          Angzarr::EventPage.new(num: 2, event: pack_event(discount2)),
          Angzarr::EventPage.new(num: 3, event: pack_event(completed))
        ]
      )

      result = described_class.project(event_book)

      expect(result.discount_cents).to eq(150)
    end
  end

  describe '.format_receipt' do
    it 'formats receipt with items' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 2, unit_price_cents: 1000)]
      receipt_data = ReceiptProjectorLogic::ReceiptData.new(
        customer_id: 'cust-001',
        items: items,
        subtotal_cents: 2000,
        discount_cents: 0,
        final_total_cents: 2000,
        payment_method: 'card',
        loyalty_points_earned: 20
      )

      result = described_class.format_receipt(receipt_data)

      expect(result).to include('RECEIPT')
      expect(result).to include('Widget')
      expect(result).to include('Thank you')
      expect(result).to include('$20.00')
    end

    it 'includes discount in formatted receipt' do
      items = [Examples::LineItem.new(product_id: 'SKU-001', name: 'Widget', quantity: 1, unit_price_cents: 2000)]
      receipt_data = ReceiptProjectorLogic::ReceiptData.new(
        customer_id: 'cust-002',
        items: items,
        subtotal_cents: 2000,
        discount_cents: 200,
        final_total_cents: 1800,
        payment_method: 'cash',
        loyalty_points_earned: 18
      )

      result = described_class.format_receipt(receipt_data)

      expect(result).to include('Discount')
      expect(result).to include('$2.00')
    end

    it 'includes transaction ID when provided' do
      items = []
      receipt_data = ReceiptProjectorLogic::ReceiptData.new(
        customer_id: 'cust-003',
        items: items,
        subtotal_cents: 1000,
        discount_cents: 0,
        final_total_cents: 1000,
        payment_method: 'card',
        loyalty_points_earned: 10
      )

      result = described_class.format_receipt(receipt_data, transaction_id: 'txn-12345')

      expect(result).to include('txn-12345')
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
