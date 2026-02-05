// Package angzarr provides helpers for working with protobuf type names.
package angzarr

import (
	"google.golang.org/protobuf/proto"
)

// TypeURLPrefix is the shared prefix for all proto type URLs in examples.
const TypeURLPrefix = "type.examples/examples."

// Name extracts the short type name from a proto message using reflection.
// Example: Name(&examples.CreateOrder{}) returns "CreateOrder"
func Name(msg proto.Message) string {
	return string(msg.ProtoReflect().Descriptor().Name())
}

// TypeURL builds the full type URL for a proto message.
// Example: TypeURL(&examples.CreateOrder{}) returns "type.examples/examples.CreateOrder"
func TypeURL(msg proto.Message) string {
	return TypeURLPrefix + Name(msg)
}
