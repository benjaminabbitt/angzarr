// Package angzarr provides event version transformation via UpcasterRouter.
package angzarr

import (
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
)

// UpcasterHandler transforms an old event Any to a new event Any.
type UpcasterHandler func(old *anypb.Any) *anypb.Any

// UpcasterRouter transforms old event versions to current versions.
//
// Events matching registered handlers are transformed.
// Events without matching handlers pass through unchanged.
//
// Example:
//
//	router := NewUpcasterRouter("order").
//	    On("OrderCreatedV1", upcastCreatedV1).
//	    On("OrderShippedV1", upcastShippedV1)
//
//	newEvents := router.Upcast(oldEvents)
type UpcasterRouter struct {
	domain   string
	handlers []upcasterEntry
}

type upcasterEntry struct {
	suffix  string
	handler UpcasterHandler
}

// NewUpcasterRouter creates a new upcaster router for a domain.
func NewUpcasterRouter(domain string) *UpcasterRouter {
	return &UpcasterRouter{
		domain:   domain,
		handlers: make([]upcasterEntry, 0),
	}
}

// On registers a handler for an old event type_url suffix.
//
// The suffix is matched against the end of the event's type_url.
// For example, suffix "OrderCreatedV1" matches "type.googleapis.com/examples.OrderCreatedV1".
func (r *UpcasterRouter) On(suffix string, handler UpcasterHandler) *UpcasterRouter {
	r.handlers = append(r.handlers, upcasterEntry{suffix: suffix, handler: handler})
	return r
}

// Upcast transforms a list of events to current versions.
//
// Events matching registered handlers are transformed.
// Events without matching handlers pass through unchanged.
func (r *UpcasterRouter) Upcast(events []*pb.EventPage) []*pb.EventPage {
	result := make([]*pb.EventPage, 0, len(events))

	for _, page := range events {
		event := page.GetEvent()
		if event == nil {
			result = append(result, page)
			continue
		}

		transformed := false
		for _, entry := range r.handlers {
			if strings.HasSuffix(event.TypeUrl, entry.suffix) {
				newEvent := entry.handler(event)
				// Clone the page and replace the event
				newPage := proto.Clone(page).(*pb.EventPage)
				newPage.Payload = &pb.EventPage_Event{Event: newEvent}
				result = append(result, newPage)
				transformed = true
				break
			}
		}

		if !transformed {
			result = append(result, page)
		}
	}

	return result
}

// Domain returns the domain this upcaster handles.
func (r *UpcasterRouter) Domain() string {
	return r.domain
}
