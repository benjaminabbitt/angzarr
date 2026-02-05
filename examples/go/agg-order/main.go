package main

import (
	"log"

	"angzarr"
	"angzarr/proto/examples"

	"order/logic"
)

func main() {
	router := angzarr.NewCommandRouter("order", logic.RebuildState).
		On(angzarr.Name(&examples.CreateOrder{}), logic.HandleCreateOrder).
		On(angzarr.Name(&examples.ApplyLoyaltyDiscount{}), logic.HandleApplyLoyaltyDiscount).
		On(angzarr.Name(&examples.SubmitPayment{}), logic.HandleSubmitPayment).
		On(angzarr.Name(&examples.ConfirmPayment{}), logic.HandleConfirmPayment).
		On(angzarr.Name(&examples.CancelOrder{}), logic.HandleCancelOrder)

	cfg := angzarr.ServerConfig{Domain: "order", DefaultPort: "50203"}
	if err := angzarr.RunAggregateServer(cfg, router); err != nil {
		log.Fatal(err)
	}
}
