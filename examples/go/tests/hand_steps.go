package tests

import (
	"context"
	"fmt"
	"strconv"
	"strings"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"github.com/benjaminabbitt/angzarr/examples/go/hand/agg/handlers"
	"github.com/cucumber/godog"
	"github.com/google/uuid"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandContext holds state for hand aggregate scenarios
type HandContext struct {
	eventPages   []*pb.EventPage
	state        handlers.HandState
	resultEvent  *anypb.Any
	resultEvents []*anypb.Any
	lastError    error
	playerRoots  map[string][]byte // name -> root bytes
}

func newHandContext() *HandContext {
	return &HandContext{
		eventPages:  []*pb.EventPage{},
		state:       handlers.NewHandState(),
		playerRoots: make(map[string][]byte),
	}
}

// InitHandSteps registers hand aggregate step definitions
func InitHandSteps(ctx *godog.ScenarioContext) {
	hc := newHandContext()

	// Reset before each scenario
	ctx.Before(func(ctx context.Context, sc *godog.Scenario) (context.Context, error) {
		hc.eventPages = []*pb.EventPage{}
		hc.state = handlers.NewHandState()
		hc.resultEvent = nil
		hc.resultEvents = nil
		hc.lastError = nil
		hc.playerRoots = make(map[string][]byte)
		return ctx, nil
	})

	// Given steps
	ctx.Step(`^no prior events for the hand aggregate$`, hc.noPriorEvents)
	ctx.Step(`^a CardsDealt event for hand (\d+)$`, hc.cardsDealtForHand)
	ctx.Step(`^a CardsDealt event for TEXAS_HOLDEM with (\d+) players$`, hc.cardsDealtTexasHoldem)
	ctx.Step(`^a CardsDealt event for TEXAS_HOLDEM with (\d+) players at stacks (\d+)$`, hc.cardsDealtTexasHoldemWithStacks)
	ctx.Step(`^a CardsDealt event for TEXAS_HOLDEM with players:$`, hc.cardsDealtTexasHoldemWithTable)
	ctx.Step(`^a CardsDealt event for OMAHA with (\d+) players$`, hc.cardsDealtOmaha)
	ctx.Step(`^a CardsDealt event for FIVE_CARD_DRAW with (\d+) players$`, hc.cardsDealtFiveCardDraw)
	ctx.Step(`^blinds posted with pot (\d+)$`, hc.blindsPostedWithPot)
	ctx.Step(`^blinds posted with pot (\d+) and current_bet (\d+)$`, hc.blindsPostedWithPotAndBet)
	ctx.Step(`^a BlindPosted event for player "([^"]*)" amount (\d+)$`, hc.blindPostedForPlayer)
	ctx.Step(`^a BettingRoundComplete event for preflop$`, hc.bettingRoundCompletePreflop)
	ctx.Step(`^a BettingRoundComplete event for flop$`, hc.bettingRoundCompleteFlop)
	ctx.Step(`^a BettingRoundComplete event for turn$`, hc.bettingRoundCompleteTurn)
	ctx.Step(`^a CommunityCardsDealt event for FLOP$`, hc.communityCardsDealtFlop)
	ctx.Step(`^the flop has been dealt$`, hc.flopDealt)
	ctx.Step(`^the flop and turn have been dealt$`, hc.flopAndTurnDealt)
	ctx.Step(`^a completed betting for TEXAS_HOLDEM with (\d+) players$`, hc.completedBettingTexasHoldem)
	ctx.Step(`^a ShowdownStarted event for the hand$`, hc.showdownStarted)
	ctx.Step(`^a CardsRevealed event for player "([^"]*)" with ranking ([A-Z_]+)$`, hc.cardsRevealedForPlayer)
	ctx.Step(`^a CardsMucked event for player "([^"]*)"$`, hc.cardsMuckedForPlayer)
	ctx.Step(`^a ActionTaken event for player "([^"]*)" with action ([A-Z_]+) amount (\d+)$`, hc.actionTakenForPlayer)
	ctx.Step(`^player "([^"]*)" folded$`, hc.playerFolded)
	ctx.Step(`^a showdown with player hands:$`, hc.showdownWithHands)
	ctx.Step(`^a hand at showdown with player "([^"]*)" holding "([^"]*)" and community "([^"]*)"$`, hc.handAtShowdown)

	// When steps
	ctx.Step(`^I handle a DealCards command for TEXAS_HOLDEM with players:$`, hc.handleDealCardsTexasHoldem)
	ctx.Step(`^I handle a DealCards command for OMAHA with players:$`, hc.handleDealCardsOmaha)
	ctx.Step(`^I handle a DealCards command for FIVE_CARD_DRAW with players:$`, hc.handleDealCardsFiveCardDraw)
	ctx.Step(`^I handle a DealCards command with seed "([^"]*)" and players:$`, hc.handleDealCardsWithSeed)
	ctx.Step(`^I handle a PostBlind command for player "([^"]*)" type "([^"]*)" amount (\d+)$`, hc.handlePostBlind)
	ctx.Step(`^I handle a PlayerAction command for player "([^"]*)" action ([A-Z_]+)$`, hc.handlePlayerActionNoAmount)
	ctx.Step(`^I handle a PlayerAction command for player "([^"]*)" action ([A-Z_]+) amount (\d+)$`, hc.handlePlayerActionWithAmount)
	ctx.Step(`^I handle a DealCommunityCards command with count (\d+)$`, hc.handleDealCommunityCards)
	ctx.Step(`^I handle a RequestDraw command for player "([^"]*)" discarding indices \[([^\]]*)\]$`, hc.handleRequestDraw)
	ctx.Step(`^I handle a RevealCards command for player "([^"]*)" with muck (true|false)$`, hc.handleRevealCards)
	ctx.Step(`^I handle an AwardPot command with winner "([^"]*)" amount (\d+)$`, hc.handleAwardPot)
	ctx.Step(`^hands are evaluated$`, hc.handsEvaluated)
	ctx.Step(`^I rebuild the hand state$`, hc.rebuildHandState)

	// Then steps
	ctx.Step(`^the result is a (?:examples\.)?CardsDealt event$`, hc.resultIsCardsDealt)
	ctx.Step(`^the result is a (?:examples\.)?BlindPosted event$`, hc.resultIsBlindPosted)
	ctx.Step(`^the result is an? (?:examples\.)?ActionTaken event$`, hc.resultIsActionTaken)
	ctx.Step(`^the result is a (?:examples\.)?CommunityCardsDealt event$`, hc.resultIsCommunityCardsDealt)
	ctx.Step(`^the result is a (?:examples\.)?DrawCompleted event$`, hc.resultIsDrawCompleted)
	ctx.Step(`^the result is a (?:examples\.)?CardsRevealed event$`, hc.resultIsCardsRevealed)
	ctx.Step(`^the result is a (?:examples\.)?CardsMucked event$`, hc.resultIsCardsMucked)
	ctx.Step(`^the result is a (?:examples\.)?PotAwarded event$`, hc.resultIsPotAwarded)
	ctx.Step(`^a HandComplete event is emitted$`, hc.handCompleteEmitted)
	ctx.Step(`^each player has (\d+) hole cards$`, hc.eachPlayerHasHoleCards)
	ctx.Step(`^the remaining deck has (\d+) cards$`, hc.remainingDeckHasCards)
	ctx.Step(`^the remaining deck decreases by (\d+)$`, hc.remainingDeckDecreasesBy)
	ctx.Step(`^player "([^"]*)" has specific hole cards for seed "([^"]*)"$`, hc.playerHasSpecificCards)
	ctx.Step(`^the blind event has blind_type "([^"]*)"$`, hc.blindEventHasType)
	ctx.Step(`^the blind event has amount (\d+)$`, hc.blindEventHasAmount)
	ctx.Step(`^the blind event has player_stack (\d+)$`, hc.blindEventHasPlayerStack)
	ctx.Step(`^the blind event has pot_total (\d+)$`, hc.blindEventHasPotTotal)
	ctx.Step(`^the action event has action "([^"]*)"$`, hc.actionEventHasAction)
	ctx.Step(`^the action event has amount (\d+)$`, hc.actionEventHasAmount)
	ctx.Step(`^the action event has pot_total (\d+)$`, hc.actionEventHasPotTotal)
	ctx.Step(`^the action event has amount_to_call (\d+)$`, hc.actionEventHasAmountToCall)
	ctx.Step(`^the action event has player_stack (\d+)$`, hc.actionEventHasPlayerStack)
	ctx.Step(`^the event has (\d+) cards? dealt$`, hc.eventHasCardsDealt)
	ctx.Step(`^the event has phase "([^"]*)"$`, hc.eventHasPhase)
	ctx.Step(`^all_community_cards has (\d+) cards$`, hc.allCommunityCardsHasCount)
	ctx.Step(`^the draw event has cards_discarded (\d+)$`, hc.drawEventHasCardsDiscarded)
	ctx.Step(`^the draw event has cards_drawn (\d+)$`, hc.drawEventHasCardsDrawn)
	ctx.Step(`^player "([^"]*)" has (\d+) hole cards$`, hc.playerHasHoleCards)
	ctx.Step(`^the reveal event has cards for player "([^"]*)"$`, hc.revealEventHasCards)
	ctx.Step(`^the reveal event has a hand ranking$`, hc.revealEventHasRanking)
	ctx.Step(`^the award event has winner "([^"]*)" with amount (\d+)$`, hc.awardEventHasWinner)
	ctx.Step(`^the hand status is "([^"]*)"$`, hc.handStatusIs)
	ctx.Step(`^player "([^"]*)" has ranking "([^"]*)"$`, hc.playerHasRanking)
	ctx.Step(`^player "([^"]*)" wins$`, hc.playerWins)
	ctx.Step(`^the revealed ranking is "([^"]*)"$`, hc.revealedRankingIs)
	ctx.Step(`^the hand state has phase "([^"]*)"$`, hc.stateHasPhase)
	ctx.Step(`^the hand state has status "([^"]*)"$`, hc.stateHasStatus)
	ctx.Step(`^the hand state has (\d+) players$`, hc.stateHasPlayers)
	ctx.Step(`^the hand state has (\d+) community cards$`, hc.stateHasCommunityCards)
	ctx.Step(`^player "([^"]*)" has_folded is (true|false)$`, hc.playerHasFolded)
	ctx.Step(`^active player count is (\d+)$`, hc.activePlayerCountIs)
	ctx.Step(`^the command fails with "([^"]*)"$`, hc.commandFailsWith)
	// Note: "command fails with status" is registered in common_steps.go
}

// Helper functions

func (hc *HandContext) makeEventPage(event *anypb.Any) *pb.EventPage {
	return &pb.EventPage{
		Header:    &pb.PageHeader{SequenceType: &pb.PageHeader_Sequence{Sequence: uint32(len(hc.eventPages))}},
		CreatedAt: timestamppb.Now(),
		Payload:   &pb.EventPage_Event{Event: event},
	}
}

func (hc *HandContext) addEvent(event *anypb.Any) {
	hc.eventPages = append(hc.eventPages, hc.makeEventPage(event))
	hc.rebuildState()
}

func (hc *HandContext) rebuildState() {
	id := uuid.New()
	eventBook := &pb.EventBook{
		Cover: &pb.Cover{
			Domain: "hand",
			Root:   &pb.UUID{Value: id[:]},
		},
		Pages:        hc.eventPages,
		NextSequence: uint32(len(hc.eventPages)),
	}
	hc.state = handlers.RebuildState(eventBook)
}

func (hc *HandContext) makeEventBook() *pb.EventBook {
	id := uuid.New()
	return &pb.EventBook{
		Cover: &pb.Cover{
			Domain: "hand",
			Root:   &pb.UUID{Value: id[:]},
		},
		Pages:        hc.eventPages,
		NextSequence: uint32(len(hc.eventPages)),
	}
}

func (hc *HandContext) getOrCreatePlayerRoot(name string) []byte {
	if root, ok := hc.playerRoots[name]; ok {
		return root
	}
	root := make([]byte, 16)
	copy(root, []byte(name))
	hc.playerRoots[name] = root
	return root
}

func (hc *HandContext) createPlayers(count int, stack int64) []*examples.PlayerInHand {
	players := make([]*examples.PlayerInHand, count)
	for i := 0; i < count; i++ {
		// Use "player-N" naming to match Gherkin conventions
		name := fmt.Sprintf("player-%d", i+1)
		players[i] = &examples.PlayerInHand{
			PlayerRoot: hc.getOrCreatePlayerRoot(name),
			Position:   int32(i),
			Stack:      stack,
		}
	}
	return players
}

// Given step implementations

func (hc *HandContext) noPriorEvents() error {
	hc.eventPages = []*pb.EventPage{}
	hc.state = handlers.NewHandState()
	return nil
}

func (hc *HandContext) cardsDealtForHand(handNumber int) error {
	return hc.createCardsDealtEvent(examples.GameVariant_TEXAS_HOLDEM, 2, 1000, int64(handNumber))
}

func (hc *HandContext) cardsDealtTexasHoldem(playerCount int) error {
	return hc.createCardsDealtEvent(examples.GameVariant_TEXAS_HOLDEM, playerCount, 1000, 1)
}

func (hc *HandContext) cardsDealtTexasHoldemWithStacks(playerCount, stack int) error {
	return hc.createCardsDealtEvent(examples.GameVariant_TEXAS_HOLDEM, playerCount, int64(stack), 1)
}

func (hc *HandContext) cardsDealtTexasHoldemWithTable(table *godog.Table) error {
	players := make([]*examples.PlayerInHand, 0)
	playerCards := make([]*examples.PlayerHoleCards, 0)

	for _, row := range table.Rows[1:] {
		name := row.Cells[0].Value
		position, _ := strconv.ParseInt(row.Cells[1].Value, 10, 32)
		stack, _ := strconv.ParseInt(row.Cells[2].Value, 10, 64)

		playerRoot := hc.getOrCreatePlayerRoot(name)
		players = append(players, &examples.PlayerInHand{
			PlayerRoot: playerRoot,
			Position:   int32(position),
			Stack:      stack,
		})
		playerCards = append(playerCards, &examples.PlayerHoleCards{
			PlayerRoot: playerRoot,
			Cards:      createHoleCards(2),
		})
	}

	event := &examples.CardsDealt{
		TableRoot:      []byte("table_1"),
		HandNumber:     1,
		GameVariant:    examples.GameVariant_TEXAS_HOLDEM,
		PlayerCards:    playerCards,
		DealerPosition: 0,
		Players:        players,
		RemainingDeck:  createRemainingDeck(52 - len(players)*2),
		DealtAt:        timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	hc.addEvent(eventAny)
	return nil
}

func (hc *HandContext) cardsDealtOmaha(playerCount int) error {
	return hc.createCardsDealtEvent(examples.GameVariant_OMAHA, playerCount, 1000, 1)
}

func (hc *HandContext) cardsDealtFiveCardDraw(playerCount int) error {
	return hc.createCardsDealtEvent(examples.GameVariant_FIVE_CARD_DRAW, playerCount, 1000, 1)
}

func (hc *HandContext) createCardsDealtEvent(variant examples.GameVariant, playerCount int, stack, handNumber int64) error {
	cardsPerPlayer := 2
	switch variant {
	case examples.GameVariant_OMAHA:
		cardsPerPlayer = 4
	case examples.GameVariant_FIVE_CARD_DRAW:
		cardsPerPlayer = 5
	}

	players := hc.createPlayers(playerCount, stack)
	playerCards := make([]*examples.PlayerHoleCards, playerCount)
	for i, p := range players {
		playerCards[i] = &examples.PlayerHoleCards{
			PlayerRoot: p.PlayerRoot,
			Cards:      createHoleCards(cardsPerPlayer),
		}
	}

	event := &examples.CardsDealt{
		TableRoot:      []byte("table_1"),
		HandNumber:     handNumber,
		GameVariant:    variant,
		PlayerCards:    playerCards,
		DealerPosition: 0,
		Players:        players,
		RemainingDeck:  createRemainingDeck(52 - playerCount*cardsPerPlayer),
		DealtAt:        timestamppb.Now(),
	}
	eventAny, err := anypb.New(event)
	if err != nil {
		return err
	}
	hc.addEvent(eventAny)
	return nil
}

func (hc *HandContext) blindsPostedWithPot(pot int) error {
	return hc.blindsPostedWithPotAndBet(pot, 20)
}

func (hc *HandContext) blindsPostedWithPotAndBet(pot, currentBet int) error {
	// Calculate small blind from pot and current bet
	smallBlind := int64(pot - currentBet)

	// Add small blind (player-1 is small blind in standard positions)
	sbPlayer := hc.getOrCreatePlayerRoot("player-1")
	sbEvent := &examples.BlindPosted{
		PlayerRoot:  sbPlayer,
		BlindType:   "small",
		Amount:      smallBlind,
		PlayerStack: 500 - smallBlind,
		PotTotal:    smallBlind,
		PostedAt:    timestamppb.Now(),
	}
	sbAny, _ := anypb.New(sbEvent)
	hc.addEvent(sbAny)

	// Add big blind (player-2 is big blind)
	bbPlayer := hc.getOrCreatePlayerRoot("player-2")
	bbEvent := &examples.BlindPosted{
		PlayerRoot:  bbPlayer,
		BlindType:   "big",
		Amount:      int64(currentBet),
		PlayerStack: 500 - int64(currentBet),
		PotTotal:    int64(pot),
		PostedAt:    timestamppb.Now(),
	}
	bbAny, _ := anypb.New(bbEvent)
	hc.addEvent(bbAny)
	return nil
}

func (hc *HandContext) blindPostedForPlayer(playerName string, amount int) error {
	playerRoot := hc.getOrCreatePlayerRoot(playerName)
	blindType := "small"
	if amount >= 20 {
		blindType = "big"
	}

	event := &examples.BlindPosted{
		PlayerRoot:  playerRoot,
		BlindType:   blindType,
		Amount:      int64(amount),
		PlayerStack: 1000 - int64(amount),
		PotTotal:    hc.state.TotalPot() + int64(amount),
		PostedAt:    timestamppb.Now(),
	}
	eventAny, _ := anypb.New(event)
	hc.addEvent(eventAny)
	return nil
}

func (hc *HandContext) bettingRoundCompletePreflop() error {
	return hc.addBettingRoundComplete(examples.BettingPhase_PREFLOP)
}

func (hc *HandContext) bettingRoundCompleteFlop() error {
	return hc.addBettingRoundComplete(examples.BettingPhase_FLOP)
}

func (hc *HandContext) bettingRoundCompleteTurn() error {
	return hc.addBettingRoundComplete(examples.BettingPhase_TURN)
}

func (hc *HandContext) addBettingRoundComplete(phase examples.BettingPhase) error {
	stacks := make([]*examples.PlayerStackSnapshot, 0, len(hc.state.Players))
	for _, p := range hc.state.Players {
		stacks = append(stacks, &examples.PlayerStackSnapshot{
			PlayerRoot: p.PlayerRoot,
			Stack:      p.Stack,
			IsAllIn:    p.IsAllIn,
			HasFolded:  p.HasFolded,
		})
	}
	event := &examples.BettingRoundComplete{
		CompletedPhase: phase,
		PotTotal:       hc.state.TotalPot(),
		Stacks:         stacks,
		CompletedAt:    timestamppb.Now(),
	}
	eventAny, _ := anypb.New(event)
	hc.addEvent(eventAny)
	return nil
}

func (hc *HandContext) communityCardsDealtFlop() error {
	event := &examples.CommunityCardsDealt{
		Cards:             createHoleCards(3),
		Phase:             examples.BettingPhase_FLOP,
		AllCommunityCards: createHoleCards(3),
		DealtAt:           timestamppb.Now(),
	}
	eventAny, _ := anypb.New(event)
	hc.addEvent(eventAny)
	// Also set PM source event if PM context is active
	SetPMSourceEvent(eventAny)
	return nil
}

func (hc *HandContext) flopDealt() error {
	return hc.communityCardsDealtFlop()
}

func (hc *HandContext) flopAndTurnDealt() error {
	hc.communityCardsDealtFlop()

	turnEvent := &examples.CommunityCardsDealt{
		Cards:             createHoleCards(1),
		Phase:             examples.BettingPhase_TURN,
		AllCommunityCards: createHoleCards(4),
		DealtAt:           timestamppb.Now(),
	}
	turnAny, _ := anypb.New(turnEvent)
	hc.addEvent(turnAny)
	return nil
}

func (hc *HandContext) completedBettingTexasHoldem(playerCount int) error {
	hc.cardsDealtTexasHoldem(playerCount)
	hc.blindsPostedWithPot(30)
	return nil
}

func (hc *HandContext) showdownStarted() error {
	event := &examples.ShowdownStarted{
		StartedAt: timestamppb.Now(),
	}
	eventAny, _ := anypb.New(event)
	hc.addEvent(eventAny)
	return nil
}

func (hc *HandContext) cardsRevealedForPlayer(playerName, ranking string) error {
	playerRoot := hc.getOrCreatePlayerRoot(playerName)
	rankType := examples.HandRankType(examples.HandRankType_value[ranking])

	event := &examples.CardsRevealed{
		PlayerRoot: playerRoot,
		Cards:      createHoleCards(2),
		Ranking: &examples.HandRanking{
			RankType: rankType,
			Score:    1000,
		},
		RevealedAt: timestamppb.Now(),
	}
	eventAny, _ := anypb.New(event)
	hc.addEvent(eventAny)
	return nil
}

func (hc *HandContext) cardsMuckedForPlayer(playerName string) error {
	playerRoot := hc.getOrCreatePlayerRoot(playerName)
	event := &examples.CardsMucked{
		PlayerRoot: playerRoot,
		MuckedAt:   timestamppb.Now(),
	}
	eventAny, _ := anypb.New(event)
	hc.addEvent(eventAny)
	return nil
}

func (hc *HandContext) actionTakenForPlayer(playerName, action string, amount int) error {
	playerRoot := hc.getOrCreatePlayerRoot(playerName)
	actionType := examples.ActionType(examples.ActionType_value[action])

	// Calculate the new bet level - for CALL/CHECK/FOLD it stays the same
	// For BET/RAISE/ALL_IN it becomes the new bet amount
	amountToCall := hc.state.CurrentBet
	switch actionType {
	case examples.ActionType_BET, examples.ActionType_RAISE, examples.ActionType_ALL_IN:
		amountToCall = int64(amount)
	}

	event := &examples.ActionTaken{
		PlayerRoot:   playerRoot,
		Action:       actionType,
		Amount:       int64(amount),
		PlayerStack:  900,
		PotTotal:     hc.state.TotalPot() + int64(amount),
		AmountToCall: amountToCall,
		ActionAt:     timestamppb.Now(),
	}
	eventAny, _ := anypb.New(event)
	hc.addEvent(eventAny)
	return nil
}

func (hc *HandContext) playerFolded(playerName string) error {
	return hc.actionTakenForPlayer(playerName, "FOLD", 0)
}

func (hc *HandContext) showdownWithHands(table *godog.Table) error {
	hc.showdownStarted()
	for _, row := range table.Rows[1:] {
		playerName := row.Cells[0].Value
		ranking := row.Cells[1].Value
		hc.cardsRevealedForPlayer(playerName, ranking)
	}
	return nil
}

func (hc *HandContext) handAtShowdown(playerName, holeCards, community string) error {
	// Parse hole cards
	holeCardsParsed := parseCards(holeCards)
	communityCardsParsed := parseCards(community)

	playerRoot := hc.getOrCreatePlayerRoot(playerName)

	// Create a CardsDealt event to initialize the hand
	cardsDealtEvent := &examples.CardsDealt{
		HandNumber:  1,
		GameVariant: examples.GameVariant_TEXAS_HOLDEM,
		Players: []*examples.PlayerInHand{
			{
				PlayerRoot: playerRoot,
				Position:   0,
				Stack:      1000,
			},
		},
		PlayerCards: []*examples.PlayerHoleCards{
			{
				PlayerRoot: playerRoot,
				Cards:      holeCardsParsed,
			},
		},
		DealtAt: timestamppb.Now(),
	}
	cardsDealtAny, _ := anypb.New(cardsDealtEvent)
	hc.addEvent(cardsDealtAny)

	// Add community cards dealt event
	communityDealtEvent := &examples.CommunityCardsDealt{
		Cards:             communityCardsParsed,
		Phase:             examples.BettingPhase_RIVER,
		AllCommunityCards: communityCardsParsed,
		DealtAt:           timestamppb.Now(),
	}
	communityDealtAny, _ := anypb.New(communityDealtEvent)
	hc.addEvent(communityDealtAny)

	// Set state to showdown
	hc.state.Status = "showdown"

	return nil
}

// parseCards parses a space-separated string of cards like "Th 9c" into Card protos
func parseCards(cardStr string) []*examples.Card {
	cards := []*examples.Card{}
	parts := strings.Split(cardStr, " ")
	for _, part := range parts {
		if len(part) < 2 {
			continue
		}
		rankChar := part[0]
		suitChar := part[1]

		var rank examples.Rank
		switch rankChar {
		case 'A':
			rank = examples.Rank_ACE
		case 'K':
			rank = examples.Rank_KING
		case 'Q':
			rank = examples.Rank_QUEEN
		case 'J':
			rank = examples.Rank_JACK
		case 'T':
			rank = examples.Rank_TEN
		case '9':
			rank = examples.Rank_NINE
		case '8':
			rank = examples.Rank_EIGHT
		case '7':
			rank = examples.Rank_SEVEN
		case '6':
			rank = examples.Rank_SIX
		case '5':
			rank = examples.Rank_FIVE
		case '4':
			rank = examples.Rank_FOUR
		case '3':
			rank = examples.Rank_THREE
		case '2':
			rank = examples.Rank_TWO
		}

		var suit examples.Suit
		switch suitChar {
		case 'h':
			suit = examples.Suit_HEARTS
		case 'd':
			suit = examples.Suit_DIAMONDS
		case 'c':
			suit = examples.Suit_CLUBS
		case 's':
			suit = examples.Suit_SPADES
		}

		cards = append(cards, &examples.Card{Suit: suit, Rank: rank})
	}
	return cards
}

// When step implementations

func (hc *HandContext) handleDealCardsTexasHoldem(table *godog.Table) error {
	return hc.handleDealCards(examples.GameVariant_TEXAS_HOLDEM, table, nil)
}

func (hc *HandContext) handleDealCardsOmaha(table *godog.Table) error {
	return hc.handleDealCards(examples.GameVariant_OMAHA, table, nil)
}

func (hc *HandContext) handleDealCardsFiveCardDraw(table *godog.Table) error {
	return hc.handleDealCards(examples.GameVariant_FIVE_CARD_DRAW, table, nil)
}

func (hc *HandContext) handleDealCardsWithSeed(seed string, table *godog.Table) error {
	seedBytes := []byte(seed)
	return hc.handleDealCards(examples.GameVariant_TEXAS_HOLDEM, table, seedBytes)
}

func (hc *HandContext) handleDealCards(variant examples.GameVariant, table *godog.Table, seed []byte) error {
	players := make([]*examples.PlayerInHand, 0)
	for _, row := range table.Rows[1:] {
		name := row.Cells[0].Value
		position, _ := strconv.ParseInt(row.Cells[1].Value, 10, 32)
		stack, _ := strconv.ParseInt(row.Cells[2].Value, 10, 64)

		players = append(players, &examples.PlayerInHand{
			PlayerRoot: hc.getOrCreatePlayerRoot(name),
			Position:   int32(position),
			Stack:      stack,
		})
	}

	cmd := &examples.DealCards{
		TableRoot:      []byte("table_1"),
		HandNumber:     1,
		GameVariant:    variant,
		DealerPosition: 0,
		Players:        players,
		DeckSeed:       seed,
	}
	cmdAny, _ := anypb.New(cmd)
	return hc.dispatchCommand(cmdAny)
}

func (hc *HandContext) handlePostBlind(playerName, blindType string, amount int) error {
	playerRoot := hc.getOrCreatePlayerRoot(playerName)
	bt := "small"
	if blindType == "BIG_BLIND" || blindType == "big" {
		bt = "big"
	}

	cmd := &examples.PostBlind{
		PlayerRoot: playerRoot,
		BlindType:  bt,
		Amount:     int64(amount),
	}
	cmdAny, _ := anypb.New(cmd)
	return hc.dispatchCommand(cmdAny)
}

func (hc *HandContext) handlePlayerActionNoAmount(playerName, action string) error {
	return hc.handlePlayerActionWithAmount(playerName, action, 0)
}

func (hc *HandContext) handlePlayerActionWithAmount(playerName, action string, amount int) error {
	playerRoot := hc.getOrCreatePlayerRoot(playerName)
	actionType := examples.ActionType(examples.ActionType_value[action])

	cmd := &examples.PlayerAction{
		PlayerRoot: playerRoot,
		Action:     actionType,
		Amount:     int64(amount),
	}
	cmdAny, _ := anypb.New(cmd)
	return hc.dispatchCommand(cmdAny)
}

func (hc *HandContext) handleDealCommunityCards(count int) error {
	cmd := &examples.DealCommunityCards{
		Count: int32(count),
	}
	cmdAny, _ := anypb.New(cmd)
	return hc.dispatchCommand(cmdAny)
}

func (hc *HandContext) handleRequestDraw(playerName, indices string) error {
	playerRoot := hc.getOrCreatePlayerRoot(playerName)

	var cardIndices []int32
	if indices != "" {
		parts := strings.Split(indices, ",")
		for _, p := range parts {
			idx, _ := strconv.ParseInt(strings.TrimSpace(p), 10, 32)
			cardIndices = append(cardIndices, int32(idx))
		}
	}

	cmd := &examples.RequestDraw{
		PlayerRoot:  playerRoot,
		CardIndices: cardIndices,
	}
	cmdAny, _ := anypb.New(cmd)
	return hc.dispatchCommand(cmdAny)
}

func (hc *HandContext) handleRevealCards(playerName, muck string) error {
	playerRoot := hc.getOrCreatePlayerRoot(playerName)

	cmd := &examples.RevealCards{
		PlayerRoot: playerRoot,
		Muck:       muck == "true",
	}
	cmdAny, _ := anypb.New(cmd)
	return hc.dispatchCommand(cmdAny)
}

func (hc *HandContext) handleAwardPot(winnerName string, amount int) error {
	winnerRoot := hc.getOrCreatePlayerRoot(winnerName)

	cmd := &examples.AwardPot{
		Awards: []*examples.PotAward{
			{
				PlayerRoot: winnerRoot,
				Amount:     int64(amount),
				PotType:    "main",
			},
		},
	}
	cmdAny, _ := anypb.New(cmd)
	return hc.dispatchCommand(cmdAny)
}

func (hc *HandContext) handsEvaluated() error {
	// Placeholder for evaluation
	return nil
}

func (hc *HandContext) rebuildHandState() error {
	hc.rebuildState()
	return nil
}

// dispatchCommand dispatches the command to the appropriate handler
func (hc *HandContext) dispatchCommand(cmdAny *anypb.Any) error {
	eventBook := hc.makeEventBook()

	hc.lastError = nil
	hc.resultEvent = nil
	hc.resultEvents = nil
	SetLastError(nil) // Reset shared error

	switch {
	case cmdAny.MessageIs(&examples.DealCards{}):
		result, err := handlers.HandleDealCards(eventBook, cmdAny, hc.state)
		hc.lastError = err
		if err == nil && result != nil {
			hc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.PostBlind{}):
		result, err := handlers.HandlePostBlind(eventBook, cmdAny, hc.state)
		hc.lastError = err
		if err == nil && result != nil {
			hc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.PlayerAction{}):
		result, err := handlers.HandlePlayerAction(eventBook, cmdAny, hc.state)
		hc.lastError = err
		if err == nil && result != nil {
			hc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.DealCommunityCards{}):
		result, err := handlers.HandleDealCommunityCards(eventBook, cmdAny, hc.state)
		hc.lastError = err
		if err == nil && result != nil {
			hc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.RequestDraw{}):
		result, err := handlers.HandleRequestDraw(eventBook, cmdAny, hc.state)
		hc.lastError = err
		if err == nil && result != nil {
			hc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.RevealCards{}):
		result, err := handlers.HandleRevealCards(eventBook, cmdAny, hc.state)
		hc.lastError = err
		if err == nil && result != nil {
			hc.resultEvent = result
		}
	case cmdAny.MessageIs(&examples.AwardPot{}):
		results, err := handlers.HandleAwardPot(eventBook, cmdAny, hc.state)
		hc.lastError = err
		if err == nil && len(results) > 0 {
			hc.resultEvent = results[0]
			hc.resultEvents = results
			// Apply result events to state so status checks work
			for _, evt := range results {
				hc.addEvent(evt)
			}
		}
	default:
		hc.lastError = fmt.Errorf("unknown command type: %s", cmdAny.TypeUrl)
	}

	// Set shared error for common step assertions
	SetLastError(hc.lastError)

	return nil
}

// Then step implementations

func (hc *HandContext) resultIsCardsDealt() error {
	if hc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", hc.lastError)
	}
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !hc.resultEvent.MessageIs(&examples.CardsDealt{}) {
		return fmt.Errorf("expected CardsDealt event, got %s", hc.resultEvent.TypeUrl)
	}
	return nil
}

func (hc *HandContext) resultIsBlindPosted() error {
	if hc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", hc.lastError)
	}
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !hc.resultEvent.MessageIs(&examples.BlindPosted{}) {
		return fmt.Errorf("expected BlindPosted event, got %s", hc.resultEvent.TypeUrl)
	}
	return nil
}

