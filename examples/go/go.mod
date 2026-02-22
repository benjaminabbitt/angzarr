module github.com/benjaminabbitt/angzarr/examples/go

go 1.23.0

require (
	github.com/benjaminabbitt/angzarr/client/go v0.0.0
	google.golang.org/protobuf v1.36.6
)

require (
	github.com/google/uuid v1.6.0 // indirect
	golang.org/x/net v0.38.0 // indirect
	golang.org/x/sys v0.31.0 // indirect
	golang.org/x/text v0.23.0 // indirect
	google.golang.org/genproto/googleapis/rpc v0.0.0-20240318140521-94a12d6c2237 // indirect
	google.golang.org/grpc v1.64.0 // indirect
)

replace github.com/benjaminabbitt/angzarr/client/go => ../../client/go
