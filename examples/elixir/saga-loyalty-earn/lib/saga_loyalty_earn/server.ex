defmodule SagaLoyaltyEarn.Server do
  use GRPC.Server, service: Angzarr.Saga.Service
  require Logger

  @points_per_dollar 10

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
      delivered = Enum.any?(event_book.pages, fn page ->
        page.event && String.ends_with?(page.event.type_url, "Delivered")
      end)

      total_cents = event_book.pages
      |> Enum.find_value(0, fn page ->
        if page.event && String.ends_with?(page.event.type_url, "PaymentSubmitted") do
          event = Examples.PaymentSubmitted.decode(page.event.value)
          event.amount_cents
        end
      end)

      if delivered and total_cents > 0 do
        order_id = extract_order_id(event_book)
        points_to_award = div(total_cents, 100) * @points_per_dollar

        if points_to_award > 0 do
          log_info("awarding_loyalty_points", %{
            order_id: order_id,
            points: points_to_award,
            total_cents: total_cents
          })

          add_points_cmd = Examples.AddLoyaltyPoints.new(
            points: points_to_award,
            reason: "Order delivery: #{order_id}"
          )

          cmd_book = Angzarr.CommandBook.new(
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

          log_info("loyalty_earn_saga_completed", %{points_awarded: points_to_award})
          [cmd_book]
        else
          []
        end
      else
        []
      end
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
      domain: "saga-loyalty-earn",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })))
  end
end
