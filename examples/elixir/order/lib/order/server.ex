defmodule Order.Server do
  use GRPC.Server, service: Angzarr.BusinessLogic.Service
  require Logger

  alias Order.Logic

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
      String.ends_with?(type_url, "CreateOrder") ->
        cmd = Examples.CreateOrder.decode(cmd_data)
        log_info("creating_order", %{customer_id: cmd.customer_id, item_count: length(cmd.items)})
        Logic.handle_create_order(state, cmd.customer_id, cmd.items, cmd.subtotal_cents, cmd.discount_cents)

      String.ends_with?(type_url, "ApplyLoyaltyDiscount") ->
        cmd = Examples.ApplyLoyaltyDiscount.decode(cmd_data)
        log_info("applying_loyalty_discount", %{points_used: cmd.points_used, discount_cents: cmd.discount_cents})
        Logic.handle_apply_loyalty_discount(state, cmd.points_used, cmd.discount_cents)

      String.ends_with?(type_url, "SubmitPayment") ->
        cmd = Examples.SubmitPayment.decode(cmd_data)
        log_info("submitting_payment", %{payment_method: cmd.payment_method, amount_cents: cmd.amount_cents})
        Logic.handle_submit_payment(state, cmd.payment_method, cmd.amount_cents)

      String.ends_with?(type_url, "ConfirmPayment") ->
        log_info("confirming_payment", %{})
        Logic.handle_confirm_payment(state)

      String.ends_with?(type_url, "CompleteOrder") ->
        log_info("completing_order", %{})
        Logic.handle_complete_order(state)

      String.ends_with?(type_url, "CancelOrder") ->
        cmd = Examples.CancelOrder.decode(cmd_data)
        log_info("cancelling_order", %{reason: cmd.reason})
        Logic.handle_cancel_order(state, cmd.reason, cmd.loyalty_points_used)

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
      domain: "order",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })))
  end
end
