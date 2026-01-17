defmodule SagaCancellation.Application do
  use Application
  require Logger

  @impl true
  def start(_type, _args) do
    port = System.get_env("PORT", "50909") |> String.to_integer()

    children = [
      {GRPC.Server.Supervisor, endpoint: SagaCancellation.Endpoint, port: port, start_server: true}
    ]

    Logger.info(Jason.encode!(%{
      level: "info",
      message: "saga_server_started",
      saga: "cancellation",
      port: port,
      source_domain: "order",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    }))

    opts = [strategy: :one_for_one, name: SagaCancellation.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