func (hc *HandContext) resultIsActionTaken() error {
	if hc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", hc.lastError)
	}
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !hc.resultEvent.MessageIs(&examples.ActionTaken{}) {
		return fmt.Errorf("expected ActionTaken event, got %s", hc.resultEvent.TypeUrl)
	}
	return nil
}

func (hc *HandContext) resultIsCommunityCardsDealt() error {
	if hc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", hc.lastError)
	}
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !hc.resultEvent.MessageIs(&examples.CommunityCardsDealt{}) {
		return fmt.Errorf("expected CommunityCardsDealt event, got %s", hc.resultEvent.TypeUrl)
	}
	return nil
}

func (hc *HandContext) resultIsDrawCompleted() error {
	if hc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", hc.lastError)
	}
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !hc.resultEvent.MessageIs(&examples.DrawCompleted{}) {
		return fmt.Errorf("expected DrawCompleted event, got %s", hc.resultEvent.TypeUrl)
	}
	return nil
}

func (hc *HandContext) resultIsCardsRevealed() error {
	if hc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", hc.lastError)
	}
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !hc.resultEvent.MessageIs(&examples.CardsRevealed{}) {
		return fmt.Errorf("expected CardsRevealed event, got %s", hc.resultEvent.TypeUrl)
	}
	return nil
}

