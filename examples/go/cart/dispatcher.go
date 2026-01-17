package main

import (
	"strings"

	"go.uber.org/zap"
	"google.golang.org/grpc/codes"
	"google.golang.org/grpc/status"
	goproto "google.golang.org/protobuf/proto"

	"cart/logic"
	"cart/proto/examples"
)

func (s *server) dispatchCommand(state *logic.CartState, typeURL string, value []byte) (goproto.Message, error) {
	switch {
	case strings.HasSuffix(typeURL, "CreateCart"):
		var cmd examples.CreateCart
		if err := goproto.Unmarshal(value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("creating cart", zap.String("customer_id", cmd.CustomerId))
		event, err := s.logic.HandleCreateCart(state, cmd.CustomerId)
		if err != nil {
			return nil, mapError(err)
		}
		return event, nil

	case strings.HasSuffix(typeURL, "AddItem"):
		var cmd examples.AddItem
		if err := goproto.Unmarshal(value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("adding item", zap.String("product_id", cmd.ProductId), zap.Int32("quantity", cmd.Quantity))
		event, err := s.logic.HandleAddItem(state, cmd.ProductId, cmd.Name, cmd.Quantity, cmd.UnitPriceCents)
		if err != nil {
			return nil, mapError(err)
		}
		return event, nil

	case strings.HasSuffix(typeURL, "UpdateQuantity"):
		var cmd examples.UpdateQuantity
		if err := goproto.Unmarshal(value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("updating quantity", zap.String("product_id", cmd.ProductId), zap.Int32("new_quantity", cmd.NewQuantity))
		event, err := s.logic.HandleUpdateQuantity(state, cmd.ProductId, cmd.NewQuantity)
		if err != nil {
			return nil, mapError(err)
		}
		return event, nil

	case strings.HasSuffix(typeURL, "RemoveItem"):
		var cmd examples.RemoveItem
		if err := goproto.Unmarshal(value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("removing item", zap.String("product_id", cmd.ProductId))
		event, err := s.logic.HandleRemoveItem(state, cmd.ProductId)
		if err != nil {
			return nil, mapError(err)
		}
		return event, nil

	case strings.HasSuffix(typeURL, "ApplyCoupon"):
		var cmd examples.ApplyCoupon
		if err := goproto.Unmarshal(value, &cmd); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("applying coupon", zap.String("code", cmd.Code))
		event, err := s.logic.HandleApplyCoupon(state, cmd.Code, cmd.CouponType, cmd.Value)
		if err != nil {
			return nil, mapError(err)
		}
		return event, nil

	case strings.HasSuffix(typeURL, "ClearCart"):
		if err := goproto.Unmarshal(value, &examples.ClearCart{}); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("clearing cart")
		event, err := s.logic.HandleClearCart(state)
		if err != nil {
			return nil, mapError(err)
		}
		return event, nil

	case strings.HasSuffix(typeURL, "Checkout"):
		if err := goproto.Unmarshal(value, &examples.Checkout{}); err != nil {
			return nil, status.Errorf(codes.InvalidArgument, "failed to unmarshal command: %v", err)
		}
		logger.Info("checking out")
		event, err := s.logic.HandleCheckout(state)
		if err != nil {
			return nil, mapError(err)
		}
		return event, nil

	default:
		return nil, status.Errorf(codes.InvalidArgument, "Unknown command type: %s", typeURL)
	}
}
