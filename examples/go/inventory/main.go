package main

import (
	"log"

	"angzarr"

	"inventory/logic"
)

func main() {
	router := angzarr.NewCommandRouter("inventory", logic.RebuildState).
		On("InitializeStock", logic.HandleInitializeStock).
		On("ReceiveStock", logic.HandleReceiveStock).
		On("ReserveStock", logic.HandleReserveStock).
		On("ReleaseReservation", logic.HandleReleaseReservation).
		On("CommitReservation", logic.HandleCommitReservation)

	cfg := angzarr.ServerConfig{Domain: "inventory", DefaultPort: "50204"}
	if err := angzarr.RunAggregateServer(cfg, router); err != nil {
		log.Fatal(err)
	}
}
