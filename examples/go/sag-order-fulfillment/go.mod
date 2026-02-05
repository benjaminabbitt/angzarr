module saga-fulfillment

go 1.24.0

toolchain go1.24.4

require (
	angzarr v0.0.0
	google.golang.org/protobuf v1.36.10
)

replace angzarr => ../angzarr

require (
	github.com/google/uuid v1.6.0 // indirect
	go.uber.org/multierr v1.11.0 // indirect
	go.uber.org/zap v1.27.1 // indirect
	golang.org/x/net v0.47.0 // indirect
	golang.org/x/sys v0.38.0 // indirect
	golang.org/x/text v0.31.0 // indirect
	google.golang.org/genproto/googleapis/rpc v0.0.0-20251029180050-ab9386a59fda // indirect
	google.golang.org/grpc v1.78.0 // indirect
)
