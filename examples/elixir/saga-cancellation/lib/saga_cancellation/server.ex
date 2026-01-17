defmodule SagaCancellation.Server do
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
      |> Enum.filter(fn page ->
        page.event && String.ends_with?(page.event.type_url, "OrderCancelled")
      end)
      |> Enum.flat_map(fn page ->
        cancelled_event = Examples.OrderCancelled.decode(page.event.value)
        order_id = extract_order_id(event_book)

        if order_id == "" do
          []
        else
          log_info("processing_order_cancellation", %{order_id: order_id})

          release_cmd = Examples.ReleaseReservation.new(order_id: order_id)

          release_cmd_book = Angzarr.CommandBook.new(
            cover: Angzarr.Cover.new(domain: "inventory", root: event_book.cover && event_book.cover.root),
            correlation_id: event_book.correlation_id,
            pages: [
              Angzarr.CommandPage.new(
                sequence: 0,
                synchronous: false,
                command: Google.Protobuf.Any.new(
                  type_url: "type.examples/examples.ReleaseReservation",
                  value: Protobuf.encode(release_cmd)
                )
              )
            ]
          )

          commands = [release_cmd_book]

          commands = if cancelled_event.loyalty_points_used > 0 do
            add_points_cmd = Examples.AddLoyaltyPoints.new(
              points: cancelled_event.loyalty_points_used,
              reason: "Order cancellation refund"
            )

            add_points_cmd_book = Angzarr.CommandBook.new(
              cover: Angzarr.Cover.new(domain: "customer"),
              correlation_id: event_book.correlation_id,
              pages: [
                Angzarr.CommandPage.new(
                  sequence: 0,
                  synchronous: false,
                  command: Google.Protobuf.Any.new(
                    type_url: "type.examples/examples.AddLoyaltyPoints",
                    value: Protobuf.encode(add_points_cmd)
                  )
                )
              ]
            )

            commands ++ [add_points_cmd_book]
          else
            commands
          end

          log_info("cancellation_saga_completed", %{compensation_commands: length(commands)})
          commands
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
      domain: "saga-cancellation",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })))
  end
end
