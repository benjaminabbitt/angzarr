package main

import (
	"log"

	"angzarr"

	"order/logic"
)

func main() {
	router := angzarr.NewCommandRouter("order", logic.RebuildState).
		On("CreateOrder", logic.HandleCreateOrder).
		On("ApplyLoyaltyDiscount", logic.HandleApplyLoyaltyDiscount).
		On("SubmitPayment", logic.HandleSubmitPayment).
		On("ConfirmPayment", logic.HandleConfirmPayment).
		On("CancelOrder", logic.HandleCancelOrder)

	cfg := angzarr.ServerConfig{Domain: "order", DefaultPort: "50203"}
	if err := angzarr.RunAggregateServer(cfg, router); err != nil {
		log.Fatal(err)
	}
}
