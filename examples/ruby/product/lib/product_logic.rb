# frozen_string_literal: true

require 'time'
require_relative 'command_validation_error'

# Pure business logic for product aggregate.
# No gRPC dependencies - can be tested in isolation.
module ProductLogic
  ProductState = Struct.new(:sku, :name, :description, :price_cents, :status, keyword_init: true) do
    def initialize(sku: '', name: '', description: '', price_cents: 0, status: :uninitialized)
      super
    end

    def exists?
      status != :uninitialized
    end

    def active?
      status == :active
    end
  end

  class << self
    def rebuild_state(event_book)
      state = ProductState.new

      return state if event_book.nil? || event_book.pages.empty?

      # Start from snapshot if present
      if event_book.snapshot&.state
        snap = event_book.snapshot.state
        if snap.type_url.end_with?('ProductState')
          snap_state = Examples::ProductState.decode(snap.value)
          state = ProductState.new(
            sku: snap_state.sku,
            name: snap_state.name,
            description: snap_state.description,
            price_cents: snap_state.price_cents,
            status: snap_state.status.to_sym
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

    def handle_create_product(state, sku, name, description, price_cents)
      raise CommandValidationError.failed_precondition('Product already exists') if state.exists?
      raise CommandValidationError.invalid_argument('SKU is required') if sku.nil? || sku.empty?
      raise CommandValidationError.invalid_argument('Name is required') if name.nil? || name.empty?
      raise CommandValidationError.invalid_argument('Price must be positive') if price_cents <= 0

      Examples::ProductCreated.new(
        sku: sku,
        name: name,
        description: description || '',
        price_cents: price_cents,
        created_at: now_timestamp
      )
    end

    def handle_update_product(state, name, description)
      raise CommandValidationError.failed_precondition('Product does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Product is discontinued') unless state.active?

      Examples::ProductUpdated.new(
        name: name || state.name,
        description: description || state.description
      )
    end

    def handle_set_price(state, price_cents)
      raise CommandValidationError.failed_precondition('Product does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Product is discontinued') unless state.active?
      raise CommandValidationError.invalid_argument('Price must be positive') if price_cents <= 0

      Examples::PriceSet.new(
        old_price_cents: state.price_cents,
        new_price_cents: price_cents
      )
    end

    def handle_discontinue(state, reason)
      raise CommandValidationError.failed_precondition('Product does not exist') unless state.exists?
      raise CommandValidationError.failed_precondition('Product already discontinued') unless state.active?

      Examples::ProductDiscontinued.new(
        reason: reason || '',
        discontinued_at: now_timestamp
      )
    end

    private

    def apply_event(state, event_any)
      type_url = event_any.type_url

      if type_url.end_with?('ProductCreated')
        event = Examples::ProductCreated.decode(event_any.value)
        ProductState.new(
          sku: event.sku,
          name: event.name,
          description: event.description,
          price_cents: event.price_cents,
          status: :active
        )
      elsif type_url.end_with?('ProductUpdated')
        event = Examples::ProductUpdated.decode(event_any.value)
        ProductState.new(
          sku: state.sku,
          name: event.name,
          description: event.description,
          price_cents: state.price_cents,
          status: state.status
        )
      elsif type_url.end_with?('PriceSet')
        event = Examples::PriceSet.decode(event_any.value)
        ProductState.new(
          sku: state.sku,
          name: state.name,
          description: state.description,
          price_cents: event.new_price_cents,
          status: state.status
        )
      elsif type_url.end_with?('ProductDiscontinued')
        ProductState.new(
          sku: state.sku,
          name: state.name,
          description: state.description,
          price_cents: state.price_cents,
          status: :discontinued
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
