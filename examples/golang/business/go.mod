module github.com/benjaminabbitt/evented-rs/examples/golang/business

go 1.21

require (
	github.com/benjaminabbitt/evented-rs/examples/golang/business/proto/evented v0.0.0-00010101000000-000000000000
	google.golang.org/protobuf v1.36.5
)

replace github.com/benjaminabbitt/evented-rs/examples/golang/business/proto/evented => ./proto/evented
