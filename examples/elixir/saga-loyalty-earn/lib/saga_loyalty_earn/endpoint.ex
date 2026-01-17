defmodule SagaLoyaltyEarn.Endpoint do
  use GRPC.Endpoint

  intercept GRPC.Server.Interceptors.Logger
  run SagaLoyaltyEarn.Server
end