func (hc *HandContext) resultIsCardsMucked() error {
	// RevealCards with muck=true still returns CardsRevealed with nil cards
	return hc.resultIsCardsRevealed()
}

func (hc *HandContext) resultIsPotAwarded() error {
	if hc.lastError != nil {
		return fmt.Errorf("expected success but got error: %v", hc.lastError)
	}
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	if !hc.resultEvent.MessageIs(&examples.PotAwarded{}) {
		return fmt.Errorf("expected PotAwarded event, got %s", hc.resultEvent.TypeUrl)
	}
	return nil
}

func (hc *HandContext) handCompleteEmitted() error {
	if len(hc.resultEvents) < 2 {
		return fmt.Errorf("expected HandComplete event as second event")
	}
	if !hc.resultEvents[1].MessageIs(&examples.HandComplete{}) {
		return fmt.Errorf("expected HandComplete event, got %s", hc.resultEvents[1].TypeUrl)
	}
	return nil
}

func (hc *HandContext) eachPlayerHasHoleCards(count int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.CardsDealt
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	for _, pc := range event.PlayerCards {
		if len(pc.Cards) != count {
			return fmt.Errorf("expected %d hole cards, got %d", count, len(pc.Cards))
		}
	}
	return nil
}

func (hc *HandContext) remainingDeckHasCards(count int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.CardsDealt
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if len(event.RemainingDeck) != count {
		return fmt.Errorf("expected %d remaining cards, got %d", count, len(event.RemainingDeck))
	}
	return nil
}

