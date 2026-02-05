package main

import (
	"log"

	"angzarr"
	"angzarr/proto/examples"

	"fulfillment/logic"
)

func main() {
	router := angzarr.NewCommandRouter("fulfillment", logic.RebuildState).
		On(angzarr.Name(&examples.CreateShipment{}), logic.HandleCreateShipment).
		On(angzarr.Name(&examples.MarkPicked{}), logic.HandleMarkPicked).
		On(angzarr.Name(&examples.MarkPacked{}), logic.HandleMarkPacked).
		On(angzarr.Name(&examples.Ship{}), logic.HandleShip).
		On(angzarr.Name(&examples.RecordDelivery{}), logic.HandleRecordDelivery)

	cfg := angzarr.ServerConfig{Domain: "fulfillment", DefaultPort: "50205"}
	if err := angzarr.RunAggregateServer(cfg, router); err != nil {
		log.Fatal(err)
	}
}
