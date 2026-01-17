defmodule SagaLoyaltyEarn.Application do
  use Application
  require Logger

  @impl true
  def start(_type, _args) do
    port = System.get_env("PORT", "50908") |> String.to_integer()

    children = [
      {GRPC.Server.Supervisor, endpoint: SagaLoyaltyEarn.Endpoint, port: port, start_server: true}
    ]

    Logger.info(Jason.encode!(%{
      level: "info",
      message: "saga_server_started",
      saga: "loyalty-earn",
      port: port,
      source_domain: "fulfillment",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    }))

    opts = [strategy: :one_for_one, name: SagaLoyaltyEarn.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
