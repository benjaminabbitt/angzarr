#!/usr/bin/env ruby
# frozen_string_literal: true

# Cart Service - Ruby Implementation
# Entry point for the cart business logic gRPC server.

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'
require_relative 'lib/cart_logic'
require_relative 'lib/command_validation_error'

DOMAIN = 'cart'

# gRPC service adapter for cart business logic.
# Thin wrapper that delegates to CartLogic and maps exceptions to gRPC status codes.
class CartService < Angzarr::BusinessLogic::Service
  def handle(contextual_cmd, _call)
    cmd_book = contextual_cmd.command
    prior_events = contextual_cmd.events

    raise GRPC::InvalidArgument, 'CommandBook has no pages' if cmd_book.pages.empty?

    cmd_page = cmd_book.pages.first
    raise GRPC::InvalidArgument, 'Command page has no command' unless cmd_page.command

    state = CartLogic.rebuild_state(prior_events)
    type_url = cmd_page.command.type_url
    cmd_data = cmd_page.command.value

    event = dispatch_command(type_url, cmd_data, state)
    event_book = create_event_book(cmd_book.cover, event)

    Angzarr::BusinessResponse.new(events: event_book)
  rescue CommandValidationError => e
    raise_grpc_error(e)
  end

  private

  def dispatch_command(type_url, cmd_data, state)
    case
    when type_url.end_with?('CreateCart')
      cmd = Examples::CreateCart.decode(cmd_data)
      log_info('creating_cart', customer_id: cmd.customer_id)
      CartLogic.handle_create_cart(state, cmd.customer_id)
    when type_url.end_with?('AddItem')
      cmd = Examples::AddItem.decode(cmd_data)
      log_info('adding_item', sku: cmd.sku, quantity: cmd.quantity)
      CartLogic.handle_add_item(state, cmd.sku, cmd.name, cmd.quantity, cmd.unit_price_cents)
    when type_url.end_with?('UpdateQuantity')
      cmd = Examples::UpdateQuantity.decode(cmd_data)
      log_info('updating_quantity', sku: cmd.sku, quantity: cmd.quantity)
      CartLogic.handle_update_quantity(state, cmd.sku, cmd.quantity)
    when type_url.end_with?('RemoveItem')
      cmd = Examples::RemoveItem.decode(cmd_data)
      log_info('removing_item', sku: cmd.sku)
      CartLogic.handle_remove_item(state, cmd.sku)
    when type_url.end_with?('ApplyCoupon')
      cmd = Examples::ApplyCoupon.decode(cmd_data)
      log_info('applying_coupon', coupon_code: cmd.coupon_code)
      CartLogic.handle_apply_coupon(state, cmd.coupon_code, cmd.discount_cents)
    when type_url.end_with?('ClearCart')
      log_info('clearing_cart')
      CartLogic.handle_clear_cart(state)
    when type_url.end_with?('Checkout')
      cmd = Examples::Checkout.decode(cmd_data)
      log_info('checkout_requested', loyalty_points_to_use: cmd.loyalty_points_to_use)
      CartLogic.handle_checkout(state, cmd.loyalty_points_to_use)
    else
      raise GRPC::InvalidArgument, "Unknown command type: #{type_url}"
    end
  end

  def create_event_book(cover, event)
    type_name = event.class.name.split('::').last
    type_name = "examples.#{type_name}"

    event_any = Google::Protobuf::Any.new(
      type_url: "type.examples/#{type_name}",
      value: event.to_proto
    )

    Angzarr::EventBook.new(
      cover: cover,
      pages: [
        Angzarr::EventPage.new(
          num: 0,
          event: event_any,
          created_at: Google::Protobuf::Timestamp.new(seconds: Time.now.to_i)
        )
      ]
    )
  end

  def raise_grpc_error(error)
    case error.status_code
    when CommandValidationError::INVALID_ARGUMENT
      raise GRPC::InvalidArgument, error.message
    when CommandValidationError::FAILED_PRECONDITION
      raise GRPC::FailedPrecondition, error.message
    else
      raise GRPC::Unknown, error.message
    end
  end

  def log_info(message, **fields)
    puts JSON.generate({ level: 'info', message: message, domain: DOMAIN, **fields, timestamp: Time.now.iso8601 })
  end
end

def main
  port = ENV.fetch('PORT', '50802')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(CartService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'business_logic_server_started', domain: DOMAIN, port: port, timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
