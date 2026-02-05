package main

import (
	"log"

	"angzarr"

	"saga-inventory-reservation/logic"
)

func main() {
	router := angzarr.NewEventRouter(logic.SagaName, logic.SourceDomain).
		Output(logic.TargetDomain).
		On("ItemAdded", logic.HandleItemAdded).
		On("ItemRemoved", logic.HandleItemRemoved).
		On("QuantityUpdated", logic.HandleQuantityUpdated).
		On("CartCleared", logic.HandleCartCleared)

	handler := angzarr.NewSagaHandler(router)

	cfg := angzarr.ServerConfig{Domain: logic.SagaName, DefaultPort: "50210"}
	if err := angzarr.RunSagaServer(cfg, handler); err != nil {
		log.Fatal(err)
	}
}