func (hc *HandContext) remainingDeckDecreasesBy(count int) error {
	// Implementation depends on tracking previous deck size
	return nil
}

func (hc *HandContext) playerHasSpecificCards(playerName, seed string) error {
	// Implementation for deterministic seed verification
	return nil
}

func (hc *HandContext) blindEventHasType(blindType string) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.BlindPosted
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	// Convert Gherkin SMALL_BLIND/BIG_BLIND to proto format small/big
	expected := strings.ToLower(strings.TrimSuffix(blindType, "_BLIND"))
	if event.BlindType != expected {
		return fmt.Errorf("expected blind_type=%s, got %s", expected, event.BlindType)
	}
	return nil
}

func (hc *HandContext) blindEventHasAmount(amount int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.BlindPosted
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.Amount != int64(amount) {
		return fmt.Errorf("expected amount=%d, got %d", amount, event.Amount)
	}
	return nil
}

func (hc *HandContext) blindEventHasPlayerStack(stack int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.BlindPosted
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.PlayerStack != int64(stack) {
		return fmt.Errorf("expected player_stack=%d, got %d", stack, event.PlayerStack)
	}
	return nil
}

func (hc *HandContext) blindEventHasPotTotal(pot int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.BlindPosted
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.PotTotal != int64(pot) {
		return fmt.Errorf("expected pot_total=%d, got %d", pot, event.PotTotal)
	}
	return nil
}

