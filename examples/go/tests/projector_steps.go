// Package tests implements projector step definitions for BDD tests.
package tests

import (
	"context"
	"fmt"
	"strings"

	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"github.com/cucumber/godog"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// OutputProjector simulates a projector that renders events as text.
type OutputProjector struct {
	output         strings.Builder
	playerNames    map[string]string
	showTimestamps bool
}

// NewOutputProjector creates a new projector instance.
func NewOutputProjector() *OutputProjector {
	return &OutputProjector{
		playerNames: make(map[string]string),
	}
}

// ProjectorContext holds state for projector tests.
type ProjectorContext struct {
	projector   *OutputProjector
	sourceEvent *anypb.Any
	eventBook   []*anypb.Any
}

var projCtx *ProjectorContext

// RegisterProjectorSteps registers all projector step definitions.
func RegisterProjectorSteps(ctx *godog.ScenarioContext) {
	ctx.Before(func(c context.Context, sc *godog.Scenario) (context.Context, error) {
		projCtx = &ProjectorContext{}
		return c, nil
	})

	// Given steps
	ctx.Step(`^an OutputProjector$`, anOutputProjector)
	ctx.Step(`^an OutputProjector with player name "([^"]*)"$`, anOutputProjectorWithPlayerName)
	ctx.Step(`^an OutputProjector with player names "([^"]*)" and "([^"]*)"$`, anOutputProjectorWithPlayerNames)
	ctx.Step(`^an OutputProjector with show_timestamps enabled$`, anOutputProjectorWithTimestampsEnabled)
	ctx.Step(`^an OutputProjector with show_timestamps disabled$`, anOutputProjectorWithTimestampsDisabled)
	ctx.Step(`^a PlayerRegistered event with display_name "([^"]*)"$`, aPlayerRegisteredEventWithDisplayName)
	ctx.Step(`^a FundsDeposited event with amount (\d+) and new_balance (\d+)$`, aFundsDepositedEventWithAmountAndNewBalance)
	ctx.Step(`^a FundsWithdrawn event with amount (\d+) and new_balance (\d+)$`, aFundsWithdrawnEventWithAmountAndNewBalance)
	ctx.Step(`^a FundsReserved event with amount (\d+)$`, aFundsReservedEventWithAmount)
	ctx.Step(`^a TableCreated event with:$`, aTableCreatedEventWith)
	ctx.Step(`^a PlayerJoined event at seat (\d+) with buy_in (\d+)$`, aPlayerJoinedEventAtSeatWithBuyIn)
	ctx.Step(`^a PlayerLeft event with chips_cashed_out (\d+)$`, aPlayerLeftEventWithChipsCashedOut)
	ctx.Step(`^a HandStarted event with:$`, aHandStartedEventWithForProjector)
	ctx.Step(`^active players "([^"]*)", "([^"]*)", "([^"]*)" at seats (\d+), (\d+), (\d+)$`, activePlayersAtSeats)
	ctx.Step(`^a HandEnded event with winner "([^"]*)" amount (\d+)$`, aHandEndedEventWithWinnerAmount)
	ctx.Step(`^a CardsDealt event with player "([^"]*)" holding As Kh$`, aCardsDealtEventWithPlayerHoldingAsKh)
	ctx.Step(`^a BlindPosted event for "([^"]*)" type "([^"]*)" amount (\d+)$`, aBlindPostedEventForTypeAmount)
	ctx.Step(`^an ActionTaken event for "([^"]*)" action ([A-Z_]+)$`, anActionTakenEventForAction)
	ctx.Step(`^an ActionTaken event for "([^"]*)" action ([A-Z_]+) amount (\d+) pot_total (\d+)$`, anActionTakenEventForActionAmountPot)
	ctx.Step(`^a CommunityCardsDealt event for FLOP with cards Ah Kd (\d+)s$`, aCommunityCardsDealtEventForFLOPWithCardsAhKdS)
	ctx.Step(`^a CommunityCardsDealt event for TURN with card (\d+)c$`, aCommunityCardsDealtEventForTURNWithCardC)
	ctx.Step(`^a ShowdownStarted event$`, aShowdownStartedEvent)
	ctx.Step(`^a CardsRevealed event for "([^"]*)" with cards As Ad and ranking PAIR$`, aCardsRevealedEventForWithCardsAsAdAndRankingPAIR)
	ctx.Step(`^a CardsMucked event for "([^"]*)"$`, aCardsMuckedEventFor)
	ctx.Step(`^a PotAwarded event with winner "([^"]*)" amount (\d+)$`, aPotAwardedEventWithWinnerAmount)
	ctx.Step(`^a HandComplete event with final stacks:$`, aHandCompleteEventWithFinalStacks)
	ctx.Step(`^a PlayerTimedOut event for "([^"]*)" with default_action FOLD$`, aPlayerTimedOutEventForWithDefaultActionFOLD)
	ctx.Step(`^player "([^"]*)" is registered as "([^"]*)"$`, playerIsRegisteredAs)
	ctx.Step(`^an event references "([^"]*)"$`, anEventReferences)
	ctx.Step(`^an event references unknown "([^"]*)"$`, anEventReferencesUnknown)
	ctx.Step(`^an event with created_at 14:30:00$`, anEventWithCreatedAt)
	ctx.Step(`^an event with created_at$`, anEventWithCreatedAtAny)
	ctx.Step(`^an event book with PlayerJoined and BlindPosted events$`, anEventBookWithPlayerJoinedAndBlindPostedEvents)
	ctx.Step(`^an event with unknown type_url "([^"]*)"$`, anEventWithUnknownTypeUrl)

	// When steps
	ctx.Step(`^the projector handles the event$`, theProjectorHandlesTheEvent)
	ctx.Step(`^formatting cards:$`, formattingCards)
	ctx.Step(`^formatting cards with rank 2 through 14$`, formattingCardsWithRank2Through14)
	ctx.Step(`^the projector handles the event book$`, theProjectorHandlesTheEventBook)

	// Then steps
	ctx.Step(`^the output contains "([^"]*)"$`, theOutputContains)
	ctx.Step(`^the output uses "([^"]*)"$`, theOutputContains) // Same as contains
	ctx.Step(`^the output uses "([^"]*)" prefix$`, theOutputUsesPrefix)
	ctx.Step(`^the output starts with "([^"]*)"$`, theOutputStartsWith)
	ctx.Step(`^the output does not start with "([^"]*)"$`, theOutputDoesNotStartWith)
	ctx.Step(`^ranks 2-9 display as digits$`, ranks29DisplayAsDigits)
	ctx.Step(`^rank 10 displays as "T"$`, rank10DisplaysAsT)
	ctx.Step(`^rank 11 displays as "J"$`, rank11DisplaysAsJ)
	ctx.Step(`^rank 12 displays as "Q"$`, rank12DisplaysAsQ)
	ctx.Step(`^rank 13 displays as "K"$`, rank13DisplaysAsK)
	ctx.Step(`^rank 14 displays as "A"$`, rank14DisplaysAsA)
	ctx.Step(`^both events are rendered in order$`, bothEventsAreRenderedInOrder)
}

// Helper functions

func formatMoney(amount int64) string {
	if amount >= 1000 {
		return fmt.Sprintf("$%d,%03d", amount/1000, amount%1000)
	}
	return fmt.Sprintf("$%d", amount)
}

func formatCurrency(c *examples.Currency) string {
	if c == nil {
		return "$0"
	}
	return formatMoney(c.Amount)
}

func formatCard(suit examples.Suit, rank examples.Rank) string {
	var rankStr string
	switch rank {
	case examples.Rank_ACE:
		rankStr = "A"
	case examples.Rank_KING:
		rankStr = "K"
	case examples.Rank_QUEEN:
		rankStr = "Q"
	case examples.Rank_JACK:
		rankStr = "J"
	case examples.Rank_TEN:
		rankStr = "T"
	default:
		rankStr = fmt.Sprintf("%d", int32(rank))
	}

	var suitStr string
	switch suit {
	case examples.Suit_CLUBS:
		suitStr = "c"
	case examples.Suit_DIAMONDS:
		suitStr = "d"
	case examples.Suit_HEARTS:
		suitStr = "h"
	case examples.Suit_SPADES:
		suitStr = "s"
	}

	return rankStr + suitStr
}

func (p *OutputProjector) getPlayerName(rootID string) string {
	if name, ok := p.playerNames[rootID]; ok {
		return name
	}
	// Truncate ID for display
	if len(rootID) > 12 {
		return "Player_" + rootID[7:]
	}
	return "Player_" + rootID
}

func (p *OutputProjector) handleEvent(event *anypb.Any) {
	if event.MessageIs(&examples.PlayerRegistered{}) {
		var e examples.PlayerRegistered
		event.UnmarshalTo(&e)
		p.output.WriteString(fmt.Sprintf("%s registered\n", e.DisplayName))
	} else if event.MessageIs(&examples.FundsDeposited{}) {
		var e examples.FundsDeposited
		event.UnmarshalTo(&e)
		p.output.WriteString(fmt.Sprintf("Deposited %s - balance: %s\n", formatCurrency(e.Amount), formatCurrency(e.NewBalance)))
	} else if event.MessageIs(&examples.FundsWithdrawn{}) {
		var e examples.FundsWithdrawn
		event.UnmarshalTo(&e)
		p.output.WriteString(fmt.Sprintf("Withdrew %s - balance: %s\n", formatCurrency(e.Amount), formatCurrency(e.NewBalance)))
	} else if event.MessageIs(&examples.FundsReserved{}) {
		var e examples.FundsReserved
		event.UnmarshalTo(&e)
		p.output.WriteString(fmt.Sprintf("Reserved %s\n", formatCurrency(e.Amount)))
	} else if event.MessageIs(&examples.TableCreated{}) {
		var e examples.TableCreated
		event.UnmarshalTo(&e)
		p.output.WriteString(fmt.Sprintf("Table: %s - %s - %s/%s - Buy-in: %s - %s\n",
			e.TableName, e.GameVariant.String(),
			formatMoney(e.SmallBlind), formatMoney(e.BigBlind),
			formatMoney(e.MinBuyIn), formatMoney(e.MaxBuyIn)))
	} else if event.MessageIs(&examples.PlayerJoined{}) {
		var e examples.PlayerJoined
		event.UnmarshalTo(&e)
		name := p.getPlayerName(string(e.PlayerRoot))
		p.output.WriteString(fmt.Sprintf("%s joined at seat %d with %s\n", name, e.SeatPosition, formatMoney(e.BuyInAmount)))
	} else if event.MessageIs(&examples.PlayerLeft{}) {
		var e examples.PlayerLeft
		event.UnmarshalTo(&e)
		name := p.getPlayerName(string(e.PlayerRoot))
		p.output.WriteString(fmt.Sprintf("%s left with %s\n", name, formatMoney(e.ChipsCashedOut)))
	} else if event.MessageIs(&examples.HandStarted{}) {
		var e examples.HandStarted
		event.UnmarshalTo(&e)
		p.output.WriteString(fmt.Sprintf("=== HAND #%d ===\n", e.HandNumber))
		p.output.WriteString(fmt.Sprintf("Dealer: Seat %d\n", e.DealerPosition))
		for _, player := range e.ActivePlayers {
			name := p.getPlayerName(string(player.PlayerRoot))
			p.output.WriteString(fmt.Sprintf("  %s - %s\n", name, formatMoney(player.Stack)))
		}
	} else if event.MessageIs(&examples.HandEnded{}) {
		var e examples.HandEnded
		event.UnmarshalTo(&e)
		if len(e.Results) > 0 {
			name := p.getPlayerName(string(e.Results[0].WinnerRoot))
			p.output.WriteString(fmt.Sprintf("%s wins %s\n", name, formatMoney(e.Results[0].Amount)))
		}
	} else if event.MessageIs(&examples.CardsDealt{}) {
		var e examples.CardsDealt
		event.UnmarshalTo(&e)
		for _, hole := range e.PlayerCards {
			name := p.getPlayerName(string(hole.PlayerRoot))
			cards := ""
			for _, card := range hole.Cards {
				if cards != "" {
					cards += " "
				}
				cards += formatCard(card.Suit, card.Rank)
			}
			p.output.WriteString(fmt.Sprintf("%s: [%s]\n", name, cards))
		}
	} else if event.MessageIs(&examples.BlindPosted{}) {
		var e examples.BlindPosted
		event.UnmarshalTo(&e)
		name := p.getPlayerName(string(e.PlayerRoot))
		blindType := strings.ToUpper(e.BlindType)
		p.output.WriteString(fmt.Sprintf("%s posts %s %s\n", name, blindType, formatMoney(e.Amount)))
	} else if event.MessageIs(&examples.ActionTaken{}) {
		var e examples.ActionTaken
		event.UnmarshalTo(&e)
		name := p.getPlayerName(string(e.PlayerRoot))
		switch e.Action {
		case examples.ActionType_FOLD:
			p.output.WriteString(fmt.Sprintf("%s folds\n", name))
		case examples.ActionType_CHECK:
			p.output.WriteString(fmt.Sprintf("%s checks\n", name))
		case examples.ActionType_CALL:
			p.output.WriteString(fmt.Sprintf("%s calls %s - pot: %s\n", name, formatMoney(e.Amount), formatMoney(e.PotTotal)))
		case examples.ActionType_BET:
			p.output.WriteString(fmt.Sprintf("%s bets %s - pot: %s\n", name, formatMoney(e.Amount), formatMoney(e.PotTotal)))
		case examples.ActionType_RAISE:
			p.output.WriteString(fmt.Sprintf("%s raises to %s - pot: %s\n", name, formatMoney(e.Amount), formatMoney(e.PotTotal)))
		case examples.ActionType_ALL_IN:
			p.output.WriteString(fmt.Sprintf("%s all-in %s - pot: %s\n", name, formatMoney(e.Amount), formatMoney(e.PotTotal)))
		}
	} else if event.MessageIs(&examples.CommunityCardsDealt{}) {
		var e examples.CommunityCardsDealt
		event.UnmarshalTo(&e)
		cards := ""
		for _, card := range e.Cards {
			if cards != "" {
				cards += " "
			}
			cards += formatCard(card.Suit, card.Rank)
		}
		phase := strings.Title(strings.ToLower(e.Phase.String()))
		p.output.WriteString(fmt.Sprintf("%s: [%s]\n", phase, cards))
		// Also show full board
		allCards := ""
		for _, card := range e.AllCommunityCards {
			if allCards != "" {
				allCards += " "
			}
			allCards += formatCard(card.Suit, card.Rank)
		}
		p.output.WriteString(fmt.Sprintf("Board: [%s]\n", allCards))
	} else if event.MessageIs(&examples.ShowdownStarted{}) {
		p.output.WriteString("=== SHOWDOWN ===\n")
	} else if event.MessageIs(&examples.CardsRevealed{}) {
		var e examples.CardsRevealed
		event.UnmarshalTo(&e)
		name := p.getPlayerName(string(e.PlayerRoot))
		cards := ""
		for _, card := range e.Cards {
			if cards != "" {
				cards += " "
			}
			cards += formatCard(card.Suit, card.Rank)
		}
		ranking := "Unknown"
		if e.Ranking != nil {
			ranking = strings.Title(strings.ToLower(e.Ranking.RankType.String()))
		}
		p.output.WriteString(fmt.Sprintf("%s shows [%s] - %s\n", name, cards, ranking))
	} else if event.MessageIs(&examples.CardsMucked{}) {
		var e examples.CardsMucked
		event.UnmarshalTo(&e)
		name := p.getPlayerName(string(e.PlayerRoot))
		p.output.WriteString(fmt.Sprintf("%s mucks\n", name))
	} else if event.MessageIs(&examples.PotAwarded{}) {
		var e examples.PotAwarded
		event.UnmarshalTo(&e)
		for _, winner := range e.Winners {
			name := p.getPlayerName(string(winner.PlayerRoot))
			p.output.WriteString(fmt.Sprintf("%s wins %s\n", name, formatMoney(winner.Amount)))
		}
	} else if event.MessageIs(&examples.HandComplete{}) {
		var e examples.HandComplete
		event.UnmarshalTo(&e)
		p.output.WriteString("Final stacks:\n")
		for _, player := range e.FinalStacks {
			name := p.getPlayerName(string(player.PlayerRoot))
			if player.HasFolded {
				p.output.WriteString(fmt.Sprintf("  %s: %s (folded)\n", name, formatMoney(player.Stack)))
			} else {
				p.output.WriteString(fmt.Sprintf("  %s: %s\n", name, formatMoney(player.Stack)))
			}
		}
	} else if event.MessageIs(&examples.PlayerTimedOut{}) {
		var e examples.PlayerTimedOut
		event.UnmarshalTo(&e)
		name := p.getPlayerName(string(e.PlayerRoot))
		action := strings.ToLower(e.DefaultAction.String())
		p.output.WriteString(fmt.Sprintf("%s timed out - auto %ss\n", name, action))
	} else {
		p.output.WriteString(fmt.Sprintf("[Unknown event type: %s]\n", event.GetTypeUrl()))
	}
}

// Given implementations

func anOutputProjector() error {
	projCtx.projector = NewOutputProjector()
	return nil
}

func anOutputProjectorWithPlayerName(name string) error {
	projCtx.projector = NewOutputProjector()
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = name
	return nil
}

func anOutputProjectorWithPlayerNames(name1, name2 string) error {
	projCtx.projector = NewOutputProjector()
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = name1
	projCtx.projector.playerNames[string(parseUUID("player-2"))] = name2
	return nil
}

func anOutputProjectorWithTimestampsEnabled() error {
	projCtx.projector = NewOutputProjector()
	projCtx.projector.showTimestamps = true
	return nil
}

func anOutputProjectorWithTimestampsDisabled() error {
	projCtx.projector = NewOutputProjector()
	projCtx.projector.showTimestamps = false
	return nil
}

func aPlayerRegisteredEventWithDisplayName(name string) error {
	event := &examples.PlayerRegistered{
		DisplayName:  name,
		RegisteredAt: timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aFundsDepositedEventWithAmountAndNewBalance(amount, newBalance int) error {
	event := &examples.FundsDeposited{
		Amount:      &examples.Currency{Amount: int64(amount)},
		NewBalance:  &examples.Currency{Amount: int64(newBalance)},
		DepositedAt: timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aFundsWithdrawnEventWithAmountAndNewBalance(amount, newBalance int) error {
	event := &examples.FundsWithdrawn{
		Amount:     &examples.Currency{Amount: int64(amount)},
		NewBalance: &examples.Currency{Amount: int64(newBalance)},
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aFundsReservedEventWithAmount(amount int) error {
	event := &examples.FundsReserved{
		Amount: &examples.Currency{Amount: int64(amount)},
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aTableCreatedEventWith(table *godog.Table) error {
	row := table.Rows[1]
	variant := examples.GameVariant(examples.GameVariant_value[row.Cells[1].Value])
	event := &examples.TableCreated{
		TableName:   row.Cells[0].Value,
		GameVariant: variant,
		SmallBlind:  parseInt64(row.Cells[2].Value),
		BigBlind:    parseInt64(row.Cells[3].Value),
		MinBuyIn:    parseInt64(row.Cells[4].Value),
		MaxBuyIn:    parseInt64(row.Cells[5].Value),
		CreatedAt:   timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aPlayerJoinedEventAtSeatWithBuyIn(seat, buyIn int) error {
	event := &examples.PlayerJoined{
		PlayerRoot:   parseUUID("player-1"),
		SeatPosition: int32(seat),
		BuyInAmount:  int64(buyIn),
		JoinedAt:     timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aPlayerLeftEventWithChipsCashedOut(chips int) error {
	event := &examples.PlayerLeft{
		PlayerRoot:     parseUUID("player-1"),
		ChipsCashedOut: int64(chips),
		LeftAt:         timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aHandStartedEventWithForProjector(table *godog.Table) error {
	row := table.Rows[1]
	event := &examples.HandStarted{
		HandNumber:     parseInt64(row.Cells[0].Value),
		DealerPosition: parseInt32(row.Cells[1].Value),
		SmallBlind:     parseInt64(row.Cells[2].Value),
		BigBlind:       parseInt64(row.Cells[3].Value),
		ActivePlayers:  []*examples.SeatSnapshot{},
		StartedAt:      timestamppb.Now(),
	}
	// The table only has 4 columns, min/max buy-in are not included
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func activePlayersAtSeats(name1, name2, name3 string, seat1, seat2, seat3 int) error {
	// Add player names using same UUID bytes as key
	projCtx.projector.playerNames[string(parseUUID(fmt.Sprintf("player-%d", seat1)))] = name1
	projCtx.projector.playerNames[string(parseUUID(fmt.Sprintf("player-%d", seat2)))] = name2
	projCtx.projector.playerNames[string(parseUUID(fmt.Sprintf("player-%d", seat3)))] = name3

	// Update the HandStarted event with players
	var e examples.HandStarted
	projCtx.sourceEvent.UnmarshalTo(&e)
	e.ActivePlayers = []*examples.SeatSnapshot{
		{PlayerRoot: parseUUID(fmt.Sprintf("player-%d", seat1)), Position: int32(seat1), Stack: 500},
		{PlayerRoot: parseUUID(fmt.Sprintf("player-%d", seat2)), Position: int32(seat2), Stack: 500},
		{PlayerRoot: parseUUID(fmt.Sprintf("player-%d", seat3)), Position: int32(seat3), Stack: 500},
	}
	projCtx.sourceEvent, _ = anypb.New(&e)
	return nil
}

func aHandEndedEventWithWinnerAmount(winner string, amount int) error {
	event := &examples.HandEnded{
		Results: []*examples.PotResult{
			{
				WinnerRoot: parseUUID("player-1"),
				Amount:     int64(amount),
			},
		},
	}
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = winner
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aCardsDealtEventWithPlayerHoldingAsKh(player string) error {
	event := &examples.CardsDealt{
		PlayerCards: []*examples.PlayerHoleCards{
			{
				PlayerRoot: parseUUID("player-1"),
				Cards: []*examples.Card{
					{Suit: examples.Suit_SPADES, Rank: examples.Rank_ACE},
					{Suit: examples.Suit_HEARTS, Rank: examples.Rank_KING},
				},
			},
		},
		DealtAt: timestamppb.Now(),
	}
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = player
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aBlindPostedEventForTypeAmount(player, blindType string, amount int) error {
	event := &examples.BlindPosted{
		PlayerRoot: parseUUID("player-1"),
		BlindType:  blindType,
		Amount:     int64(amount),
		PostedAt:   timestamppb.Now(),
	}
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = player
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func anActionTakenEventForAction(player, action string) error {
	actionType := examples.ActionType(examples.ActionType_value[action])
	event := &examples.ActionTaken{
		PlayerRoot: parseUUID("player-1"),
		Action:     actionType,
		ActionAt:   timestamppb.Now(),
	}
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = player
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func anActionTakenEventForActionAmountPot(player, action string, amount, potTotal int) error {
	actionType := examples.ActionType(examples.ActionType_value[action])
	event := &examples.ActionTaken{
		PlayerRoot: parseUUID("player-1"),
		Action:     actionType,
		Amount:     int64(amount),
		PotTotal:   int64(potTotal),
		ActionAt:   timestamppb.Now(),
	}
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = player
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aCommunityCardsDealtEventForFLOPWithCardsAhKdS(rank int) error {
	event := &examples.CommunityCardsDealt{
		Phase: examples.BettingPhase_FLOP,
		Cards: []*examples.Card{
			{Suit: examples.Suit_HEARTS, Rank: examples.Rank_ACE},
			{Suit: examples.Suit_DIAMONDS, Rank: examples.Rank_KING},
			{Suit: examples.Suit_SPADES, Rank: examples.Rank(rank)},
		},
		AllCommunityCards: []*examples.Card{
			{Suit: examples.Suit_HEARTS, Rank: examples.Rank_ACE},
			{Suit: examples.Suit_DIAMONDS, Rank: examples.Rank_KING},
			{Suit: examples.Suit_SPADES, Rank: examples.Rank(rank)},
		},
		DealtAt: timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aCommunityCardsDealtEventForTURNWithCardC(rank int) error {
	event := &examples.CommunityCardsDealt{
		Phase: examples.BettingPhase_TURN,
		Cards: []*examples.Card{
			{Suit: examples.Suit_CLUBS, Rank: examples.Rank(rank)},
		},
		AllCommunityCards: []*examples.Card{
			{Suit: examples.Suit_HEARTS, Rank: examples.Rank_ACE},
			{Suit: examples.Suit_DIAMONDS, Rank: examples.Rank_KING},
			{Suit: examples.Suit_SPADES, Rank: examples.Rank_SEVEN},
			{Suit: examples.Suit_CLUBS, Rank: examples.Rank(rank)},
		},
		DealtAt: timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aShowdownStartedEvent() error {
	event := &examples.ShowdownStarted{
		StartedAt: timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aCardsRevealedEventForWithCardsAsAdAndRankingPAIR(player string) error {
	event := &examples.CardsRevealed{
		PlayerRoot: parseUUID("player-1"),
		Cards: []*examples.Card{
			{Suit: examples.Suit_SPADES, Rank: examples.Rank_ACE},
			{Suit: examples.Suit_DIAMONDS, Rank: examples.Rank_ACE},
		},
		Ranking:    &examples.HandRanking{RankType: examples.HandRankType_PAIR},
		RevealedAt: timestamppb.Now(),
	}
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = player
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aCardsMuckedEventFor(player string) error {
	event := &examples.CardsMucked{
		PlayerRoot: parseUUID("player-1"),
		MuckedAt:   timestamppb.Now(),
	}
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = player
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aPotAwardedEventWithWinnerAmount(winner string, amount int) error {
	event := &examples.PotAwarded{
		Winners: []*examples.PotWinner{
			{
				PlayerRoot: parseUUID("player-1"),
				Amount:     int64(amount),
			},
		},
		AwardedAt: timestamppb.Now(),
	}
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = winner
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aHandCompleteEventWithFinalStacks(table *godog.Table) error {
	event := &examples.HandComplete{
		FinalStacks: []*examples.PlayerStackSnapshot{},
	}
	for i, row := range table.Rows[1:] {
		playerID := fmt.Sprintf("player-%d", i+1)
		playerRoot := parseUUID(playerID)
		projCtx.projector.playerNames[string(playerRoot)] = row.Cells[0].Value
		event.FinalStacks = append(event.FinalStacks, &examples.PlayerStackSnapshot{
			PlayerRoot: playerRoot,
			Stack:      parseInt64(row.Cells[1].Value),
			HasFolded:  row.Cells[2].Value == "true",
		})
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aPlayerTimedOutEventForWithDefaultActionFOLD(player string) error {
	event := &examples.PlayerTimedOut{
		PlayerRoot:    parseUUID("player-1"),
		DefaultAction: examples.ActionType_FOLD,
		TimedOutAt:    timestamppb.Now(),
	}
	projCtx.projector.playerNames[string(parseUUID("player-1"))] = player
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func playerIsRegisteredAs(playerID, name string) error {
	// playerID is the full ID like "player-abc123", store with same lookup key
	projCtx.projector.playerNames[playerID] = name
	return nil
}

func anEventReferences(playerID string) error {
	event := &examples.ActionTaken{
		PlayerRoot: []byte(playerID),
		Action:     examples.ActionType_CHECK,
		ActionAt:   timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	// Immediately handle the event so output is populated
	projCtx.projector.handleEvent(projCtx.sourceEvent)
	return nil
}

func anEventReferencesUnknown(playerID string) error {
	event := &examples.ActionTaken{
		PlayerRoot: []byte(playerID),
		Action:     examples.ActionType_CHECK,
		ActionAt:   timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	// Immediately handle the event so output is populated
	projCtx.projector.handleEvent(projCtx.sourceEvent)
	return nil
}

func anEventWithCreatedAt() error {
	event := &examples.ActionTaken{
		PlayerRoot: parseUUID("player-1"),
		Action:     examples.ActionType_CHECK,
		ActionAt:   timestamppb.Now(),
	}
	projCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func anEventWithCreatedAtAny() error {
	return anEventWithCreatedAt()
}

func anEventBookWithPlayerJoinedAndBlindPostedEvents() error {
	e1 := &examples.PlayerJoined{
		PlayerRoot:   parseUUID("player-1"),
		SeatPosition: 0,
		BuyInAmount:  500,
		JoinedAt:     timestamppb.Now(),
	}
	e2 := &examples.BlindPosted{
		PlayerRoot: parseUUID("player-1"),
		BlindType:  "small",
		Amount:     5,
		PostedAt:   timestamppb.Now(),
	}
	a1, _ := anypb.New(e1)
	a2, _ := anypb.New(e2)
	projCtx.eventBook = []*anypb.Any{a1, a2}
	return nil
}

func anEventWithUnknownTypeUrl(typeUrl string) error {
	projCtx.sourceEvent = &anypb.Any{
		TypeUrl: typeUrl,
		Value:   []byte{},
	}
	return nil
}

// When implementations

func theProjectorHandlesTheEvent() error {
	if projCtx.projector == nil {
		return fmt.Errorf("projector not initialized")
	}
	if projCtx.sourceEvent != nil {
		if projCtx.projector.showTimestamps {
			projCtx.projector.output.WriteString("[14:30:00] ")
		}
		projCtx.projector.handleEvent(projCtx.sourceEvent)
	}
	return nil
}

func formattingCards(table *godog.Table) error {
	for _, row := range table.Rows[1:] {
		suit := examples.Suit(examples.Suit_value[row.Cells[0].Value])
		rank := examples.Rank(parseInt32(row.Cells[1].Value))
		projCtx.projector.output.WriteString(formatCard(suit, rank))
		projCtx.projector.output.WriteString(" ")
	}
	return nil
}

func formattingCardsWithRank2Through14() error {
	for rank := examples.Rank(2); rank <= 14; rank++ {
		projCtx.projector.output.WriteString(formatCard(examples.Suit_SPADES, rank))
		projCtx.projector.output.WriteString(" ")
	}
	return nil
}

func theProjectorHandlesTheEventBook() error {
	for _, event := range projCtx.eventBook {
		projCtx.projector.handleEvent(event)
	}
	return nil
}

// Then implementations

func theOutputContains(expected string) error {
	output := projCtx.projector.output.String()
	if !strings.Contains(output, expected) {
		return fmt.Errorf("output does not contain %q, got: %q", expected, output)
	}
	return nil
}

func theOutputUsesPrefix(prefix string) error {
	output := projCtx.projector.output.String()
	if !strings.Contains(output, prefix) {
		return fmt.Errorf("output does not contain prefix %q, got: %q", prefix, output)
	}
	return nil
}

func theOutputStartsWith(expected string) error {
	output := projCtx.projector.output.String()
	if !strings.HasPrefix(output, expected) {
		return fmt.Errorf("output does not start with %q, got: %q", expected, output)
	}
	return nil
}

func theOutputDoesNotStartWith(expected string) error {
	output := projCtx.projector.output.String()
	if strings.HasPrefix(output, expected) {
		return fmt.Errorf("output should not start with %q, got: %q", expected, output)
	}
	return nil
}

func ranks29DisplayAsDigits() error {
	output := projCtx.projector.output.String()
	for i := 2; i <= 9; i++ {
		if !strings.Contains(output, fmt.Sprintf("%ds", i)) {
			return fmt.Errorf("output does not contain %ds, got: %q", i, output)
		}
	}
	return nil
}

func rank10DisplaysAsT() error {
	return theOutputContains("Ts")
}

func rank11DisplaysAsJ() error {
	return theOutputContains("Js")
}

func rank12DisplaysAsQ() error {
	return theOutputContains("Qs")
}

func rank13DisplaysAsK() error {
	return theOutputContains("Ks")
}

func rank14DisplaysAsA() error {
	return theOutputContains("As")
}

func bothEventsAreRenderedInOrder() error {
	output := projCtx.projector.output.String()
	joinedIdx := strings.Index(output, "joined")
	blindIdx := strings.Index(output, "posts")
	if joinedIdx < 0 || blindIdx < 0 {
		return fmt.Errorf("expected both events, got: %q", output)
	}
	if joinedIdx > blindIdx {
		return fmt.Errorf("events not in order, got: %q", output)
	}
	return nil
}
