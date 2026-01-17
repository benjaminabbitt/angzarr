defmodule SagaFulfillment.Application do
  use Application
  require Logger

  @impl true
  def start(_type, _args) do
    port = System.get_env("PORT", "50907") |> String.to_integer()

    children = [
      {GRPC.Server.Supervisor, endpoint: SagaFulfillment.Endpoint, port: port, start_server: true}
    ]

    Logger.info(Jason.encode!(%{
      level: "info",
      message: "saga_server_started",
      saga: "fulfillment",
      port: port,
      source_domain: "order",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    }))

    opts = [strategy: :one_for_one, name: SagaFulfillment.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
