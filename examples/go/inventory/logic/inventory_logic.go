package logic

import (
	"inventory/proto/angzarr"
	"inventory/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

type InventoryLogic interface {
	RebuildState(eventBook *angzarr.EventBook) *InventoryState
	HandleInitializeStock(state *InventoryState, productID string, quantity, lowStockThreshold int32) (*examples.StockInitialized, error)
	HandleReceiveStock(state *InventoryState, quantity int32, reference string) (*examples.StockReceived, error)
	HandleReserveStock(state *InventoryState, quantity int32, orderID string) ([]proto.Message, error)
	HandleReleaseReservation(state *InventoryState, orderID string) (*examples.ReservationReleased, error)
	HandleCommitReservation(state *InventoryState, orderID string) (*examples.ReservationCommitted, error)
}

type DefaultInventoryLogic struct{}

func NewInventoryLogic() InventoryLogic {
	return &DefaultInventoryLogic{}
}

func PackEvent(cover *angzarr.Cover, event proto.Message, seq uint32) (*angzarr.EventBook, error) {
	eventAny, err := anypb.New(event)
	if err != nil {
		return nil, err
	}

	return &angzarr.EventBook{
		Cover: cover,
		Pages: []*angzarr.EventPage{
			{
				Sequence:  &angzarr.EventPage_Num{Num: seq},
				Event:     eventAny,
				CreatedAt: timestamppb.Now(),
			},
		},
	}, nil
}

func PackEvents(cover *angzarr.Cover, events []proto.Message, startSeq uint32) (*angzarr.EventBook, error) {
	pages := make([]*angzarr.EventPage, 0, len(events))
	for i, event := range events {
		eventAny, err := anypb.New(event)
		if err != nil {
			return nil, err
		}
		pages = append(pages, &angzarr.EventPage{
			Sequence:  &angzarr.EventPage_Num{Num: startSeq + uint32(i)},
			Event:     eventAny,
			CreatedAt: timestamppb.Now(),
		})
	}

	return &angzarr.EventBook{
		Cover: cover,
		Pages: pages,
	}, nil
}
