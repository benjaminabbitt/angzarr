defmodule ProjectorReceipt.Endpoint do
  use GRPC.Endpoint

  intercept GRPC.Server.Interceptors.Logger
  run ProjectorReceipt.Server
end
