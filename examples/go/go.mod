module github.com/benjaminabbitt/angzarr/examples/go

go 1.23

require (
	github.com/benjaminabbitt/angzarr/client/go v0.0.0
	github.com/cucumber/godog v0.15.1
	github.com/google/uuid v1.6.0
	google.golang.org/protobuf v1.36.6
)

require (
	github.com/cucumber/gherkin/go/v26 v26.2.0 // indirect
	github.com/cucumber/messages/go/v21 v21.0.1 // indirect
	github.com/gofrs/uuid v4.3.1+incompatible // indirect
	github.com/hashicorp/go-immutable-radix v1.3.1 // indirect
	github.com/hashicorp/go-memdb v1.3.4 // indirect
	github.com/hashicorp/golang-lru v0.5.4 // indirect
	github.com/spf13/pflag v1.0.7 // indirect
	golang.org/x/net v0.22.0 // indirect
	golang.org/x/sys v0.18.0 // indirect
	golang.org/x/text v0.14.0 // indirect
	google.golang.org/genproto/googleapis/rpc v0.0.0-20240318140521-94a12d6c2237 // indirect
	google.golang.org/grpc v1.64.0 // indirect
)

replace github.com/benjaminabbitt/angzarr/client/go => ../../client/go
