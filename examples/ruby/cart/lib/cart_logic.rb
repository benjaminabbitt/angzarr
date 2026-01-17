# frozen_string_literal: true

require 'time'
require_relative 'command_validation_error'

# Pure business logic for cart aggregate.
# No gRPC dependencies - can be tested in isolation.
module CartLogic
  CartItem = Struct.new(:sku, :name, :quantity, :unit_price_cents, keyword_init: true)

  CartState = Struct.new(:customer_id, :items, :coupon_code, :discount_cents, :status, keyword_init: true) do
    def initialize(customer_id: '', items: [], coupon_code: '', discount_cents: 0, status: :uninitialized)
      super
    end

    def exists?
      status != :uninitialized
    end

    def active?
      status == :active
    end

    def subtotal_cents
      items.sum { |item| item.quantity * item.unit_price_cents }
    end
  end

  class << self
    def rebuild_state(event_book)
      state = CartState.new

      return state if event_book.nil? || event_book.pages.empty?

      # Apply events
      event_book.pages.each do |page|
        next unless page.event

        state = apply_event(state, page.event)
      end

      state
    end

    def handle_create_cart(state, customer_id)
      raise CommandValidationError.failed_precondition('Cart already exists') if state.exists?
      raise CommandValidationError.invalid_argument('Customer ID is required') if customer_id.nil? || customer_id.empty?

      Examples::CartCreated.new(
        customer_id: customer_id,
        created_at: now_timestamp
      )
    end

    def handle_add_item(state, sku, name, quantity, unit_price_cents)
      raise CommandValidationError.failed_precondition('Cart does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Cart is not active') unless state.active?
      raise CommandValidationError.invalid_argument('SKU is required') if sku.nil? || sku.empty?
      raise CommandValidationError.invalid_argument('Quantity must be positive') if quantity <= 0

      Examples::ItemAdded.new(
        sku: sku,
        name: name || '',
        quantity: quantity,
        unit_price_cents: unit_price_cents
      )
    end

    def handle_update_quantity(state, sku, quantity)
      raise CommandValidationError.failed_precondition('Cart does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Cart is not active') unless state.active?
      raise CommandValidationError.invalid_argument('SKU is required') if sku.nil? || sku.empty?
      raise CommandValidationError.invalid_argument('Quantity must be positive') if quantity <= 0

      item = state.items.find { |i| i.sku == sku }
      raise CommandValidationError.failed_precondition("Item #{sku} not in cart") unless item

      Examples::QuantityUpdated.new(
        sku: sku,
        old_quantity: item.quantity,
        new_quantity: quantity
      )
    end

    def handle_remove_item(state, sku)
      raise CommandValidationError.failed_precondition('Cart does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Cart is not active') unless state.active?
      raise CommandValidationError.invalid_argument('SKU is required') if sku.nil? || sku.empty?

      item = state.items.find { |i| i.sku == sku }
      raise CommandValidationError.failed_precondition("Item #{sku} not in cart") unless item

      Examples::ItemRemoved.new(sku: sku)
    end

    def handle_apply_coupon(state, coupon_code, discount_cents)
      raise CommandValidationError.failed_precondition('Cart does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Cart is not active') unless state.active?
      raise CommandValidationError.invalid_argument('Coupon code is required') if coupon_code.nil? || coupon_code.empty?
      raise CommandValidationError.failed_precondition('Coupon already applied') unless state.coupon_code.empty?

      Examples::CouponApplied.new(
        coupon_code: coupon_code,
        discount_cents: discount_cents
      )
    end

    def handle_clear_cart(state)
      raise CommandValidationError.failed_precondition('Cart does not exist') unless state.exists?

      Examples::CartCleared.new(cleared_at: now_timestamp)
    end

    def handle_checkout(state, loyalty_points_to_use)
      raise CommandValidationError.failed_precondition('Cart does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Cart is not active') unless state.active?
      raise CommandValidationError.failed_precondition('Cart is empty') if state.items.empty?

      line_items = state.items.map do |item|
        Examples::LineItem.new(
          sku: item.sku,
          name: item.name,
          quantity: item.quantity,
          unit_price_cents: item.unit_price_cents
        )
      end

      Examples::CartCheckoutRequested.new(
        customer_id: state.customer_id,
        items: line_items,
        subtotal_cents: state.subtotal_cents,
        discount_cents: state.discount_cents,
        loyalty_points_to_use: loyalty_points_to_use || 0
      )
    end

    private

    def apply_event(state, event_any)
      type_url = event_any.type_url

      if type_url.end_with?('CartCreated')
        event = Examples::CartCreated.decode(event_any.value)
        CartState.new(
          customer_id: event.customer_id,
          items: [],
          coupon_code: '',
          discount_cents: 0,
          status: :active
        )
      elsif type_url.end_with?('ItemAdded')
        event = Examples::ItemAdded.decode(event_any.value)
        existing_item = state.items.find { |i| i.sku == event.sku }
        new_items = if existing_item
                      state.items.map do |i|
                        if i.sku == event.sku
                          CartItem.new(
                            sku: i.sku,
                            name: i.name,
                            quantity: i.quantity + event.quantity,
                            unit_price_cents: i.unit_price_cents
                          )
                        else
                          i
                        end
                      end
                    else
                      state.items + [CartItem.new(
                        sku: event.sku,
                        name: event.name,
                        quantity: event.quantity,
                        unit_price_cents: event.unit_price_cents
                      )]
                    end
        CartState.new(
          customer_id: state.customer_id,
          items: new_items,
          coupon_code: state.coupon_code,
          discount_cents: state.discount_cents,
          status: state.status
        )
      elsif type_url.end_with?('QuantityUpdated')
        event = Examples::QuantityUpdated.decode(event_any.value)
        new_items = state.items.map do |i|
          if i.sku == event.sku
            CartItem.new(
              sku: i.sku,
              name: i.name,
              quantity: event.new_quantity,
              unit_price_cents: i.unit_price_cents
            )
          else
            i
          end
        end
        CartState.new(
          customer_id: state.customer_id,
          items: new_items,
          coupon_code: state.coupon_code,
          discount_cents: state.discount_cents,
          status: state.status
        )
      elsif type_url.end_with?('ItemRemoved')
        event = Examples::ItemRemoved.decode(event_any.value)
        new_items = state.items.reject { |i| i.sku == event.sku }
        CartState.new(
          customer_id: state.customer_id,
          items: new_items,
          coupon_code: state.coupon_code,
          discount_cents: state.discount_cents,
          status: state.status
        )
      elsif type_url.end_with?('CouponApplied')
        event = Examples::CouponApplied.decode(event_any.value)
        CartState.new(
          customer_id: state.customer_id,
          items: state.items,
          coupon_code: event.coupon_code,
          discount_cents: event.discount_cents,
          status: state.status
        )
      elsif type_url.end_with?('CartCleared')
        CartState.new(
          customer_id: state.customer_id,
          items: [],
          coupon_code: '',
          discount_cents: 0,
          status: :cleared
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
