// Table bounded context gRPC server.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	"github.com/benjaminabbitt/angzarr/examples/go/table/agg/handlers"
)

func main() {
	router := angzarr.NewCommandRouter("table", handlers.RebuildState).
		On("CreateTable", handlers.HandleCreateTable).
		On("JoinTable", handlers.HandleJoinTable).
		On("LeaveTable", handlers.HandleLeaveTable).
		On("StartHand", handlers.HandleStartHand).
		On("EndHand", handlers.HandleEndHand)

	angzarr.RunAggregateServer("table", "50202", router)
}
