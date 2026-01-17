defmodule Fulfillment.Endpoint do
  use GRPC.Endpoint

  intercept GRPC.Server.Interceptors.Logger
  run Fulfillment.Server
end
