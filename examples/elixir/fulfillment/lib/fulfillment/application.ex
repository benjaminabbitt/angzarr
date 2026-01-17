defmodule Fulfillment.Application do
  use Application
  require Logger

  @impl true
  def start(_type, _args) do
    port = System.get_env("PORT", "50905") |> String.to_integer()

    children = [
      {GRPC.Server.Supervisor, endpoint: Fulfillment.Endpoint, port: port, start_server: true}
    ]

    Logger.info(Jason.encode!(%{
      level: "info",
      message: "business_logic_server_started",
      domain: "fulfillment",
      port: port,
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    }))

    opts = [strategy: :one_for_one, name: Fulfillment.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
