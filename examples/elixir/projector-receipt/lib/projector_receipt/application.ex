defmodule ProjectorReceipt.Application do
  use Application
  require Logger

  @impl true
  def start(_type, _args) do
    port = System.get_env("PORT", "50910") |> String.to_integer()

    children = [
      {GRPC.Server.Supervisor, endpoint: ProjectorReceipt.Endpoint, port: port, start_server: true}
    ]

    Logger.info(Jason.encode!(%{
      level: "info",
      message: "projector_server_started",
      domain: "projector-receipt",
      port: port,
      listens_to: "order domain",
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    }))

    opts = [strategy: :one_for_one, name: ProjectorReceipt.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
