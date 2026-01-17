#!/usr/bin/env ruby
# frozen_string_literal: true

# Product Service - Ruby Implementation
# Entry point for the product business logic gRPC server.

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'
require_relative 'lib/product_logic'
require_relative 'lib/command_validation_error'

DOMAIN = 'product'

# gRPC service adapter for product business logic.
# Thin wrapper that delegates to ProductLogic and maps exceptions to gRPC status codes.
class ProductService < Angzarr::BusinessLogic::Service
  def handle(contextual_cmd, _call)
    cmd_book = contextual_cmd.command
    prior_events = contextual_cmd.events

    raise GRPC::InvalidArgument, 'CommandBook has no pages' if cmd_book.pages.empty?

    cmd_page = cmd_book.pages.first
    raise GRPC::InvalidArgument, 'Command page has no command' unless cmd_page.command

    state = ProductLogic.rebuild_state(prior_events)
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
    when type_url.end_with?('CreateProduct')
      cmd = Examples::CreateProduct.decode(cmd_data)
      log_info('creating_product', sku: cmd.sku, name: cmd.name, price_cents: cmd.price_cents)
      ProductLogic.handle_create_product(state, cmd.sku, cmd.name, cmd.description, cmd.price_cents)
    when type_url.end_with?('UpdateProduct')
      cmd = Examples::UpdateProduct.decode(cmd_data)
      log_info('updating_product', name: cmd.name)
      ProductLogic.handle_update_product(state, cmd.name, cmd.description)
    when type_url.end_with?('SetPrice')
      cmd = Examples::SetPrice.decode(cmd_data)
      log_info('setting_price', price_cents: cmd.price_cents)
      ProductLogic.handle_set_price(state, cmd.price_cents)
    when type_url.end_with?('Discontinue')
      cmd = Examples::Discontinue.decode(cmd_data)
      log_info('discontinuing_product', reason: cmd.reason)
      ProductLogic.handle_discontinue(state, cmd.reason)
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
  port = ENV.fetch('PORT', '50801')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(ProductService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'business_logic_server_started', domain: DOMAIN, port: port, timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