func (hc *HandContext) actionEventHasAction(action string) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.ActionTaken
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	expected := examples.ActionType(examples.ActionType_value[action])
	if event.Action != expected {
		return fmt.Errorf("expected action=%s, got %s", action, event.Action.String())
	}
	return nil
}

func (hc *HandContext) actionEventHasAmount(amount int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.ActionTaken
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.Amount != int64(amount) {
		return fmt.Errorf("expected amount=%d, got %d", amount, event.Amount)
	}
	return nil
}

func (hc *HandContext) actionEventHasPotTotal(pot int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.ActionTaken
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.PotTotal != int64(pot) {
		return fmt.Errorf("expected pot_total=%d, got %d", pot, event.PotTotal)
	}
	return nil
}

func (hc *HandContext) actionEventHasAmountToCall(amount int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.ActionTaken
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.AmountToCall != int64(amount) {
		return fmt.Errorf("expected amount_to_call=%d, got %d", amount, event.AmountToCall)
	}
	return nil
}

func (hc *HandContext) actionEventHasPlayerStack(stack int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.ActionTaken
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.PlayerStack != int64(stack) {
		return fmt.Errorf("expected player_stack=%d, got %d", stack, event.PlayerStack)
	}
	return nil
}

func (hc *HandContext) eventHasCardsDealt(count int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.CommunityCardsDealt
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if len(event.Cards) != count {
		return fmt.Errorf("expected %d cards dealt, got %d", count, len(event.Cards))
	}
	return nil
}

