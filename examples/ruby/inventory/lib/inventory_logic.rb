# frozen_string_literal: true

require 'time'
require_relative 'command_validation_error'

# Pure business logic for inventory aggregate.
# No gRPC dependencies - can be tested in isolation.
module InventoryLogic
  Reservation = Struct.new(:order_id, :quantity, keyword_init: true)

  InventoryState = Struct.new(:sku, :on_hand, :reserved, :reservations, :low_stock_threshold, keyword_init: true) do
    def initialize(sku: '', on_hand: 0, reserved: 0, reservations: [], low_stock_threshold: 10)
      super
    end

    def exists?
      !sku.empty?
    end

    def available
      on_hand - reserved
    end
  end

  LOW_STOCK_THRESHOLD = 10

  class << self
    def rebuild_state(event_book)
      state = InventoryState.new

      return state if event_book.nil? || event_book.pages.empty?

      # Apply events
      event_book.pages.each do |page|
        next unless page.event

        state = apply_event(state, page.event)
      end

      state
    end

    def handle_initialize_stock(state, sku, initial_quantity, low_stock_threshold)
      raise CommandValidationError.failed_precondition('Inventory already exists') if state.exists?
      raise CommandValidationError.invalid_argument('SKU is required') if sku.nil? || sku.empty?
      raise CommandValidationError.invalid_argument('Quantity must be non-negative') if initial_quantity < 0

      Examples::StockInitialized.new(
        sku: sku,
        initial_quantity: initial_quantity,
        low_stock_threshold: low_stock_threshold || LOW_STOCK_THRESHOLD
      )
    end

    def handle_receive_stock(state, quantity, reference)
      raise CommandValidationError.failed_precondition('Inventory does not exist') unless state.exists?
      raise CommandValidationError.invalid_argument('Quantity must be positive') if quantity <= 0

      Examples::StockReceived.new(
        quantity: quantity,
        new_on_hand: state.on_hand + quantity,
        reference: reference || ''
      )
    end

    def handle_reserve_stock(state, order_id, quantity)
      raise CommandValidationError.failed_precondition('Inventory does not exist') unless state.exists?
      raise CommandValidationError.invalid_argument('Order ID is required') if order_id.nil? || order_id.empty?
      raise CommandValidationError.invalid_argument('Quantity must be positive') if quantity <= 0

      if quantity > state.available
        raise CommandValidationError.failed_precondition(
          "Insufficient stock: available #{state.available}, requested #{quantity}"
        )
      end

      existing = state.reservations.find { |r| r.order_id == order_id }
      raise CommandValidationError.failed_precondition("Reservation already exists for order #{order_id}") if existing

      events = []
      events << Examples::StockReserved.new(
        order_id: order_id,
        quantity: quantity,
        new_available: state.available - quantity
      )

      new_available = state.available - quantity
      if new_available <= state.low_stock_threshold && state.available > state.low_stock_threshold
        events << Examples::LowStockAlert.new(
          sku: state.sku,
          current_available: new_available,
          threshold: state.low_stock_threshold
        )
      end

      events
    end

    def handle_release_reservation(state, order_id)
      raise CommandValidationError.failed_precondition('Inventory does not exist') unless state.exists?
      raise CommandValidationError.invalid_argument('Order ID is required') if order_id.nil? || order_id.empty?

      reservation = state.reservations.find { |r| r.order_id == order_id }
      raise CommandValidationError.failed_precondition("No reservation found for order #{order_id}") unless reservation

      Examples::ReservationReleased.new(
        order_id: order_id,
        quantity: reservation.quantity,
        new_available: state.available + reservation.quantity
      )
    end

    def handle_commit_reservation(state, order_id)
      raise CommandValidationError.failed_precondition('Inventory does not exist') unless state.exists?
      raise CommandValidationError.invalid_argument('Order ID is required') if order_id.nil? || order_id.empty?

      reservation = state.reservations.find { |r| r.order_id == order_id }
      raise CommandValidationError.failed_precondition("No reservation found for order #{order_id}") unless reservation

      Examples::ReservationCommitted.new(
        order_id: order_id,
        quantity: reservation.quantity,
        new_on_hand: state.on_hand - reservation.quantity
      )
    end

    private

    def apply_event(state, event_any)
      type_url = event_any.type_url

      if type_url.end_with?('StockInitialized')
        event = Examples::StockInitialized.decode(event_any.value)
        InventoryState.new(
          sku: event.sku,
          on_hand: event.initial_quantity,
          reserved: 0,
          reservations: [],
          low_stock_threshold: event.low_stock_threshold
        )
      elsif type_url.end_with?('StockReceived')
        event = Examples::StockReceived.decode(event_any.value)
        InventoryState.new(
          sku: state.sku,
          on_hand: event.new_on_hand,
          reserved: state.reserved,
          reservations: state.reservations,
          low_stock_threshold: state.low_stock_threshold
        )
      elsif type_url.end_with?('StockReserved')
        event = Examples::StockReserved.decode(event_any.value)
        new_reservations = state.reservations + [Reservation.new(order_id: event.order_id, quantity: event.quantity)]
        InventoryState.new(
          sku: state.sku,
          on_hand: state.on_hand,
          reserved: state.reserved + event.quantity,
          reservations: new_reservations,
          low_stock_threshold: state.low_stock_threshold
        )
      elsif type_url.end_with?('ReservationReleased')
        event = Examples::ReservationReleased.decode(event_any.value)
        new_reservations = state.reservations.reject { |r| r.order_id == event.order_id }
        InventoryState.new(
          sku: state.sku,
          on_hand: state.on_hand,
          reserved: state.reserved - event.quantity,
          reservations: new_reservations,
          low_stock_threshold: state.low_stock_threshold
        )
      elsif type_url.end_with?('ReservationCommitted')
        event = Examples::ReservationCommitted.decode(event_any.value)
        new_reservations = state.reservations.reject { |r| r.order_id == event.order_id }
        InventoryState.new(
          sku: state.sku,
          on_hand: event.new_on_hand,
          reserved: state.reserved - event.quantity,
          reservations: new_reservations,
          low_stock_threshold: state.low_stock_threshold
        )
      else
        state
      end
    end
  end
end
