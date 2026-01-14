#!/usr/bin/env ruby
# frozen_string_literal: true

# Customer Log Projector Service - Ruby Implementation
# Logs customer domain events for audit/debugging.

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'
require_relative 'lib/log_projector_logic'

DOMAIN = 'projector-log-customer'

class CustomerLogProjectorService < Angzarr::ProjectorCoordinator::Service
  def handle_sync(event_book, _call)
    log_events(event_book)
    # Log projector doesn't produce a projection
    nil
  end

  def handle_async(event_book, _call)
    log_events(event_book)
    Angzarr::ProjectorAsyncResponse.new
  end

  private

  def log_events(event_book)
    return if event_book.pages.empty?

    event_book.pages.each do |page|
      next unless page.event

      log_entry = LogProjectorLogic.process_event(page.event)
      next unless log_entry

      log_data = LogProjectorLogic.format_log(
        log_entry,
        domain: event_book.cover&.domain,
        aggregate_id: event_book.cover&.id,
        sequence: page.num,
        created_at: page.created_at ? Time.at(page.created_at.seconds).iso8601 : nil
      )

      puts JSON.generate(log_data)
    end
  end
end

def main
  port = ENV.fetch('PORT', '50075')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(CustomerLogProjectorService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'projector server started', domain: DOMAIN, port: port, timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
