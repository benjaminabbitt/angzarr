// StateBuilder provides declarative event handler registration for state reconstruction.
//
// Replaces manual switch/case chains in RebuildState functions.
// Mirrors CommandRouter's pattern of registering handlers by type suffix.
package angzarr

import (
	"strings"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"

	angzarrpb "angzarr/proto/angzarr"
)

// StateApplier applies a raw event (Any) to state.
//
// Each handler is responsible for decoding the event and type-checking.
// This matches CommandHandler's pattern where handlers decode commands.
type StateApplier[S any] func(state *S, event *anypb.Any)

// SnapshotLoader loads state from a snapshot Any.
//
// Optional - if not set, snapshots are ignored.
// Receives the snapshot Any and a pointer to state to populate.
type SnapshotLoader[S any] func(state *S, snapshot *anypb.Any)

type applierEntry[S any] struct {
	suffix string
	apply  StateApplier[S]
}

// StateBuilder builds state from events with registered handlers.
//
// Each handler receives the raw protobuf Any and is responsible
// for decoding. This matches CommandRouter's pattern.
//
// Example:
//
//	builder := angzarr.NewStateBuilder(func() OrderState { return OrderState{} }).
//	    WithSnapshot(loadOrderSnapshot).
//	    On("OrderCreated", applyOrderCreated).
//	    On("OrderCompleted", applyOrderCompleted)
//
//	func applyOrderCreated(state *OrderState, event *anypb.Any) {
//	    var e examples.OrderCreated
//	    if err := event.UnmarshalTo(&e); err != nil { return }
//	    state.CustomerID = e.CustomerId
//	    // ...
//	}
//
//	func RebuildState(eventBook *angzarrpb.EventBook) OrderState {
//	    return builder.Rebuild(eventBook)
//	}
type StateBuilder[S any] struct {
	newState       func() S
	snapshotLoader SnapshotLoader[S]
	appliers       []applierEntry[S]
}

// NewStateBuilder creates a StateBuilder for state type S.
//
// The newState function creates a default/zero state.
func NewStateBuilder[S any](newState func() S) *StateBuilder[S] {
	return &StateBuilder[S]{
		newState: newState,
		appliers: make([]applierEntry[S], 0),
	}
}

// WithSnapshot sets a snapshot loader for restoring state from snapshots.
func (sb *StateBuilder[S]) WithSnapshot(loader SnapshotLoader[S]) *StateBuilder[S] {
	sb.snapshotLoader = loader
	return sb
}

// On registers an event applier for a type_url suffix.
//
// The applier function is responsible for decoding the event.
func (sb *StateBuilder[S]) On(typeSuffix string, apply StateApplier[S]) *StateBuilder[S] {
	sb.appliers = append(sb.appliers, applierEntry[S]{
		suffix: typeSuffix,
		apply:  apply,
	})
	return sb
}

// Apply applies a single event to state using registered handlers.
//
// Useful for applying newly-created events to current state
// without going through full EventBook reconstruction.
func (sb *StateBuilder[S]) Apply(state *S, event *anypb.Any) {
	if event == nil {
		return
	}
	for _, applier := range sb.appliers {
		if strings.HasSuffix(event.TypeUrl, applier.suffix) {
			applier.apply(state, event)
			break
		}
	}
}

// Rebuild reconstructs state from an EventBook.
//
// Handles snapshots first (if loader configured), then applies events.
// Unknown event types are silently ignored.
func (sb *StateBuilder[S]) Rebuild(eventBook *angzarrpb.EventBook) S {
	state := sb.newState()

	if eventBook == nil {
		return state
	}

	// Load snapshot if present and loader configured
	if sb.snapshotLoader != nil && eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		sb.snapshotLoader(&state, eventBook.Snapshot.State)
	}

	// Apply events
	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}
		sb.Apply(&state, page.Event)
	}

	return state
}

// RebuildFunc returns a function compatible with CommandRouter.
//
// Useful for passing to NewCommandRouter as the rebuild function.
func (sb *StateBuilder[S]) RebuildFunc() func(*angzarrpb.EventBook) S {
	return sb.Rebuild
}

// ============================================================================
// Protobuf state helpers
// ============================================================================

// LoadProtoSnapshot creates a snapshot loader for protobuf message states.
//
// Use when your Go state type can be populated directly from a proto message.
// The converter function receives the decoded proto and populates the state.
func LoadProtoSnapshot[S any, M proto.Message](msg M, converter func(*S, M)) SnapshotLoader[S] {
	return func(state *S, snapshot *anypb.Any) {
		if err := snapshot.UnmarshalTo(msg); err == nil {
			converter(state, msg)
		}
	}
}
