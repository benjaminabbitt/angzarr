// Hand bounded context gRPC server.
package main

import (
	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	"github.com/benjaminabbitt/angzarr/examples/go/agg-hand/handlers"
)

func main() {
	router := angzarr.NewCommandRouter("hand", handlers.RebuildState).
		On("DealCards", handlers.HandleDealCards).
		On("PostBlind", handlers.HandlePostBlind).
		On("PlayerAction", handlers.HandlePlayerAction).
		On("DealCommunityCards", handlers.HandleDealCommunityCards).
		On("RequestDraw", handlers.HandleRequestDraw).
		On("RevealCards", handlers.HandleRevealCards).
		On("AwardPot", handlers.HandleAwardPot)

	angzarr.RunAggregateServer("hand", "50203", router)
}
