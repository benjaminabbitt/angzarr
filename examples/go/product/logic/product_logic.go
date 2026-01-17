package logic

import (
	"product/proto/angzarr"
	"product/proto/examples"

	"google.golang.org/protobuf/proto"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// ProductLogic provides business logic operations for the product domain.
type ProductLogic interface {
	// RebuildState reconstructs product state from an event book.
	RebuildState(eventBook *angzarr.EventBook) *ProductState

	// HandleCreateProduct handles the CreateProduct command.
	HandleCreateProduct(state *ProductState, sku, name, description string, priceCents int32) (*examples.ProductCreated, error)

	// HandleUpdateProduct handles the UpdateProduct command.
	HandleUpdateProduct(state *ProductState, name, description string) (*examples.ProductUpdated, error)

	// HandleSetPrice handles the SetPrice command.
	HandleSetPrice(state *ProductState, priceCents int32) (*examples.PriceSet, error)

	// HandleDiscontinue handles the Discontinue command.
	HandleDiscontinue(state *ProductState, reason string) (*examples.ProductDiscontinued, error)
}

// DefaultProductLogic is the default implementation of ProductLogic.
type DefaultProductLogic struct{}

// NewProductLogic creates a new ProductLogic instance.
func NewProductLogic() ProductLogic {
	return &DefaultProductLogic{}
}

// RebuildState reconstructs product state from events.
func (l *DefaultProductLogic) RebuildState(eventBook *angzarr.EventBook) *ProductState {
	state := EmptyState()

	if eventBook == nil || len(eventBook.Pages) == 0 {
		return state
	}

	// Start from snapshot if present
	if eventBook.Snapshot != nil && eventBook.Snapshot.State != nil {
		var snapState examples.ProductState
		if err := eventBook.Snapshot.State.UnmarshalTo(&snapState); err == nil {
			state.SKU = snapState.Sku
			state.Name = snapState.Name
			state.Description = snapState.Description
			state.PriceCents = snapState.PriceCents
			state.Status = snapState.Status
		}
	}

	// Apply events
	for _, page := range eventBook.Pages {
		if page.Event == nil {
			continue
		}

		switch {
		case page.Event.MessageIs(&examples.ProductCreated{}):
			var event examples.ProductCreated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.SKU = event.Sku
				state.Name = event.Name
				state.Description = event.Description
				state.PriceCents = event.PriceCents
				state.Status = "active"
			}

		case page.Event.MessageIs(&examples.ProductUpdated{}):
			var event examples.ProductUpdated
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.Name = event.Name
				state.Description = event.Description
			}

		case page.Event.MessageIs(&examples.PriceSet{}):
			var event examples.PriceSet
			if err := page.Event.UnmarshalTo(&event); err == nil {
				state.PriceCents = event.PriceCents
			}

		case page.Event.MessageIs(&examples.ProductDiscontinued{}):
			if err := page.Event.UnmarshalTo(&examples.ProductDiscontinued{}); err == nil {
				state.Status = "discontinued"
			}
		}
	}

	return state
}

// HandleCreateProduct validates and creates a ProductCreated event.
func (l *DefaultProductLogic) HandleCreateProduct(state *ProductState, sku, name, description string, priceCents int32) (*examples.ProductCreated, error) {
	if state.Exists() {
		return nil, NewFailedPrecondition("Product already exists")
	}

	if sku == "" {
		return nil, NewInvalidArgument("Product SKU is required")
	}
	if name == "" {
		return nil, NewInvalidArgument("Product name is required")
	}
	if priceCents < 0 {
		return nil, NewInvalidArgument("Price cannot be negative")
	}

	return &examples.ProductCreated{
		Sku:         sku,
		Name:        name,
		Description: description,
		PriceCents:  priceCents,
		CreatedAt:   timestamppb.Now(),
	}, nil
}

// HandleUpdateProduct validates and creates a ProductUpdated event.
func (l *DefaultProductLogic) HandleUpdateProduct(state *ProductState, name, description string) (*examples.ProductUpdated, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Product does not exist")
	}

	if !state.IsActive() {
		return nil, NewFailedPrecondition("Cannot update discontinued product")
	}

	if name == "" {
		return nil, NewInvalidArgument("Product name is required")
	}

	return &examples.ProductUpdated{
		Name:        name,
		Description: description,
		UpdatedAt:   timestamppb.Now(),
	}, nil
}

// HandleSetPrice validates and creates a PriceSet event.
func (l *DefaultProductLogic) HandleSetPrice(state *ProductState, priceCents int32) (*examples.PriceSet, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Product does not exist")
	}

	if !state.IsActive() {
		return nil, NewFailedPrecondition("Cannot set price on discontinued product")
	}

	if priceCents < 0 {
		return nil, NewInvalidArgument("Price cannot be negative")
	}

	return &examples.PriceSet{
		PriceCents:         priceCents,
		PreviousPriceCents: state.PriceCents,
		SetAt:              timestamppb.Now(),
	}, nil
}

// HandleDiscontinue validates and creates a ProductDiscontinued event.
func (l *DefaultProductLogic) HandleDiscontinue(state *ProductState, reason string) (*examples.ProductDiscontinued, error) {
	if !state.Exists() {
		return nil, NewFailedPrecondition("Product does not exist")
	}

	if !state.IsActive() {
		return nil, NewFailedPrecondition("Product is already discontinued")
	}

	return &examples.ProductDiscontinued{
		Reason:          reason,
		DiscontinuedAt:  timestamppb.Now(),
	}, nil
}

// PackEvent wraps an event into an EventBook.
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

// NextSequence returns the next event sequence number.
func NextSequence(priorEvents *angzarr.EventBook) uint32 {
	if priorEvents == nil || len(priorEvents.Pages) == 0 {
		return 0
	}
	return uint32(len(priorEvents.Pages))
}
