#!/usr/bin/env ruby
# frozen_string_literal: true

# Inventory Service - Ruby Implementation
# Entry point for the inventory business logic gRPC server.

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'
require_relative 'lib/inventory_logic'
require_relative 'lib/command_validation_error'

DOMAIN = 'inventory'

# gRPC service adapter for inventory business logic.
# Thin wrapper that delegates to InventoryLogic and maps exceptions to gRPC status codes.
class InventoryService < Angzarr::BusinessLogic::Service
  def handle(contextual_cmd, _call)
    cmd_book = contextual_cmd.command
    prior_events = contextual_cmd.events

    raise GRPC::InvalidArgument, 'CommandBook has no pages' if cmd_book.pages.empty?

    cmd_page = cmd_book.pages.first
    raise GRPC::InvalidArgument, 'Command page has no command' unless cmd_page.command

    state = InventoryLogic.rebuild_state(prior_events)
    type_url = cmd_page.command.type_url
    cmd_data = cmd_page.command.value

    events = dispatch_command(type_url, cmd_data, state)
    events = [events] unless events.is_a?(Array)
    event_book = create_event_book(cmd_book.cover, events)

    Angzarr::BusinessResponse.new(events: event_book)
  rescue CommandValidationError => e
    raise_grpc_error(e)
  end

  private

  def dispatch_command(type_url, cmd_data, state)
    case
    when type_url.end_with?('InitializeStock')
      cmd = Examples::InitializeStock.decode(cmd_data)
      log_info('initializing_stock', sku: cmd.sku, quantity: cmd.initial_quantity)
      InventoryLogic.handle_initialize_stock(state, cmd.sku, cmd.initial_quantity, cmd.low_stock_threshold)
    when type_url.end_with?('ReceiveStock')
      cmd = Examples::ReceiveStock.decode(cmd_data)
      log_info('receiving_stock', quantity: cmd.quantity, reference: cmd.reference)
      InventoryLogic.handle_receive_stock(state, cmd.quantity, cmd.reference)
    when type_url.end_with?('ReserveStock')
      cmd = Examples::ReserveStock.decode(cmd_data)
      log_info('reserving_stock', order_id: cmd.order_id, quantity: cmd.quantity)
      InventoryLogic.handle_reserve_stock(state, cmd.order_id, cmd.quantity)
    when type_url.end_with?('ReleaseReservation')
      cmd = Examples::ReleaseReservation.decode(cmd_data)
      log_info('releasing_reservation', order_id: cmd.order_id)
      InventoryLogic.handle_release_reservation(state, cmd.order_id)
    when type_url.end_with?('CommitReservation')
      cmd = Examples::CommitReservation.decode(cmd_data)
      log_info('committing_reservation', order_id: cmd.order_id)
      InventoryLogic.handle_commit_reservation(state, cmd.order_id)
    else
      raise GRPC::InvalidArgument, "Unknown command type: #{type_url}"
    end
  end

  def create_event_book(cover, events)
    pages = events.map.with_index do |event, idx|
      type_name = event.class.name.split('::').last
      type_name = "examples.#{type_name}"

      event_any = Google::Protobuf::Any.new(
        type_url: "type.examples/#{type_name}",
        value: event.to_proto
      )

      Angzarr::EventPage.new(
        num: idx,
        event: event_any,
        created_at: Google::Protobuf::Timestamp.new(seconds: Time.now.to_i)
      )
    end

    Angzarr::EventBook.new(cover: cover, pages: pages)
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
  port = ENV.fetch('PORT', '50804')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(InventoryService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'business_logic_server_started', domain: DOMAIN, port: port, timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
