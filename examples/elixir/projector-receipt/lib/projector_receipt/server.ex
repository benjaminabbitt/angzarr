defmodule ProjectorReceipt.Server do
  use GRPC.Server, service: Angzarr.ProjectorCoordinator.Service
  require Logger

  @points_per_dollar 10

  def handle_sync(event_book, _stream) do
    case project(event_book) do
      nil ->
        nil

      receipt_data ->
        order_id = extract_order_id(event_book)
        formatted_text = format_receipt(receipt_data, order_id)

        short_order_id = if String.length(order_id) > 16, do: String.slice(order_id, 0, 16), else: order_id

        log_info("generated_receipt", %{
          order_id: short_order_id,
          total_cents: receipt_data.final_total_cents,
          payment_method: receipt_data.payment_method
        })

        receipt = Examples.Receipt.new(
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

        Angzarr.Projection.new(
          data: Google.Protobuf.Any.new(
            type_url: "type.examples/examples.Receipt",
            value: Protobuf.encode(receipt)
          )
        )
    end
  end

  def handle_async(event_book, _stream) do
    log_info("async_receipt_projection", %{pages: length(event_book.pages)})
    Angzarr.ProjectorAsyncResponse.new()
  end

  defp project(event_book) do
    if is_nil(event_book) or Enum.empty?(event_book.pages) do
      nil
    else
      initial = %{
        customer_id: "",
        items: [],
        subtotal_cents: 0,
        discount_cents: 0,
        loyalty_points_used: 0,
        final_total_cents: 0,
        payment_method: "",
        completed: false
      }

      result = Enum.reduce(event_book.pages, initial, fn page, acc ->
        if page.event do
          type_url = page.event.type_url

          cond do
            String.ends_with?(type_url, "OrderCreated") ->
              event = Examples.OrderCreated.decode(page.event.value)
              %{acc |
                customer_id: event.customer_id,
                items: event.items,
                subtotal_cents: event.subtotal_cents,
                discount_cents: event.discount_cents
              }

            String.ends_with?(type_url, "LoyaltyDiscountApplied") ->
              event = Examples.LoyaltyDiscountApplied.decode(page.event.value)
              %{acc |
                loyalty_points_used: event.points_used,
                discount_cents: acc.discount_cents + event.discount_cents
              }

            String.ends_with?(type_url, "PaymentSubmitted") ->
              event = Examples.PaymentSubmitted.decode(page.event.value)
              %{acc |
                final_total_cents: event.amount_cents,
                payment_method: event.payment_method
              }

            String.ends_with?(type_url, "OrderCompleted") ->
              %{acc | completed: true}

            true ->
              acc
          end
        else
          acc
        end
      end)

      if result.completed do
        loyalty_points_earned = div(result.final_total_cents, 100) * @points_per_dollar
        Map.put(result, :loyalty_points_earned, loyalty_points_earned)
      else
        nil
      end
    end
  end

  defp format_receipt(receipt_data, order_id) do
    short_order_id = if String.length(order_id) > 16, do: String.slice(order_id, 0, 16), else: order_id
    short_customer_id = if String.length(receipt_data.customer_id) > 16,
      do: String.slice(receipt_data.customer_id, 0, 16),
      else: receipt_data.customer_id

    line = String.duplicate("=", 40)
    thin_line = String.duplicate("-", 40)

    items_text = receipt_data.items
    |> Enum.map(fn item ->
      line_total = item.quantity * item.unit_price_cents
      "#{item.quantity} x #{item.name} @ $#{format_cents(item.unit_price_cents)} = $#{format_cents(line_total)}"
    end)
    |> Enum.join("\n")

    discount_text = if receipt_data.discount_cents > 0 do
      discount_type = if receipt_data.loyalty_points_used > 0, do: "loyalty", else: "coupon"
      "\nDiscount (#{discount_type}): -$#{format_cents(receipt_data.discount_cents)}"
    else
      ""
    end

    """
    #{line}
               RECEIPT
    #{line}
    Order: #{short_order_id}...
    Customer: #{if short_customer_id == "", do: "N/A", else: "#{short_customer_id}..."}
    #{thin_line}
    #{items_text}
    #{thin_line}
    Subtotal: $#{format_cents(receipt_data.subtotal_cents)}#{discount_text}
    #{thin_line}
    TOTAL: $#{format_cents(receipt_data.final_total_cents)}
    Payment: #{receipt_data.payment_method}
    #{thin_line}
    Loyalty Points Earned: #{receipt_data.loyalty_points_earned}
    #{line}
         Thank you for your purchase!
    #{line}
    """
  end

  defp format_cents(cents) do
    :erlang.float_to_binary(cents / 100.0, decimals: 2)
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
      domain: "projector-receipt",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })))
  end
end
