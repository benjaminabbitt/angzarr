// Player bounded context gRPC server using functional pattern.
//
// This command handler uses the functional pattern with CommandRouter
// and standalone handler functions following guard/validate/compute.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	"github.com/benjaminabbitt/angzarr/examples/go/player/agg/handlers"
)

// docs:start:command_router
func main() {
	router := angzarr.NewCommandRouter("player", handlers.RebuildState).
		On("examples.RegisterPlayer", handlers.HandleRegisterPlayer).
		On("examples.DepositFunds", handlers.HandleDepositFunds).
		On("examples.WithdrawFunds", handlers.HandleWithdrawFunds).
		On("examples.ReserveFunds", handlers.HandleReserveFunds).
		On("examples.ReleaseFunds", handlers.HandleReleaseFunds).
		OnRejected("table", "examples.JoinTable", handlers.HandleTableJoinRejected)

	angzarr.RunCommandHandlerServer("player", "50201", router)
}

// docs:end:command_router