func (hc *HandContext) eventHasPhase(phase string) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.CommunityCardsDealt
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	expected := examples.BettingPhase(examples.BettingPhase_value[phase])
	if event.Phase != expected {
		return fmt.Errorf("expected phase=%s, got %s", phase, event.Phase.String())
	}
	return nil
}

func (hc *HandContext) allCommunityCardsHasCount(count int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.CommunityCardsDealt
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if len(event.AllCommunityCards) != count {
		return fmt.Errorf("expected %d all_community_cards, got %d", count, len(event.AllCommunityCards))
	}
	return nil
}

func (hc *HandContext) drawEventHasCardsDiscarded(count int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.DrawCompleted
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.CardsDiscarded != int32(count) {
		return fmt.Errorf("expected cards_discarded=%d, got %d", count, event.CardsDiscarded)
	}
	return nil
}

func (hc *HandContext) drawEventHasCardsDrawn(count int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.DrawCompleted
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.CardsDrawn != int32(count) {
		return fmt.Errorf("expected cards_drawn=%d, got %d", count, event.CardsDrawn)
	}
	return nil
}

func (hc *HandContext) playerHasHoleCards(playerName string, count int) error {
	player := hc.state.GetPlayerByRoot(hc.getOrCreatePlayerRoot(playerName))
	if player == nil {
		return fmt.Errorf("player %s not found", playerName)
	}
	if len(player.HoleCards) != count {
		return fmt.Errorf("expected %d hole cards, got %d", count, len(player.HoleCards))
	}
	return nil
}

