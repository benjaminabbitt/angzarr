defmodule Customer.Server do
  use GRPC.Server, service: Angzarr.BusinessLogic.Service
  require Logger

  alias Customer.Logic

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
      String.ends_with?(type_url, "CreateCustomer") ->
        cmd = Examples.CreateCustomer.decode(cmd_data)
        log_info("creating_customer", %{name: cmd.name, email: cmd.email})
        Logic.handle_create_customer(state, cmd.name, cmd.email)

      String.ends_with?(type_url, "AddLoyaltyPoints") ->
        cmd = Examples.AddLoyaltyPoints.decode(cmd_data)
        log_info("adding_loyalty_points", %{points: cmd.points, reason: cmd.reason})
        Logic.handle_add_loyalty_points(state, cmd.points, cmd.reason)

      String.ends_with?(type_url, "RedeemLoyaltyPoints") ->
        cmd = Examples.RedeemLoyaltyPoints.decode(cmd_data)
        log_info("redeeming_loyalty_points", %{points: cmd.points, redemption_type: cmd.redemption_type})
        Logic.handle_redeem_loyalty_points(state, cmd.points, cmd.redemption_type)

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
      domain: "customer",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    })))
  end
end
