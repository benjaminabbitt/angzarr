defmodule Cart.Server do
  use GRPC.Server, service: Angzarr.BusinessLogic.Service
  require Logger

  alias Cart.Logic

  @spec handle(Angzarr.ContextualCommand.t(), GRPC.Server.Stream.t()) ::
          Angzarr.BusinessResponse.t()
  def handle(contextual_cmd, _stream) do
    cmd_book = contextual_cmd.command
    prior_events = contextual_cmd.events

    if Enum.empty?(cmd_book.pages) do
      raise GRPC.RPCError, status: :invalid_argument, message: "CommandBook has no pages"
    end

    cmd_page = hd(cmd_book.pages)

    unless cmd_page.command do
      raise GRPC.RPCError, status: :invalid_argument, message: "Command page has no command"
    end

    state = Logic.rebuild_state(prior_events)
    type_url = cmd_page.command.type_url
    cmd_data = cmd_page.command.value

    case dispatch_command(type_url, cmd_data, state) do
      {:ok, event} ->
        event_book = create_event_book(cmd_book.cover, event)
        Angzarr.BusinessResponse.new(events: event_book)

      {:error, :invalid_argument, message} ->
        raise GRPC.RPCError, status: :invalid_argument, message: message

      {:error, :failed_precondition, message} ->
        raise GRPC.RPCError, status: :failed_precondition, message: message
    end
  end

  defp dispatch_command(type_url, cmd_data, state) do
    cond do
      String.ends_with?(type_url, "CreateCart") ->
        cmd = Examples.CreateCart.decode(cmd_data)
        log_info("creating_cart", %{customer_id: cmd.customer_id})
        Logic.handle_create_cart(state, cmd.customer_id)

      String.ends_with?(type_url, "AddItem") ->
        cmd = Examples.AddItem.decode(cmd_data)
        log_info("adding_item", %{sku: cmd.sku, quantity: cmd.quantity})
        Logic.handle_add_item(state, cmd.sku, cmd.name, cmd.quantity, cmd.unit_price_cents)

      String.ends_with?(type_url, "UpdateQuantity") ->
        cmd = Examples.UpdateQuantity.decode(cmd_data)
        log_info("updating_quantity", %{sku: cmd.sku, quantity: cmd.quantity})
        Logic.handle_update_quantity(state, cmd.sku, cmd.quantity)

      String.ends_with?(type_url, "RemoveItem") ->
        cmd = Examples.RemoveItem.decode(cmd_data)
        log_info("removing_item", %{sku: cmd.sku})
        Logic.handle_remove_item(state, cmd.sku)

      String.ends_with?(type_url, "ApplyCoupon") ->
        cmd = Examples.ApplyCoupon.decode(cmd_data)
        log_info("applying_coupon", %{coupon_code: cmd.coupon_code})
        Logic.handle_apply_coupon(state, cmd.coupon_code, cmd.discount_cents)

      String.ends_with?(type_url, "ClearCart") ->
        log_info("clearing_cart", %{})
        Logic.handle_clear_cart(state)

      String.ends_with?(type_url, "Checkout") ->
        cmd = Examples.Checkout.decode(cmd_data)
        log_info("checkout_requested", %{loyalty_points_to_use: cmd.loyalty_points_to_use})
        Logic.handle_checkout(state, cmd.loyalty_points_to_use)

      true ->
        {:error, :invalid_argument, "Unknown command type: #{type_url}"}
    end
  end

  defp create_event_book(cover, event) do
    type_name = event.__struct__ |> Module.split() |> List.last()

    event_any = Google.Protobuf.Any.new(
      type_url: "type.examples/examples.#{type_name}",
      value: Protobuf.encode(event)
    )

    Angzarr.EventBook.new(
      cover: cover,
      pages: [
        Angzarr.EventPage.new(
          num: 0,
          event: event_any,
          created_at: Google.Protobuf.Timestamp.new(seconds: System.system_time(:second))
        )
      ]
    )
  end

  defp log_info(message, fields) do
    Logger.info(Jason.encode!(Map.merge(fields, %{
      level: "info",
      message: message,
      domain: "cart",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })))
  end
end