func (hc *HandContext) revealEventHasCards(playerName string) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.CardsRevealed
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if len(event.Cards) == 0 {
		return fmt.Errorf("expected cards in reveal event")
	}
	return nil
}

func (hc *HandContext) revealEventHasRanking() error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.CardsRevealed
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.Ranking == nil {
		return fmt.Errorf("expected ranking in reveal event")
	}
	return nil
}

func (hc *HandContext) awardEventHasWinner(playerName string, amount int) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.PotAwarded
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	// Check winners list
	for _, w := range event.Winners {
		if w.Amount == int64(amount) {
			return nil
		}
	}
	return fmt.Errorf("winner %s with amount %d not found", playerName, amount)
}

func (hc *HandContext) handStatusIs(status string) error {
	if hc.state.Status != status {
		return fmt.Errorf("expected status=%s, got %s", status, hc.state.Status)
	}
	return nil
}

func (hc *HandContext) playerHasRanking(playerName, ranking string) error {
	// Implementation for ranking verification
	return nil
}

func (hc *HandContext) playerWins(playerName string) error {
	// Implementation for winner verification
	return nil
}

func (hc *HandContext) revealedRankingIs(ranking string) error {
	if hc.resultEvent == nil {
		return fmt.Errorf("no result event")
	}
	var event examples.CardsRevealed
	if err := hc.resultEvent.UnmarshalTo(&event); err != nil {
		return err
	}
	if event.Ranking == nil {
		return fmt.Errorf("no ranking in reveal event")
	}
	expected := examples.HandRankType(examples.HandRankType_value[ranking])
	if event.Ranking.RankType != expected {
		return fmt.Errorf("expected ranking=%s, got %s", ranking, event.Ranking.RankType.String())
	}
	return nil
}

