#!/usr/bin/env ruby
# frozen_string_literal: true

# Loyalty Saga Service - Ruby Implementation
# Awards loyalty points when transactions are completed.

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'
require_relative 'lib/loyalty_saga_logic'

DOMAIN = 'saga-loyalty'

class LoyaltySagaService < Angzarr::Saga::Service
  def handle(event_book, _call)
    return empty_response if event_book.pages.empty?

    transaction_id = event_book.cover&.id
    command_results = LoyaltySagaLogic.process(event_book, transaction_id: transaction_id)

    commands = command_results.map do |result|
      build_command_book(result, event_book.cover)
    end

    Angzarr::SagaResponse.new(commands: commands)
  end

  private

  def build_command_book(result, source_cover)
    # Extract customer_id from cover - assumes format "customer:{id}" or uses transaction's associated customer
    customer_id = extract_customer_id(source_cover)

    log_info('awarding loyalty points', customer_id: customer_id, points: result.points)

    cmd = Examples::AddLoyaltyPoints.new(
      points: result.points,
      reason: result.reason
    )

    cmd_any = Google::Protobuf::Any.new(
      type_url: 'type.examples/examples.AddLoyaltyPoints',
      value: cmd.to_proto
    )

    Angzarr::CommandBook.new(
      cover: Angzarr::Cover.new(
        domain: 'customer',
        id: customer_id
      ),
      pages: [
        Angzarr::CommandPage.new(
          command: cmd_any
        )
      ]
    )
  end

  def extract_customer_id(cover)
    # Transaction cover contains customer reference
    # This is a simplified lookup - in production would query event store
    cover&.id || ''
  end

  def empty_response
    Angzarr::SagaResponse.new(commands: [])
  end

  def log_info(message, **fields)
    puts JSON.generate({ level: 'info', message: message, domain: DOMAIN, **fields, timestamp: Time.now.iso8601 })
  end
end

def main
  port = ENV.fetch('PORT', '50054')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(LoyaltySagaService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'saga server started', domain: DOMAIN, port: port, timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
