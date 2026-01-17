#!/usr/bin/env ruby
# frozen_string_literal: true

# Loyalty Earn Saga Service - Ruby Implementation
# Awards loyalty points to customers when orders are delivered.

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'

DOMAIN = 'saga-loyalty-earn'
POINTS_PER_DOLLAR = 10

class LoyaltyEarnSagaService < Angzarr::Saga::Service
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

    # Look for Delivered event and calculate points from order totals
    delivered = false
    total_cents = 0

    event_book.pages.each do |page|
      next unless page.event

      type_url = page.event.type_url
      if type_url.end_with?('Delivered')
        delivered = true
      elsif type_url.end_with?('PaymentSubmitted')
        event = Examples::PaymentSubmitted.decode(page.event.value)
        total_cents = event.amount_cents
      end
    end

    return commands unless delivered && total_cents > 0

    order_id = extract_order_id(event_book)
    points_to_award = (total_cents / 100) * POINTS_PER_DOLLAR

    return commands if points_to_award <= 0

    log_info('awarding_loyalty_points', order_id: order_id, points: points_to_award, total_cents: total_cents)

    add_points_cmd = Examples::AddLoyaltyPoints.new(
      points: points_to_award,
      reason: "Order delivery: #{order_id}"
    )

    cmd_book = Angzarr::CommandBook.new(
      cover: Angzarr::Cover.new(domain: 'customer'),
      correlation_id: event_book.correlation_id
    )
    cmd_book.pages << Angzarr::CommandPage.new(
      sequence: 0,
      synchronous: false,
      command: Google::Protobuf::Any.new(
        type_url: 'type.examples/examples.AddLoyaltyPoints',
        value: add_points_cmd.to_proto
      )
    )

    commands << cmd_book

    log_info('loyalty_earn_saga_completed', points_awarded: points_to_award)

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
  port = ENV.fetch('PORT', '50808')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(LoyaltyEarnSagaService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'saga_server_started', saga: 'loyalty-earn', port: port, source_domain: 'fulfillment', timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