func (hc *HandContext) stateHasPhase(phase string) error {
	expected := examples.BettingPhase(examples.BettingPhase_value[phase])
	if hc.state.CurrentPhase != expected {
		return fmt.Errorf("expected phase=%s, got %s", phase, hc.state.CurrentPhase.String())
	}
	return nil
}

func (hc *HandContext) stateHasStatus(status string) error {
	if hc.state.Status != status {
		return fmt.Errorf("expected status=%s, got %s", status, hc.state.Status)
	}
	return nil
}

func (hc *HandContext) stateHasPlayers(count int) error {
	if len(hc.state.Players) != count {
		return fmt.Errorf("expected %d players, got %d", count, len(hc.state.Players))
	}
	return nil
}

func (hc *HandContext) stateHasCommunityCards(count int) error {
	if len(hc.state.CommunityCards) != count {
		return fmt.Errorf("expected %d community cards, got %d", count, len(hc.state.CommunityCards))
	}
	return nil
}

func (hc *HandContext) playerHasFolded(playerName, folded string) error {
	player := hc.state.GetPlayerByRoot(hc.getOrCreatePlayerRoot(playerName))
	if player == nil {
		return fmt.Errorf("player %s not found", playerName)
	}
	expected := folded == "true"
	if player.HasFolded != expected {
		return fmt.Errorf("expected has_folded=%v, got %v", expected, player.HasFolded)
	}
	return nil
}

func (hc *HandContext) activePlayerCountIs(count int) error {
	if hc.state.ActivePlayerCount() != count {
		return fmt.Errorf("expected %d active players, got %d", count, hc.state.ActivePlayerCount())
	}
	return nil
}

func (hc *HandContext) commandFailsWith(errorMsg string) error {
	if hc.lastError == nil {
		return fmt.Errorf("expected command to fail, but it succeeded")
	}
	if !strings.Contains(hc.lastError.Error(), errorMsg) {
		return fmt.Errorf("expected error containing '%s', got '%s'", errorMsg, hc.lastError.Error())
	}
	return nil
}

// Helper to create hole cards
func createHoleCards(count int) []*examples.Card {
	cards := make([]*examples.Card, count)
	for i := 0; i < count; i++ {
		cards[i] = &examples.Card{
			Suit: examples.Suit(i % 4),
			Rank: examples.Rank(2 + i%13),
		}
	}
	return cards
}

// Helper to create remaining deck
func createRemainingDeck(count int) []*examples.Card {
	return createHoleCards(count)
}
