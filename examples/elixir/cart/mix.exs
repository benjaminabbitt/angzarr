defmodule Cart.MixProject do
  use Mix.Project

  def project do
    [
      app: :cart,
      version: "0.1.0",
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  def application do
    [
      extra_applications: [:logger],
      mod: {Cart.Application, []}
    ]
  end

  defp deps do
    [
      {:grpc, "~> 0.7"},
      {:protobuf, "~> 0.12"},
      {:jason, "~> 1.4"}
    ]
  end
end
