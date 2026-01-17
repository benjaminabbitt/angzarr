#!/usr/bin/env ruby
# frozen_string_literal: true

# Receipt Projector Service - Ruby Implementation
# Generates receipt projections from order events.

require 'grpc'
require 'grpc/health/checker'
require 'grpc/health/v1/health_services_pb'
require 'json'
require 'time'

# Load generated proto files
$LOAD_PATH.unshift(File.join(__dir__, 'lib'))
require 'angzarr/angzarr_services_pb'
require 'examples/domains_pb'
require_relative 'lib/receipt_projector_logic'

DOMAIN = 'projector-receipt'

class ReceiptProjectorService < Angzarr::ProjectorCoordinator::Service
  def handle_sync(event_book, _call)
    receipt_data = ReceiptProjectorLogic.project(event_book)
    return nil unless receipt_data

    order_id = extract_order_id(event_book)
    formatted_text = ReceiptProjectorLogic.format_receipt(receipt_data, order_id: order_id)

    short_order_id = order_id.length > 16 ? order_id[0...16] : order_id

    log_info('generated_receipt',
             order_id: short_order_id,
             total_cents: receipt_data.final_total_cents,
             payment_method: receipt_data.payment_method)

    receipt = Examples::Receipt.new(
      order_id: order_id,
      customer_id: receipt_data.customer_id,
      items: receipt_data.items,
      subtotal_cents: receipt_data.subtotal_cents,
      discount_cents: receipt_data.discount_cents,
      final_total_cents: receipt_data.final_total_cents,
      payment_method: receipt_data.payment_method,
      loyalty_points_earned: receipt_data.loyalty_points_earned,
      formatted_text: formatted_text
    )

    Angzarr::Projection.new(
      data: Google::Protobuf::Any.new(
        type_url: 'type.examples/examples.Receipt',
        value: receipt.to_proto
      )
    )
  end

  def handle_async(event_book, _call)
    log_info('async_receipt_projection', pages: event_book.pages.size)
    Angzarr::ProjectorAsyncResponse.new
  end

  private

  def extract_order_id(event_book)
    return '' unless event_book.cover&.root&.value

    event_book.cover.root.value.unpack1('H*')
  end

  def log_info(message, **fields)
    puts JSON.generate({ level: 'info', message: message, domain: DOMAIN, **fields, timestamp: Time.now.iso8601 })
  end
end

def main
  port = ENV.fetch('PORT', '50810')

  server = GRPC::RpcServer.new
  server.add_http2_port("0.0.0.0:#{port}", :this_port_is_insecure)
  server.handle(ReceiptProjectorService)

  # Register gRPC health service
  health_checker = Grpc::Health::Checker.new
  health_checker.add_status('', Grpc::Health::V1::HealthCheckResponse::ServingStatus::SERVING)
  server.handle(health_checker)

  puts JSON.generate({ level: 'info', message: 'projector_server_started', domain: DOMAIN, port: port, listens_to: 'order domain', timestamp: Time.now.iso8601 })

  server.run_till_terminated
end

main if __FILE__ == $PROGRAM_NAME
