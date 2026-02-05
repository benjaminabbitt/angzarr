package logic

import (
	"angzarr"
	angzarrpb "angzarr/proto/angzarr"
	"angzarr/proto/examples"

	"google.golang.org/protobuf/types/known/anypb"
)

// ============================================================================
// Named event appliers
// ============================================================================

func applyStockInitialized(state *InventoryState, event *anypb.Any) {
	var e examples.StockInitialized
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.ProductID = e.ProductId
	state.OnHand = e.Quantity
	state.Reserved = 0
	state.LowStockThreshold = e.LowStockThreshold
	state.Reservations = make(map[string]int32)
}

func applyStockReceived(state *InventoryState, event *anypb.Any) {
	var e examples.StockReceived
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.OnHand = e.NewOnHand
}

func applyStockReserved(state *InventoryState, event *anypb.Any) {
	var e examples.StockReserved
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.OnHand = e.NewOnHand
	state.Reserved = e.NewReserved
	if state.Reservations == nil {
		state.Reservations = make(map[string]int32)
	}
	state.Reservations[e.OrderId] = e.Quantity
}

func applyReservationReleased(state *InventoryState, event *anypb.Any) {
	var e examples.ReservationReleased
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.OnHand = e.NewOnHand
	state.Reserved = e.NewReserved
	delete(state.Reservations, e.OrderId)
}

func applyReservationCommitted(state *InventoryState, event *anypb.Any) {
	var e examples.ReservationCommitted
	if err := event.UnmarshalTo(&e); err != nil {
		return
	}
	state.OnHand = e.NewOnHand
	state.Reserved = e.NewReserved
	delete(state.Reservations, e.OrderId)
}

func loadInventorySnapshot(state *InventoryState, snapshot *anypb.Any) {
	var snapState examples.InventoryState
	if err := snapshot.UnmarshalTo(&snapState); err != nil {
		return
	}
	state.ProductID = snapState.ProductId
	state.OnHand = snapState.OnHand
	state.Reserved = snapState.Reserved
	state.LowStockThreshold = snapState.LowStockThreshold
	state.Reservations = make(map[string]int32)
	for k, v := range snapState.Reservations {
		state.Reservations[k] = v
	}
}

// ============================================================================
// State rebuilding
// ============================================================================

// stateBuilder is the single source of truth for event type → applier mapping.
var stateBuilder = angzarr.NewStateBuilder(func() InventoryState {
	return InventoryState{Reservations: make(map[string]int32)}
}).
	WithSnapshot(loadInventorySnapshot).
	On("StockInitialized", applyStockInitialized).
	On("StockReceived", applyStockReceived).
	On("StockReserved", applyStockReserved).
	On("ReservationReleased", applyReservationReleased).
	On("ReservationCommitted", applyReservationCommitted)

// RebuildState reconstructs inventory state from an event book.
func RebuildState(eventBook *angzarrpb.EventBook) InventoryState {
	return stateBuilder.Rebuild(eventBook)
}

// ApplyEvent applies a single event to the inventory state.
func ApplyEvent(state *InventoryState, event *anypb.Any) {
	stateBuilder.Apply(state, event)
}
