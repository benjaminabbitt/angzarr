#!/usr/bin/env ruby
# frozen_string_literal: true

# Order Service - Ruby Implementation
# Entry point for the order business logic gRPC server.

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'
require_relative 'lib/order_logic'
require_relative 'lib/command_validation_error'

DOMAIN = 'order'

# gRPC service adapter for order business logic.
# Thin wrapper that delegates to OrderLogic and maps exceptions to gRPC status codes.
class OrderService < Angzarr::BusinessLogic::Service
  def handle(contextual_cmd, _call)
    cmd_book = contextual_cmd.command
    prior_events = contextual_cmd.events

    raise GRPC::InvalidArgument, 'CommandBook has no pages' if cmd_book.pages.empty?

    cmd_page = cmd_book.pages.first
    raise GRPC::InvalidArgument, 'Command page has no command' unless cmd_page.command

    state = OrderLogic.rebuild_state(prior_events)
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
    when type_url.end_with?('CreateOrder')
      cmd = Examples::CreateOrder.decode(cmd_data)
      log_info('creating_order', customer_id: cmd.customer_id, item_count: cmd.items.size)
      OrderLogic.handle_create_order(state, cmd.customer_id, cmd.items, cmd.subtotal_cents, cmd.discount_cents)
    when type_url.end_with?('ApplyLoyaltyDiscount')
      cmd = Examples::ApplyLoyaltyDiscount.decode(cmd_data)
      log_info('applying_loyalty_discount', points_used: cmd.points_used, discount_cents: cmd.discount_cents)
      OrderLogic.handle_apply_loyalty_discount(state, cmd.points_used, cmd.discount_cents)
    when type_url.end_with?('SubmitPayment')
      cmd = Examples::SubmitPayment.decode(cmd_data)
      log_info('submitting_payment', payment_method: cmd.payment_method, amount_cents: cmd.amount_cents)
      OrderLogic.handle_submit_payment(state, cmd.payment_method, cmd.amount_cents)
    when type_url.end_with?('ConfirmPayment')
      log_info('confirming_payment')
      OrderLogic.handle_confirm_payment(state)
    when type_url.end_with?('CompleteOrder')
      log_info('completing_order')
      OrderLogic.handle_complete_order(state)
    when type_url.end_with?('CancelOrder')
      cmd = Examples::CancelOrder.decode(cmd_data)
      log_info('cancelling_order', reason: cmd.reason)
      OrderLogic.handle_cancel_order(state, cmd.reason, cmd.loyalty_points_used)
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
  port = ENV.fetch('PORT', '50803')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(OrderService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'business_logic_server_started', domain: DOMAIN, port: port, timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
