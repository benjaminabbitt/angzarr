#!/usr/bin/env ruby
# frozen_string_literal: true

# Cancellation Saga Service - Ruby Implementation
# Handles compensation when orders are cancelled (releases inventory, refunds loyalty points).

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'

DOMAIN = 'saga-cancellation'

class CancellationSagaService < Angzarr::Saga::Service
  def handle(event_book, _call)
    process_events(event_book)
    Google::Protobuf::Empty.new
  end

  def handle_sync(event_book, _call)
    commands = process_events(event_book)
    response = Angzarr::SagaResponse.new
    commands.each { |cmd| response.commands << cmd }
    response
  end

  private

  def process_events(event_book)
    commands = []

    return commands if event_book.pages.empty?

    event_book.pages.each do |page|
      next unless page.event

      type_url = page.event.type_url
      next unless type_url.end_with?('OrderCancelled')

      cancelled_event = Examples::OrderCancelled.decode(page.event.value)
      order_id = extract_order_id(event_book)

      next if order_id.empty?

      log_info('processing_order_cancellation', order_id: order_id)

      # Release inventory reservation
      release_cmd = Examples::ReleaseReservation.new(order_id: order_id)

      release_cmd_book = Angzarr::CommandBook.new(
        cover: Angzarr::Cover.new(
          domain: 'inventory',
          root: event_book.cover&.root
        ),
        correlation_id: event_book.correlation_id
      )
      release_cmd_book.pages << Angzarr::CommandPage.new(
        sequence: 0,
        synchronous: false,
        command: Google::Protobuf::Any.new(
          type_url: 'type.examples/examples.ReleaseReservation',
          value: release_cmd.to_proto
        )
      )

      commands << release_cmd_book

      # Refund loyalty points if any were used
      if cancelled_event.loyalty_points_used > 0
        add_points_cmd = Examples::AddLoyaltyPoints.new(
          points: cancelled_event.loyalty_points_used,
          reason: 'Order cancellation refund'
        )

        add_points_cmd_book = Angzarr::CommandBook.new(
          cover: Angzarr::Cover.new(domain: 'customer'),
          correlation_id: event_book.correlation_id
        )
        add_points_cmd_book.pages << Angzarr::CommandPage.new(
          sequence: 0,
          synchronous: false,
          command: Google::Protobuf::Any.new(
            type_url: 'type.examples/examples.AddLoyaltyPoints',
            value: add_points_cmd.to_proto
          )
        )

        commands << add_points_cmd_book
      end
    end

    if commands.any?
      log_info('cancellation_saga_completed', compensation_commands: commands.size)
    end

    commands
  end

  def extract_order_id(event_book)
    return '' unless event_book.cover&.root&.value

    event_book.cover.root.value.unpack1('H*')
  end

  def log_info(message, **fields)
    puts JSON.generate({ level: 'info', message: message, domain: DOMAIN, **fields, timestamp: Time.now.iso8601 })
  end
end

def main
  port = ENV.fetch('PORT', '50809')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(CancellationSagaService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'saga_server_started', saga: 'cancellation', port: port, source_domain: 'order', timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
