# frozen_string_literal: true

require 'time'
require_relative 'command_validation_error'

# Pure business logic for fulfillment aggregate.
# No gRPC dependencies - can be tested in isolation.
module FulfillmentLogic
  # State machine: pending -> picking -> packing -> shipped -> delivered
  FulfillmentState = Struct.new(:order_id, :status, :tracking_number, :carrier,
                                 :shipped_at, :delivered_at, keyword_init: true) do
    def initialize(order_id: '', status: :uninitialized, tracking_number: '',
                   carrier: '', shipped_at: nil, delivered_at: nil)
      super
    end

    def exists?
      status != :uninitialized
    end
  end

  class << self
    def rebuild_state(event_book)
      state = FulfillmentState.new

      return state if event_book.nil? || event_book.pages.empty?

      # Apply events
      event_book.pages.each do |page|
        next unless page.event

        state = apply_event(state, page.event)
      end

      state
    end

    def handle_create_shipment(state, order_id, items)
      raise CommandValidationError.failed_precondition('Shipment already exists') if state.exists?
      raise CommandValidationError.invalid_argument('Order ID is required') if order_id.nil? || order_id.empty?

      Examples::ShipmentCreated.new(
        order_id: order_id,
        items: items || [],
        created_at: now_timestamp
      )
    end

    def handle_mark_picked(state)
      raise CommandValidationError.failed_precondition('Shipment does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition("Cannot pick from status #{state.status}") unless state.status == :pending

      Examples::ItemsPicked.new(picked_at: now_timestamp)
    end

    def handle_mark_packed(state)
      raise CommandValidationError.failed_precondition('Shipment does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition("Cannot pack from status #{state.status}") unless state.status == :picking

      Examples::ItemsPacked.new(packed_at: now_timestamp)
    end

    def handle_ship(state, tracking_number, carrier)
      raise CommandValidationError.failed_precondition('Shipment does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition("Cannot ship from status #{state.status}") unless state.status == :packing
      raise CommandValidationError.invalid_argument('Tracking number is required') if tracking_number.nil? || tracking_number.empty?
      raise CommandValidationError.invalid_argument('Carrier is required') if carrier.nil? || carrier.empty?

      Examples::Shipped.new(
        tracking_number: tracking_number,
        carrier: carrier,
        shipped_at: now_timestamp
      )
    end

    def handle_record_delivery(state)
      raise CommandValidationError.failed_precondition('Shipment does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition("Cannot deliver from status #{state.status}") unless state.status == :shipped

      Examples::Delivered.new(delivered_at: now_timestamp)
    end

    private

    def apply_event(state, event_any)
      type_url = event_any.type_url

      if type_url.end_with?('ShipmentCreated')
        event = Examples::ShipmentCreated.decode(event_any.value)
        FulfillmentState.new(
          order_id: event.order_id,
          status: :pending,
          tracking_number: '',
          carrier: '',
          shipped_at: nil,
          delivered_at: nil
        )
      elsif type_url.end_with?('ItemsPicked')
        FulfillmentState.new(
          order_id: state.order_id,
          status: :picking,
          tracking_number: state.tracking_number,
          carrier: state.carrier,
          shipped_at: state.shipped_at,
          delivered_at: state.delivered_at
        )
      elsif type_url.end_with?('ItemsPacked')
        FulfillmentState.new(
          order_id: state.order_id,
          status: :packing,
          tracking_number: state.tracking_number,
          carrier: state.carrier,
          shipped_at: state.shipped_at,
          delivered_at: state.delivered_at
        )
      elsif type_url.end_with?('Shipped')
        event = Examples::Shipped.decode(event_any.value)
        FulfillmentState.new(
          order_id: state.order_id,
          status: :shipped,
          tracking_number: event.tracking_number,
          carrier: event.carrier,
          shipped_at: event.shipped_at,
          delivered_at: state.delivered_at
        )
      elsif type_url.end_with?('Delivered')
        event = Examples::Delivered.decode(event_any.value)
        FulfillmentState.new(
          order_id: state.order_id,
          status: :delivered,
          tracking_number: state.tracking_number,
          carrier: state.carrier,
          shipped_at: state.shipped_at,
          delivered_at: event.delivered_at
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
