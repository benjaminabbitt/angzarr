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
	google.golang.org/genproto/googleapis/rpc v0.0.0-20241015192408-796eee8c2d53 // indirect
	google.golang.org/grpc v1.69.4 // indirect
)

replace github.com/benjaminabbitt/angzarr/client/go => ../../client/go
