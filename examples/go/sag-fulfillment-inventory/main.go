package main

import (
	"log"

	"angzarr"

	"sag-fulfillment-inventory/logic"
)

func main() {
	router := angzarr.NewEventRouter(logic.SagaName, logic.SourceDomain).
		Output(logic.TargetDomain).
		On("Shipped", logic.HandleShipped)

	handler := angzarr.NewSagaHandler(router)

	cfg := angzarr.ServerConfig{Domain: logic.SagaName, DefaultPort: "50211"}
	if err := angzarr.RunSagaServer(cfg, handler); err != nil {
		log.Fatal(err)
	}
}
