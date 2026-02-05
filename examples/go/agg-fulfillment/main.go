package main

import (
	"log"

	"angzarr"

	"fulfillment/logic"
)

func main() {
	router := angzarr.NewCommandRouter("fulfillment", logic.RebuildState).
		On("CreateShipment", logic.HandleCreateShipment).
		On("MarkPicked", logic.HandleMarkPicked).
		On("MarkPacked", logic.HandleMarkPacked).
		On("Ship", logic.HandleShip).
		On("RecordDelivery", logic.HandleRecordDelivery)

	cfg := angzarr.ServerConfig{Domain: "fulfillment", DefaultPort: "50205"}
	if err := angzarr.RunAggregateServer(cfg, router); err != nil {
		log.Fatal(err)
	}
}
