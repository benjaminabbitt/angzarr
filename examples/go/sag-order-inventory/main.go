package main

import (
	"log"

	"angzarr"

	"saga-inventory-reservation/logic"
)

func main() {
	router := angzarr.NewEventRouter(logic.SagaName, logic.SourceDomain).
		Output(logic.TargetDomain).
		On("OrderCreated", logic.HandleOrderCreated)

	handler := angzarr.NewSagaHandler(router)

	cfg := angzarr.ServerConfig{Domain: logic.SagaName, DefaultPort: "50210"}
	if err := angzarr.RunSagaServer(cfg, handler); err != nil {
		log.Fatal(err)
	}
}
