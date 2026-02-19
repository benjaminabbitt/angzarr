package angzarr

import (
	"encoding/hex"
	"fmt"

	"github.com/google/uuid"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
)

// CoverW wraps a Cover proto with extension methods.
type CoverW struct {
	*pb.Cover
}

// NewCoverW creates a new CoverW wrapper.
func NewCoverW(proto *pb.Cover) *CoverW {
	return &CoverW{Cover: proto}
}

// Domain returns the domain, or UnknownDomain if missing.
func (w *CoverW) Domain() string {
	if w.Cover == nil || w.Cover.Domain == "" {
		return UnknownDomain
	}
	return w.Cover.Domain
}

// CorrelationID returns the correlation_id, or empty string if missing.
func (w *CoverW) CorrelationID() string {
	if w.Cover == nil {
		return ""
	}
	return w.Cover.CorrelationId
}

// HasCorrelationID returns true if the correlation_id is present and non-empty.
func (w *CoverW) HasCorrelationID() bool {
	return w.CorrelationID() != ""
}

// RootUUID extracts the root UUID.
func (w *CoverW) RootUUID() (uuid.UUID, bool) {
	if w.Cover == nil || w.Cover.Root == nil {
		return uuid.UUID{}, false
	}
	u, err := uuid.FromBytes(w.Cover.Root.Value)
	if err != nil {
		return uuid.UUID{}, false
	}
	return u, true
}

// RootIDHex returns the root UUID as a hex string, or empty string if missing.
func (w *CoverW) RootIDHex() string {
	if w.Cover == nil || w.Cover.Root == nil {
		return ""
	}
	return hex.EncodeToString(w.Cover.Root.Value)
}

// Edition returns the edition name, defaulting to DefaultEdition.
func (w *CoverW) Edition() string {
	if w.Cover == nil || w.Cover.Edition == nil || w.Cover.Edition.Name == "" {
		return DefaultEdition
	}
	return w.Cover.Edition.Name
}

// EditionOpt returns the edition name as a pointer, nil if not set.
func (w *CoverW) EditionOpt() *string {
	if w.Cover == nil || w.Cover.Edition == nil || w.Cover.Edition.Name == "" {
		return nil
	}
	return &w.Cover.Edition.Name
}

// RoutingKey computes the bus routing key.
func (w *CoverW) RoutingKey() string {
	return w.Domain()
}

// CacheKey generates a cache key based on domain + root.
func (w *CoverW) CacheKey() string {
	return fmt.Sprintf("%s:%s", w.Domain(), w.RootIDHex())
}

// EventBookW wraps an EventBook proto with extension methods.
type EventBookW struct {
	*pb.EventBook
}

// NewEventBookW creates a new EventBookW wrapper.
func NewEventBookW(proto *pb.EventBook) *EventBookW {
	return &EventBookW{EventBook: proto}
}

// NextSequence returns the next sequence number.
func (w *EventBookW) NextSequence() uint32 {
	if w.EventBook == nil {
		return 0
	}
	return w.EventBook.NextSequence
}

// Pages returns the event pages as wrapped EventPageW instances.
func (w *EventBookW) Pages() []*EventPageW {
	if w.EventBook == nil {
		return nil
	}
	result := make([]*EventPageW, len(w.EventBook.Pages))
	for i, p := range w.EventBook.Pages {
		result[i] = NewEventPageW(p)
	}
	return result
}

func (w *EventBookW) cover() *pb.Cover {
	if w.EventBook == nil {
		return nil
	}
	return w.EventBook.Cover
}

// Domain returns the domain from the cover, or UnknownDomain if missing.
func (w *EventBookW) Domain() string {
	c := w.cover()
	if c == nil || c.Domain == "" {
		return UnknownDomain
	}
	return c.Domain
}

// CorrelationID returns the correlation_id from the cover, or empty string if missing.
func (w *EventBookW) CorrelationID() string {
	c := w.cover()
	if c == nil {
		return ""
	}
	return c.CorrelationId
}

// HasCorrelationID returns true if the correlation_id is present and non-empty.
func (w *EventBookW) HasCorrelationID() bool {
	return w.CorrelationID() != ""
}

