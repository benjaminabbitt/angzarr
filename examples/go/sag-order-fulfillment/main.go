package main

import (
	"log"

	"angzarr"

	"saga-fulfillment/logic"
)

func main() {
	router := angzarr.NewEventRouter(logic.SagaName, logic.SourceDomain).
		Output(logic.TargetDomain).
		On("OrderCompleted", logic.HandleOrderCompleted)

	handler := angzarr.NewSagaHandler(router).
		WithPrepare(logic.Prepare).
		WithExecute(logic.Execute)

	cfg := angzarr.ServerConfig{Domain: logic.SagaName, DefaultPort: "50207"}
	if err := angzarr.RunSagaServer(cfg, handler); err != nil {
		log.Fatal(err)
	}
}
