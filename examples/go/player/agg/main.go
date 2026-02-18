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
		// Rejection handling auto-delegates to framework when no specific handler registered.
		// Register specific handlers with OnRejected(domain, command, handler) if custom
		// compensation is needed.
		OnRejected("table", "JoinTable", handlers.HandleTableJoinRejected)

	angzarr.RunAggregateServer("player", "50201", router)
}