// RootUUID extracts the root UUID from the cover.
func (w *EventBookW) RootUUID() (uuid.UUID, bool) {
	c := w.cover()
	if c == nil || c.Root == nil {
		return uuid.UUID{}, false
	}
	u, err := uuid.FromBytes(c.Root.Value)
	if err != nil {
		return uuid.UUID{}, false
	}
	return u, true
}

// RootIDHex returns the root UUID as a hex string, or empty string if missing.
func (w *EventBookW) RootIDHex() string {
	c := w.cover()
	if c == nil || c.Root == nil {
		return ""
	}
	return hex.EncodeToString(c.Root.Value)
}

// Edition returns the edition name, defaulting to DefaultEdition.
func (w *EventBookW) Edition() string {
	c := w.cover()
	if c == nil || c.Edition == nil || c.Edition.Name == "" {
		return DefaultEdition
	}
	return c.Edition.Name
}

// RoutingKey computes the bus routing key.
func (w *EventBookW) RoutingKey() string {
	return w.Domain()
}

// CacheKey generates a cache key based on domain + root.
func (w *EventBookW) CacheKey() string {
	return fmt.Sprintf("%s:%s", w.Domain(), w.RootIDHex())
}

// CoverWrapper returns a CoverW wrapping the cover.
func (w *EventBookW) CoverWrapper() *CoverW {
	c := w.cover()
	if c == nil {
		return NewCoverW(&pb.Cover{})
	}
	return NewCoverW(c)
}

// CommandBookW wraps a CommandBook proto with extension methods.
type CommandBookW struct {
	*pb.CommandBook
}

// NewCommandBookW creates a new CommandBookW wrapper.
func NewCommandBookW(proto *pb.CommandBook) *CommandBookW {
	return &CommandBookW{CommandBook: proto}
}

// Pages returns the command pages as wrapped CommandPageW instances.
func (w *CommandBookW) Pages() []*CommandPageW {
	if w.CommandBook == nil {
		return nil
	}
	result := make([]*CommandPageW, len(w.CommandBook.Pages))
	for i, p := range w.CommandBook.Pages {
		result[i] = NewCommandPageW(p)
	}
	return result
}

func (w *CommandBookW) cover() *pb.Cover {
	if w.CommandBook == nil {
		return nil
	}
	return w.CommandBook.Cover
}

// Domain returns the domain from the cover, or UnknownDomain if missing.
func (w *CommandBookW) Domain() string {
	c := w.cover()
	if c == nil || c.Domain == "" {
		return UnknownDomain
	}
	return c.Domain
}

// CorrelationID returns the correlation_id from the cover, or empty string if missing.
func (w *CommandBookW) CorrelationID() string {
	c := w.cover()
	if c == nil {
		return ""
	}
	return c.CorrelationId
}

// HasCorrelationID returns true if the correlation_id is present and non-empty.
func (w *CommandBookW) HasCorrelationID() bool {
	return w.CorrelationID() != ""
}

// RootUUID extracts the root UUID from the cover.
func (w *CommandBookW) RootUUID() (uuid.UUID, bool) {
	c := w.cover()
	if c == nil || c.Root == nil {
		return uuid.UUID{}, false
	}
	u, err := uuid.FromBytes(c.Root.Value)
	if err != nil {
		return uuid.UUID{}, false
	}
	return u, true
}

// RoutingKey computes the bus routing key.
func (w *CommandBookW) RoutingKey() string {
	return w.Domain()
}

// CacheKey generates a cache key based on domain + root.
func (w *CommandBookW) CacheKey() string {
	c := w.cover()
	if c == nil || c.Root == nil {
		return fmt.Sprintf("%s:", w.Domain())
	}
	return fmt.Sprintf("%s:%s", w.Domain(), hex.EncodeToString(c.Root.Value))
}

// CoverWrapper returns a CoverW wrapping the cover.
func (w *CommandBookW) CoverWrapper() *CoverW {
	c := w.cover()
	if c == nil {
		return NewCoverW(&pb.Cover{})
	}
	return NewCoverW(c)
}

