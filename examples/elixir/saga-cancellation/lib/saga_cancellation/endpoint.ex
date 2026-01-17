defmodule SagaCancellation.Endpoint do
  use GRPC.Endpoint

  intercept GRPC.Server.Interceptors.Logger
  run SagaCancellation.Server
end
