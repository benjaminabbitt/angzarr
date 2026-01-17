#!/usr/bin/env ruby
# frozen_string_literal: true

# Fulfillment Service - Ruby Implementation
# Entry point for the fulfillment business logic gRPC server.

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'
require_relative 'lib/fulfillment_logic'
require_relative 'lib/command_validation_error'

DOMAIN = 'fulfillment'

# gRPC service adapter for fulfillment business logic.
# Thin wrapper that delegates to FulfillmentLogic and maps exceptions to gRPC status codes.
class FulfillmentService < Angzarr::BusinessLogic::Service
  def handle(contextual_cmd, _call)
    cmd_book = contextual_cmd.command
    prior_events = contextual_cmd.events

    raise GRPC::InvalidArgument, 'CommandBook has no pages' if cmd_book.pages.empty?

    cmd_page = cmd_book.pages.first
    raise GRPC::InvalidArgument, 'Command page has no command' unless cmd_page.command

    state = FulfillmentLogic.rebuild_state(prior_events)
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
    when type_url.end_with?('CreateShipment')
      cmd = Examples::CreateShipment.decode(cmd_data)
      log_info('creating_shipment', order_id: cmd.order_id)
      FulfillmentLogic.handle_create_shipment(state, cmd.order_id, cmd.items)
    when type_url.end_with?('MarkPicked')
      log_info('marking_picked')
      FulfillmentLogic.handle_mark_picked(state)
    when type_url.end_with?('MarkPacked')
      log_info('marking_packed')
      FulfillmentLogic.handle_mark_packed(state)
    when type_url.end_with?('Ship')
      cmd = Examples::Ship.decode(cmd_data)
      log_info('shipping', tracking_number: cmd.tracking_number, carrier: cmd.carrier)
      FulfillmentLogic.handle_ship(state, cmd.tracking_number, cmd.carrier)
    when type_url.end_with?('RecordDelivery')
      log_info('recording_delivery')
      FulfillmentLogic.handle_record_delivery(state)
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
  port = ENV.fetch('PORT', '50805')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(FulfillmentService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'business_logic_server_started', domain: DOMAIN, port: port, timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