// QueryW wraps a Query proto with extension methods.
type QueryW struct {
	*pb.Query
}

// NewQueryW creates a new QueryW wrapper.
func NewQueryW(proto *pb.Query) *QueryW {
	return &QueryW{Query: proto}
}

func (w *QueryW) cover() *pb.Cover {
	if w.Query == nil {
		return nil
	}
	return w.Query.Cover
}

// Domain returns the domain from the cover, or UnknownDomain if missing.
func (w *QueryW) Domain() string {
	c := w.cover()
	if c == nil || c.Domain == "" {
		return UnknownDomain
	}
	return c.Domain
}

// CorrelationID returns the correlation_id from the cover, or empty string if missing.
func (w *QueryW) CorrelationID() string {
	c := w.cover()
	if c == nil {
		return ""
	}
	return c.CorrelationId
}

// HasCorrelationID returns true if the correlation_id is present and non-empty.
func (w *QueryW) HasCorrelationID() bool {
	return w.CorrelationID() != ""
}

// RootUUID extracts the root UUID from the cover.
func (w *QueryW) RootUUID() (uuid.UUID, bool) {
	c := w.cover()
	if c == nil || c.Root == nil {
		return uuid.UUID{}, false
	}
	u, err := uuid.FromBytes(c.Root.Value)
	if err != nil {
		return uuid.UUID{}, false
	}
	return u, true
}

// RoutingKey computes the bus routing key.
func (w *QueryW) RoutingKey() string {
	return w.Domain()
}

// CoverWrapper returns a CoverW wrapping the cover.
func (w *QueryW) CoverWrapper() *CoverW {
	c := w.cover()
	if c == nil {
		return NewCoverW(&pb.Cover{})
	}
	return NewCoverW(c)
}

// EventPageW wraps an EventPage proto with extension methods.
type EventPageW struct {
	*pb.EventPage
}

// NewEventPageW creates a new EventPageW wrapper.
func NewEventPageW(proto *pb.EventPage) *EventPageW {
	return &EventPageW{EventPage: proto}
}

// DecodeEvent attempts to decode an event payload if the type URL matches.
func (w *EventPageW) DecodeEvent(typeSuffix string, msg interface{ Unmarshal([]byte) error }) bool {
	event := w.EventPage.GetEvent()
	if w.EventPage == nil || event == nil {
		return false
	}
	if !TypeURLMatches(event.TypeUrl, typeSuffix) {
		return false
	}
	return msg.Unmarshal(event.Value) == nil
}

// CommandPageW wraps a CommandPage proto with extension methods.
type CommandPageW struct {
	*pb.CommandPage
}

// NewCommandPageW creates a new CommandPageW wrapper.
func NewCommandPageW(proto *pb.CommandPage) *CommandPageW {
	return &CommandPageW{CommandPage: proto}
}

// Sequence returns the sequence number.
func (w *CommandPageW) Sequence() uint32 {
	if w.CommandPage == nil {
		return 0
	}
	return w.CommandPage.Sequence
}

// CommandResponseW wraps a CommandResponse proto with extension methods.
type CommandResponseW struct {
	*pb.CommandResponse
}

// NewCommandResponseW creates a new CommandResponseW wrapper.
func NewCommandResponseW(proto *pb.CommandResponse) *CommandResponseW {
	return &CommandResponseW{CommandResponse: proto}
}

// EventsBook returns the events as a wrapped EventBookW, or nil if not set.
func (w *CommandResponseW) EventsBook() *EventBookW {
	if w.CommandResponse == nil || w.CommandResponse.Events == nil {
		return nil
	}
	return NewEventBookW(w.CommandResponse.Events)
}

// Events extracts the event pages as wrapped EventPageW instances.
func (w *CommandResponseW) Events() []*EventPageW {
	if w.CommandResponse == nil || w.CommandResponse.Events == nil {
		return nil
	}
	result := make([]*EventPageW, len(w.CommandResponse.Events.Pages))
	for i, p := range w.CommandResponse.Events.Pages {
		result[i] = NewEventPageW(p)
	}
	return result
}
