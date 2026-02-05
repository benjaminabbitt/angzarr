package main

import (
	"log"

	"angzarr"
	"angzarr/proto/examples"

	"inventory/logic"
)

func main() {
	router := angzarr.NewCommandRouter("inventory", logic.RebuildState).
		On(angzarr.Name(&examples.InitializeStock{}), logic.HandleInitializeStock).
		On(angzarr.Name(&examples.ReceiveStock{}), logic.HandleReceiveStock).
		On(angzarr.Name(&examples.ReserveStock{}), logic.HandleReserveStock).
		On(angzarr.Name(&examples.ReleaseReservation{}), logic.HandleReleaseReservation).
		On(angzarr.Name(&examples.CommitReservation{}), logic.HandleCommitReservation)

	cfg := angzarr.ServerConfig{Domain: "inventory", DefaultPort: "50204"}
	if err := angzarr.RunAggregateServer(cfg, router); err != nil {
		log.Fatal(err)
	}
}
