defmodule Customer.Application do
  use Application
  require Logger

  @impl true
  def start(_type, _args) do
    port = System.get_env("PORT", "50900") |> String.to_integer()

    children = [
      {GRPC.Server.Supervisor, endpoint: Customer.Endpoint, port: port, start_server: true}
    ]

    Logger.info(Jason.encode!(%{
      level: "info",
      message: "business_logic_server_started",
      domain: "customer",
      port: port,
      timestamp: DateTime.utc_now() |> DateTime.to_iso8601()
    }))

    opts = [strategy: :one_for_one, name: Customer.Supervisor]
    Supervisor.start_link(children, opts)
  end
end
