package logic

import (
	"strings"

	angzarrpb "angzarr/proto/angzarr"
	"projector-inventory/proto/examples"

	"go.uber.org/zap"
	"google.golang.org/protobuf/types/known/anypb"
)

// Suppress unused import warning
var _ = angzarrpb.EventBook{}

const (
	ProjectorName = "inventory"
	SourceDomain  = "inventory"
)

var logger *zap.Logger

func init() {
	var err error
	logger, err = zap.NewProduction()
	if err != nil {
		panic(err)
	}
}

// Handle processes inventory events and logs them.
func Handle(book *angzarrpb.EventBook) (*angzarrpb.Projection, error) {
	for _, page := range book.Pages {
		if page.Event == nil {
			continue
		}
		processEvent(page.Event)
	}
	return &angzarrpb.Projection{}, nil
}

func processEvent(event *anypb.Any) {
	typeURL := event.TypeUrl

	if strings.HasSuffix(typeURL, "StockInitialized") {
		var e examples.StockInitialized
		if err := event.UnmarshalTo(&e); err == nil {
			logger.Info("inventory_projected",
				zap.String("event", "StockInitialized"),
				zap.String("product_id", e.ProductId),
				zap.Int32("quantity", e.Quantity),
				zap.Int32("threshold", e.LowStockThreshold),
			)
		}
	} else if strings.HasSuffix(typeURL, "StockReceived") {
		var e examples.StockReceived
		if err := event.UnmarshalTo(&e); err == nil {
			logger.Info("inventory_projected",
				zap.String("event", "StockReceived"),
				zap.Int32("quantity", e.Quantity),
				zap.Int32("new_on_hand", e.NewOnHand),
				zap.String("reference", e.Reference),
			)
		}
	} else if strings.HasSuffix(typeURL, "StockReserved") {
		var e examples.StockReserved
		if err := event.UnmarshalTo(&e); err == nil {
			logger.Info("inventory_projected",
				zap.String("event", "StockReserved"),
				zap.String("order_id", e.OrderId),
				zap.Int32("quantity", e.Quantity),
				zap.Int32("new_available", e.NewAvailable),
				zap.Int32("new_reserved", e.NewReserved),
			)
		}
	} else if strings.HasSuffix(typeURL, "ReservationReleased") {
		var e examples.ReservationReleased
		if err := event.UnmarshalTo(&e); err == nil {
			logger.Info("inventory_projected",
				zap.String("event", "ReservationReleased"),
				zap.String("order_id", e.OrderId),
				zap.Int32("quantity", e.Quantity),
				zap.Int32("new_available", e.NewAvailable),
			)
		}
	} else if strings.HasSuffix(typeURL, "ReservationCommitted") {
		var e examples.ReservationCommitted
		if err := event.UnmarshalTo(&e); err == nil {
			logger.Info("inventory_projected",
				zap.String("event", "ReservationCommitted"),
				zap.String("order_id", e.OrderId),
				zap.Int32("quantity", e.Quantity),
				zap.Int32("new_on_hand", e.NewOnHand),
			)
		}
	} else if strings.HasSuffix(typeURL, "LowStockAlert") {
		var e examples.LowStockAlert
		if err := event.UnmarshalTo(&e); err == nil {
			logger.Info("inventory_projected",
				zap.String("event", "LowStockAlert"),
				zap.String("product_id", e.ProductId),
				zap.Int32("available", e.Available),
				zap.Int32("threshold", e.Threshold),
			)
		}
	}
}
