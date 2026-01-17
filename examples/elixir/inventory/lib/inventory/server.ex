defmodule Inventory.Server do
  use GRPC.Server, service: Angzarr.BusinessLogic.Service
  require Logger

  alias Inventory.Logic

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
      {:ok, events} when is_list(events) ->
        event_book = create_event_book(cmd_book.cover, events)
        Angzarr.BusinessResponse.new(events: event_book)

      {:ok, event} ->
        event_book = create_event_book(cmd_book.cover, [event])
        Angzarr.BusinessResponse.new(events: event_book)

      {:error, :invalid_argument, message} ->
        raise GRPC.RPCError, status: :invalid_argument, message: message

      {:error, :failed_precondition, message} ->
        raise GRPC.RPCError, status: :failed_precondition, message: message
    end
  end

  defp dispatch_command(type_url, cmd_data, state) do
    cond do
      String.ends_with?(type_url, "InitializeStock") ->
        cmd = Examples.InitializeStock.decode(cmd_data)
        log_info("initializing_stock", %{sku: cmd.sku, quantity: cmd.initial_quantity})
        Logic.handle_initialize_stock(state, cmd.sku, cmd.initial_quantity, cmd.low_stock_threshold)

      String.ends_with?(type_url, "ReceiveStock") ->
        cmd = Examples.ReceiveStock.decode(cmd_data)
        log_info("receiving_stock", %{quantity: cmd.quantity, reference: cmd.reference})
        Logic.handle_receive_stock(state, cmd.quantity, cmd.reference)

      String.ends_with?(type_url, "ReserveStock") ->
        cmd = Examples.ReserveStock.decode(cmd_data)
        log_info("reserving_stock", %{order_id: cmd.order_id, quantity: cmd.quantity})
        Logic.handle_reserve_stock(state, cmd.order_id, cmd.quantity)

      String.ends_with?(type_url, "ReleaseReservation") ->
        cmd = Examples.ReleaseReservation.decode(cmd_data)
        log_info("releasing_reservation", %{order_id: cmd.order_id})
        Logic.handle_release_reservation(state, cmd.order_id)

      String.ends_with?(type_url, "CommitReservation") ->
        cmd = Examples.CommitReservation.decode(cmd_data)
        log_info("committing_reservation", %{order_id: cmd.order_id})
        Logic.handle_commit_reservation(state, cmd.order_id)

      true ->
        {:error, :invalid_argument, "Unknown command type: #{type_url}"}
    end
  end

  defp create_event_book(cover, events) do
    pages = events
    |> Enum.with_index()
    |> Enum.map(fn {event, idx} ->
      type_name = event.__struct__ |> Module.split() |> List.last()

      event_any = Google.Protobuf.Any.new(
        type_url: "type.examples/examples.#{type_name}",
        value: Protobuf.encode(event)
      )

      Angzarr.EventPage.new(
        num: idx,
        event: event_any,
        created_at: Google.Protobuf.Timestamp.new(seconds: System.system_time(:second))
      )
    end)

    Angzarr.EventBook.new(cover: cover, pages: pages)
  end

  defp log_info(message, fields) do
    Logger.info(Jason.encode!(Map.merge(fields, %{
      level: "info",
      message: message,
      domain: "inventory",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })))
  end
end
