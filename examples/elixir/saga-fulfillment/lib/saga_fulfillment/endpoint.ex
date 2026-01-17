defmodule SagaFulfillment.Endpoint do
  use GRPC.Endpoint

  intercept GRPC.Server.Interceptors.Logger
  run SagaFulfillment.Server
end
