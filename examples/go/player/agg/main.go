// Player bounded context gRPC server.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	"github.com/benjaminabbitt/angzarr/examples/go/player/agg/handlers"
)

func main() {
	router := angzarr.NewCommandRouter("player", handlers.RebuildState).
		On("RegisterPlayer", handlers.HandleRegisterPlayer).
		On("DepositFunds", handlers.HandleDepositFunds).
		On("WithdrawFunds", handlers.HandleWithdrawFunds).
		On("ReserveFunds", handlers.HandleReserveFunds).
		On("ReleaseFunds", handlers.HandleReleaseFunds).
		WithRevocationHandler(handlers.HandleRevocation)

	angzarr.RunAggregateServer("player", "50201", router)
}
