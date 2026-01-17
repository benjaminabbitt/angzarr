defmodule Fulfillment.Server do
  use GRPC.Server, service: Angzarr.BusinessLogic.Service
  require Logger

  alias Fulfillment.Logic

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
      String.ends_with?(type_url, "CreateShipment") ->
        cmd = Examples.CreateShipment.decode(cmd_data)
        log_info("creating_shipment", %{order_id: cmd.order_id})
        Logic.handle_create_shipment(state, cmd.order_id, cmd.items)

      String.ends_with?(type_url, "MarkPicked") ->
        log_info("marking_picked", %{})
        Logic.handle_mark_picked(state)

      String.ends_with?(type_url, "MarkPacked") ->
        log_info("marking_packed", %{})
        Logic.handle_mark_packed(state)

      String.ends_with?(type_url, "Ship") ->
        cmd = Examples.Ship.decode(cmd_data)
        log_info("shipping", %{tracking_number: cmd.tracking_number, carrier: cmd.carrier})
        Logic.handle_ship(state, cmd.tracking_number, cmd.carrier)

      String.ends_with?(type_url, "RecordDelivery") ->
        log_info("recording_delivery", %{})
        Logic.handle_record_delivery(state)

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
      domain: "fulfillment",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })))
  end
end
