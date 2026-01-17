defmodule SagaFulfillment.Server do
  use GRPC.Server, service: Angzarr.Saga.Service
  require Logger

  def handle(event_book, _stream) do
    process_events(event_book)
    Google.Protobuf.Empty.new()
  end

  def handle_sync(event_book, _stream) do
    commands = process_events(event_book)
    Angzarr.SagaResponse.new(commands: commands)
  end

  defp process_events(event_book) do
    if Enum.empty?(event_book.pages) do
      []
    else
      event_book.pages
      |> Enum.filter(fn page -> page.event && String.ends_with?(page.event.type_url, "PaymentConfirmed") end)
      |> Enum.flat_map(fn _page ->
        order_id = extract_order_id(event_book)

        if order_id == "" do
          []
        else
          log_info("processing_payment_confirmed", %{order_id: order_id})

          create_shipment_cmd = Examples.CreateShipment.new(order_id: order_id, items: [])
          commit_reservation_cmd = Examples.CommitReservation.new(order_id: order_id)

          shipment_cmd_book = Angzarr.CommandBook.new(
            cover: Angzarr.Cover.new(domain: "fulfillment", root: event_book.cover && event_book.cover.root),
            correlation_id: event_book.correlation_id,
            pages: [
              Angzarr.CommandPage.new(
                sequence: 0,
                synchronous: false,
                command: Google.Protobuf.Any.new(
                  type_url: "type.examples/examples.CreateShipment",
                  value: Protobuf.encode(create_shipment_cmd)
                )
              )
            ]
          )

          commit_cmd_book = Angzarr.CommandBook.new(
            cover: Angzarr.Cover.new(domain: "inventory", root: event_book.cover && event_book.cover.root),
            correlation_id: event_book.correlation_id,
            pages: [
              Angzarr.CommandPage.new(
                sequence: 0,
                synchronous: false,
                command: Google.Protobuf.Any.new(
                  type_url: "type.examples/examples.CommitReservation",
                  value: Protobuf.encode(commit_reservation_cmd)
                )
              )
            ]
          )

          log_info("fulfillment_saga_completed", %{commands_generated: 2})
          [shipment_cmd_book, commit_cmd_book]
        end
      end)
    end
  end

  defp extract_order_id(event_book) do
    case event_book.cover do
      %{root: %{value: value}} when is_binary(value) and byte_size(value) > 0 ->
        Base.encode16(value, case: :lower)
      _ ->
        ""
    end
  end

  defp log_info(message, fields) do
    Logger.info(Jason.encode!(Map.merge(fields, %{
      level: "info",
      message: message,
      domain: "saga-fulfillment",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })))
  end
end
