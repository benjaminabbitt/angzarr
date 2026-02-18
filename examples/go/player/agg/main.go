// Player bounded context gRPC server.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	"github.com/benjaminabbitt/angzarr/examples/go/player/agg/handlers"
)

func main() {
	// docs:start:command_router
	router := angzarr.NewCommandRouter("player", handlers.RebuildState).
		On("RegisterPlayer", handlers.HandleRegisterPlayer).
		On("DepositFunds", handlers.HandleDepositFunds).
		On("WithdrawFunds", handlers.HandleWithdrawFunds).
		On("ReserveFunds", handlers.HandleReserveFunds).
		On("ReleaseFunds", handlers.HandleReleaseFunds).
		OnRejected("table", "JoinTable", handlers.HandleTableJoinRejected)
	// docs:end:command_router

	angzarr.RunAggregateServer("player", "50201", router)
}
