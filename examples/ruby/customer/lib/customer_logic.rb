# frozen_string_literal: true

require 'time'
require_relative 'command_validation_error'

# Pure business logic for customer aggregate.
# No gRPC dependencies - can be tested in isolation.
module CustomerLogic
  CustomerState = Struct.new(:name, :email, :loyalty_points, :lifetime_points, keyword_init: true) do
    def initialize(name: '', email: '', loyalty_points: 0, lifetime_points: 0)
      super
    end

    def exists?
      !name.empty?
    end
  end

  class << self
    def rebuild_state(event_book)
      state = CustomerState.new

      return state if event_book.nil? || event_book.pages.empty?

      # Start from snapshot if present
      if event_book.snapshot&.state
        snap = event_book.snapshot.state
        if snap.type_url.end_with?('CustomerState')
          snap_state = Examples::CustomerState.decode(snap.value)
          state = CustomerState.new(
            name: snap_state.name,
            email: snap_state.email,
            loyalty_points: snap_state.loyalty_points,
            lifetime_points: snap_state.lifetime_points
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

    def handle_create_customer(state, name, email)
      raise CommandValidationError.failed_precondition('Customer already exists') if state.exists?
      raise CommandValidationError.invalid_argument('Customer name is required') if name.nil? || name.empty?
      raise CommandValidationError.invalid_argument('Customer email is required') if email.nil? || email.empty?

      Examples::CustomerCreated.new(
        name: name,
        email: email,
        created_at: now_timestamp
      )
    end

    def handle_add_loyalty_points(state, points, reason)
      raise CommandValidationError.failed_precondition('Customer does not exist') unless state.exists?
      raise CommandValidationError.invalid_argument('Points must be positive') if points <= 0

      new_balance = state.loyalty_points + points

      Examples::LoyaltyPointsAdded.new(
        points: points,
        new_balance: new_balance,
        reason: reason || ''
      )
    end

    def handle_redeem_loyalty_points(state, points, redemption_type)
      raise CommandValidationError.failed_precondition('Customer does not exist') unless state.exists?
      raise CommandValidationError.invalid_argument('Points must be positive') if points <= 0
      if points > state.loyalty_points
        raise CommandValidationError.failed_precondition(
          "Insufficient points: have #{state.loyalty_points}, need #{points}"
        )
      end

      new_balance = state.loyalty_points - points

      Examples::LoyaltyPointsRedeemed.new(
        points: points,
        new_balance: new_balance,
        redemption_type: redemption_type || ''
      )
    end

    private

    def apply_event(state, event_any)
      type_url = event_any.type_url

      if type_url.end_with?('CustomerCreated')
        event = Examples::CustomerCreated.decode(event_any.value)
        CustomerState.new(
          name: event.name,
          email: event.email,
          loyalty_points: state.loyalty_points,
          lifetime_points: state.lifetime_points
        )
      elsif type_url.end_with?('LoyaltyPointsAdded')
        event = Examples::LoyaltyPointsAdded.decode(event_any.value)
        CustomerState.new(
          name: state.name,
          email: state.email,
          loyalty_points: event.new_balance,
          lifetime_points: state.lifetime_points + event.points
        )
      elsif type_url.end_with?('LoyaltyPointsRedeemed')
        event = Examples::LoyaltyPointsRedeemed.decode(event_any.value)
        CustomerState.new(
          name: state.name,
          email: state.email,
          loyalty_points: event.new_balance,
          lifetime_points: state.lifetime_points
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
