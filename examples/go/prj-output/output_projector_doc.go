// Output projector examples for documentation.
//
// This file contains simplified examples used in the projector documentation,
// demonstrating both OO-style and StateRouter patterns.
package main

import (
	"fmt"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
)

// docs:start:projector_oo
type OutputProjector struct {
	playerNames map[string]string
}

func (p *OutputProjector) HandlePlayerRegistered(event *examples.PlayerRegistered) {
	p.playerNames[event.PlayerId] = event.DisplayName
	fmt.Printf("[Player] %s registered\n", event.DisplayName)
}

func (p *OutputProjector) HandleFundsDeposited(event *examples.FundsDeposited) {
	name := p.playerNames[event.PlayerId]
	if name == "" {
		name = event.PlayerId
	}
	fmt.Printf("[Player] %s deposited $%.2f\n", name, float64(event.Amount.Amount)/100)
}

func (p *OutputProjector) HandleCardsDealt(event *examples.CardsDealt) {
	for _, player := range event.PlayerCards {
		name := p.playerNames[player.PlayerId]
		cards := formatCards(player.HoleCards)
		fmt.Printf("[Hand] %s dealt %s\n", name, cards)
	}
}

// docs:end:projector_oo

func formatCards(cards []*examples.Card) string {
	// Implementation omitted for brevity
	return "cards"
}

// docs:start:state_router
var playerNames = make(map[string]string)

func handlePlayerRegistered(event *examples.PlayerRegistered) {
	playerNames[event.PlayerId] = event.DisplayName
	fmt.Printf("[Player] %s registered\n", event.DisplayName)
}

func handleFundsDeposited(event *examples.FundsDeposited) {
	name := playerNames[event.PlayerId]
	if name == "" {
		name = event.PlayerId
	}
	fmt.Printf("[Player] %s deposited $%.2f\n", name, float64(event.Amount.Amount)/100)
}

func handleCardsDealt(event *examples.CardsDealt) {
	for _, player := range event.PlayerCards {
		name := playerNames[player.PlayerId]
		fmt.Printf("[Hand] %s dealt cards\n", name)
	}
}

var stateRouter = angzarr.NewStateRouter("prj-output").
	Subscribes("player", []string{"PlayerRegistered", "FundsDeposited"}).
	Subscribes("hand", []string{"CardsDealt", "ActionTaken", "PotAwarded"}).
	On("PlayerRegistered", handlePlayerRegistered).
	On("FundsDeposited", handleFundsDeposited).
	On("CardsDealt", handleCardsDealt)

// docs:end:state_router
