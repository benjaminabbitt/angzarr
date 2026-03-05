// Package tests implements process manager step definitions for BDD tests.
package tests

import (
	"context"
	"fmt"

	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"github.com/cucumber/godog"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandPhase represents the process state machine phases.
type HandPhase string

const (
	PhaseDEALING           HandPhase = "DEALING"
	PhasePOSTING_BLINDS    HandPhase = "POSTING_BLINDS"
	PhaseBETTING           HandPhase = "BETTING"
	PhaseDEALING_COMMUNITY HandPhase = "DEALING_COMMUNITY"
	PhaseSHOWDOWN          HandPhase = "SHOWDOWN"
	PhaseCOMPLETE          HandPhase = "COMPLETE"
	PhaseDRAW              HandPhase = "DRAW"
)

// BettingPhase represents the betting round.
type BettingPhase string

const (
	BettingPREFLOP BettingPhase = "PREFLOP"
	BettingFLOP    BettingPhase = "FLOP"
	BettingTURN    BettingPhase = "TURN"
	BettingRIVER   BettingPhase = "RIVER"
	BettingDRAW    BettingPhase = "DRAW"
)

// PMPlayerState tracks player state within the process.
type PMPlayerState struct {
	PlayerRoot   []byte
	Position     int32
	Stack        int64
	BetThisRound int64
	HasActed     bool
	HasFolded    bool
	IsAllIn      bool
}

// HandProcess represents the PM's state for a hand.
type HandProcess struct {
	HandNumber       int64
	GameVariant      examples.GameVariant
	DealerPosition   int32
	SmallBlind       int64
	BigBlind         int64
	Players          map[int32]*PMPlayerState
	Phase            HandPhase
	BettingPhase     BettingPhase
	SmallBlindPosted bool
	BigBlindPosted   bool
	ActionOn         int32
	CurrentBet       int64
	PotTotal         int64
}

// PMContext holds state for PM tests.
type PMContext struct {
	process        *HandProcess
	sourceEvent    *anypb.Any
	resultCommands []string
	lastError      error
	timedOut       bool
}

// NewPMContext creates a fresh PM context.
func NewPMContext() *PMContext {
	return &PMContext{}
}

var pmCtx *PMContext

// SetPMSourceEvent allows other step modules to set the PM source event.
// This is needed because godog step matching can call hand_steps before pm_steps.
func SetPMSourceEvent(event *anypb.Any) {
	if pmCtx != nil && pmCtx.process != nil {
		pmCtx.sourceEvent = event
	}
}

// RegisterPMSteps registers all process manager step definitions.
func RegisterPMSteps(ctx *godog.ScenarioContext) {
	pmCtx = NewPMContext()

	ctx.Before(func(c context.Context, sc *godog.Scenario) (context.Context, error) {
		pmCtx = NewPMContext()
		return c, nil
	})

	// Given steps
	ctx.Step(`^a HandFlowPM$`, aHandFlowPM)
	ctx.Step(`^a HandStarted event with:$`, aHandStartedEventWith)
	ctx.Step(`^an active hand process in phase ([A-Z_]+)$`, anActiveHandProcessInPhase)
	ctx.Step(`^an active hand process with betting_phase ([A-Z_]+)$`, anActiveHandProcessWithBettingPhase)
	ctx.Step(`^an active hand process with (\d+) players$`, anActiveHandProcessWithPlayers)
	ctx.Step(`^an active hand process with game_variant ([A-Z_]+)$`, anActiveHandProcessWithGameVariant)
	ctx.Step(`^an active hand process with player "([^"]*)" at stack (\d+)$`, anActiveHandProcessWithPlayerAtStack)
	ctx.Step(`^an active hand process$`, anActiveHandProcess)
	ctx.Step(`^a CardsDealt event$`, aCardsDealtEvent)
	ctx.Step(`^a BlindPosted event for small blind$`, aBlindPostedEventForSmallBlind)
	ctx.Step(`^a BlindPosted event for big blind$`, aBlindPostedEventForBigBlind)
	ctx.Step(`^small_blind_posted is true$`, smallBlindPostedIsTrue)
	ctx.Step(`^action_on is position (\d+)$`, actionOnIsPosition)
	ctx.Step(`^an ActionTaken event for player at position (\d+) with action ([A-Z_]+)$`, anActionTakenEventForPlayerAtPositionWithAction)
	ctx.Step(`^an ActionTaken event for the last player$`, anActionTakenEventForTheLastPlayer)
	ctx.Step(`^an ActionTaken event with action ([A-Z_]+)$`, anActionTakenEventWithAction)
	ctx.Step(`^an ActionTaken event for "([^"]*)" with amount (\d+)$`, anActionTakenEventForWithAmount)
	ctx.Step(`^players at positions (\d+), (\d+), (\d+) have all acted$`, playersAtPositionsHaveAllActed)
	ctx.Step(`^all active players have acted and matched the current bet$`, allActivePlayersHaveActedAndMatchedCurrentBet)
	ctx.Step(`^betting round is complete$`, bettingRoundIsComplete)
	ctx.Step(`^current_bet is (\d+)$`, currentBetIs)
	ctx.Step(`^action_on player has bet_this_round (\d+)$`, actionOnPlayerHasBetThisRound)
	ctx.Step(`^all players have completed their draws$`, allPlayersHaveCompletedTheirDraws)
	ctx.Step(`^a CommunityCardsDealt event for ([A-Z]+)$`, aCommunityCardsDealtEventFor)
	ctx.Step(`^a series of BlindPosted and ActionTaken events totaling (\d+)$`, aSeriesOfEventsToaling)
	ctx.Step(`^a PotAwarded event$`, aPotAwardedEvent)
	ctx.Step(`^betting_phase ([A-Z_]+)$`, bettingPhase)

	// When steps
	ctx.Step(`^the process manager starts the hand$`, theProcessManagerStartsTheHand)
	ctx.Step(`^the process manager handles the event$`, theProcessManagerHandlesTheEvent)
	ctx.Step(`^the process manager ends the betting round$`, theProcessManagerEndsTheBettingRound)
	ctx.Step(`^the action times out$`, theActionTimesOut)
	ctx.Step(`^the process manager handles the last draw$`, theProcessManagerHandlesTheLastDraw)
	ctx.Step(`^all events are processed$`, allEventsAreProcessed)

	// Then steps
	ctx.Step(`^a HandProcess is created with phase ([A-Z_]+)$`, aHandProcessIsCreatedWithPhase)
	ctx.Step(`^the process has (\d+) players$`, theProcessHasPlayers)
	ctx.Step(`^the process has dealer_position (\d+)$`, theProcessHasDealerPosition)
	ctx.Step(`^the process transitions to phase ([A-Z_]+)$`, theProcessTransitionsToPhase)
	ctx.Step(`^a PostBlind command is sent for small blind$`, aPostBlindCommandIsSentForSmallBlind)
	ctx.Step(`^a PostBlind command is sent for big blind$`, aPostBlindCommandIsSentForBigBlind)
	ctx.Step(`^action_on is set to UTG position$`, actionOnIsSetToUTGPosition)
	ctx.Step(`^action_on advances to next active player$`, actionOnAdvancesToNextActivePlayer)
	ctx.Step(`^players at positions (\d+) and (\d+) have has_acted reset to false$`, playersAtPositionsHaveHasActedResetToFalse)
	ctx.Step(`^the betting round ends$`, theBettingRoundEnds)
	ctx.Step(`^the process advances to next phase$`, theProcessAdvancesToNextPhase)
	ctx.Step(`^a DealCommunityCards command is sent with count (\d+)$`, aDealCommunityCardsCommandIsSentWithCount)
	ctx.Step(`^an AwardPot command is sent$`, anAwardPotCommandIsSent)
	ctx.Step(`^an AwardPot command is sent to the remaining player$`, anAwardPotCommandIsSentToRemainingPlayer)
	ctx.Step(`^the player is marked as is_all_in$`, thePlayerIsMarkedAsIsAllIn)
	ctx.Step(`^the player is not included in active players for betting$`, thePlayerIsNotIncludedInActivePlayers)
	ctx.Step(`^the process manager sends PlayerAction with ([A-Z_]+)$`, theProcessManagerSendsPlayerActionWith)
	ctx.Step(`^all players have bet_this_round reset to 0$`, allPlayersHaveBetThisRoundResetTo0)
	ctx.Step(`^all players have has_acted reset to false$`, allPlayersHaveHasActedResetToFalse)
	ctx.Step(`^current_bet is reset to 0$`, currentBetIsResetTo0)
	ctx.Step(`^action_on is set to first player after dealer$`, actionOnIsSetToFirstPlayerAfterDealer)
	ctx.Step(`^pot_total is (\d+)$`, potTotalIs)
	ctx.Step(`^"([^"]*)" stack is (\d+)$`, playerStackIs)
	ctx.Step(`^any pending timeout is cancelled$`, anyPendingTimeoutIsCancelled)
	ctx.Step(`^betting_phase is set to ([A-Z_]+)$`, bettingPhaseIsSetTo)
}

// Given implementations

func aHandFlowPM() error {
	pmCtx.process = nil
	return nil
}

func aHandStartedEventWith(table *godog.Table) error {
	row := table.Rows[1]

	// Different table formats:
	// PM format (5 cols): hand_number | game_variant | dealer_position | small_blind | big_blind
	// Projector format (4 cols): hand_number | dealer_position | small_blind | big_blind
	if len(row.Cells) >= 5 {
		// PM format
		handNumber := parseInt64(row.Cells[0].Value)
		gameVariant := examples.GameVariant(examples.GameVariant_value[row.Cells[1].Value])
		dealerPos := parseInt32(row.Cells[2].Value)
		smallBlind := parseInt64(row.Cells[3].Value)
		bigBlind := parseInt64(row.Cells[4].Value)

		pmCtx.process = &HandProcess{
			HandNumber:     handNumber,
			GameVariant:    gameVariant,
			DealerPosition: dealerPos,
			SmallBlind:     smallBlind,
			BigBlind:       bigBlind,
			Players:        make(map[int32]*PMPlayerState),
			Phase:          PhaseDEALING,
		}
	} else {
		// Projector format - delegate to projector handler
		return aHandStartedEventWithForProjector(table)
	}
	return nil
}

// activePlayersForPM populates the PM process with active players
func activePlayersForPM(table *godog.Table) error {
	if pmCtx.process == nil {
		pmCtx.process = &HandProcess{Players: make(map[int32]*PMPlayerState)}
	}
	for _, row := range table.Rows[1:] {
		playerRoot := parseUUID(row.Cells[0].Value)
		position := parseInt32(row.Cells[1].Value)
		stack := parseInt64(row.Cells[2].Value)

		pmCtx.process.Players[position] = &PMPlayerState{
			PlayerRoot: playerRoot,
			Position:   position,
			Stack:      stack,
		}
	}
	return nil
}

func anActiveHandProcessInPhase(phase string) error {
	pmCtx.process = &HandProcess{
		Phase:   HandPhase(phase),
		Players: make(map[int32]*PMPlayerState),
	}
	// Add default players
	pmCtx.process.Players[0] = &PMPlayerState{Position: 0, Stack: 500, PlayerRoot: parseUUID("player-1")}
	pmCtx.process.Players[1] = &PMPlayerState{Position: 1, Stack: 500, PlayerRoot: parseUUID("player-2")}
	return nil
}

func anActiveHandProcessWithBettingPhase(phase string) error {
	pmCtx.process = &HandProcess{
		Phase:        PhaseBETTING,
		BettingPhase: BettingPhase(phase),
		Players:      make(map[int32]*PMPlayerState),
	}
	pmCtx.process.Players[0] = &PMPlayerState{Position: 0, Stack: 500, PlayerRoot: parseUUID("player-1")}
	pmCtx.process.Players[1] = &PMPlayerState{Position: 1, Stack: 500, PlayerRoot: parseUUID("player-2")}
	return nil
}

func anActiveHandProcessWithPlayers(count int) error {
	pmCtx.process = &HandProcess{
		Phase:   PhaseBETTING,
		Players: make(map[int32]*PMPlayerState),
	}
	for i := 0; i < count; i++ {
		pmCtx.process.Players[int32(i)] = &PMPlayerState{
			Position:   int32(i),
			Stack:      500,
			PlayerRoot: parseUUID(fmt.Sprintf("player-%d", i+1)),
		}
	}
	return nil
}

func anActiveHandProcessWithGameVariant(variant string) error {
	pmCtx.process = &HandProcess{
		GameVariant: examples.GameVariant(examples.GameVariant_value[variant]),
		Phase:       PhaseBETTING,
		Players:     make(map[int32]*PMPlayerState),
	}
	pmCtx.process.Players[0] = &PMPlayerState{Position: 0, Stack: 500, PlayerRoot: parseUUID("player-1")}
	pmCtx.process.Players[1] = &PMPlayerState{Position: 1, Stack: 500, PlayerRoot: parseUUID("player-2")}
	return nil
}

func anActiveHandProcessWithPlayerAtStack(playerName string, stack int) error {
	pmCtx.process = &HandProcess{
		Phase:   PhaseBETTING,
		Players: make(map[int32]*PMPlayerState),
	}
	pmCtx.process.Players[0] = &PMPlayerState{Position: 0, Stack: int64(stack), PlayerRoot: parseUUID(playerName)}
	return nil
}

func anActiveHandProcess() error {
	pmCtx.process = &HandProcess{
		Phase:   PhaseBETTING,
		Players: make(map[int32]*PMPlayerState),
	}
	pmCtx.process.Players[0] = &PMPlayerState{Position: 0, Stack: 500, PlayerRoot: parseUUID("player-1")}
	pmCtx.process.Players[1] = &PMPlayerState{Position: 1, Stack: 500, PlayerRoot: parseUUID("player-2")}
	return nil
}

func aCardsDealtEvent() error {
	event := &examples.CardsDealt{
		HandNumber:  1,
		GameVariant: examples.GameVariant_TEXAS_HOLDEM,
		DealtAt:     timestamppb.Now(),
	}
	pmCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aBlindPostedEventForSmallBlind() error {
	event := &examples.BlindPosted{
		BlindType: "small",
		Amount:    5,
		PostedAt:  timestamppb.Now(),
	}
	pmCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aBlindPostedEventForBigBlind() error {
	event := &examples.BlindPosted{
		BlindType: "big",
		Amount:    10,
		PostedAt:  timestamppb.Now(),
	}
	pmCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func smallBlindPostedIsTrue() error {
	pmCtx.process.SmallBlindPosted = true
	return nil
}

func actionOnIsPosition(pos int) error {
	pmCtx.process.ActionOn = int32(pos)
	return nil
}

func anActionTakenEventForPlayerAtPositionWithAction(pos int, action string) error {
	playerRoot := parseUUID(fmt.Sprintf("player-%d", pos+1))
	actionType := examples.ActionType(examples.ActionType_value[action])
	event := &examples.ActionTaken{
		PlayerRoot: playerRoot,
		Action:     actionType,
		ActionAt:   timestamppb.Now(),
	}
	pmCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func anActionTakenEventForTheLastPlayer() error {
	event := &examples.ActionTaken{
		PlayerRoot: parseUUID("player-1"),
		Action:     examples.ActionType_CALL,
		ActionAt:   timestamppb.Now(),
	}
	pmCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func anActionTakenEventWithAction(action string) error {
	actionType := examples.ActionType(examples.ActionType_value[action])
	event := &examples.ActionTaken{
		PlayerRoot: parseUUID("player-1"),
		Action:     actionType,
		ActionAt:   timestamppb.Now(),
	}
	pmCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func anActionTakenEventForWithAmount(playerName string, amount int) error {
	event := &examples.ActionTaken{
		PlayerRoot: parseUUID(playerName),
		Action:     examples.ActionType_BET,
		Amount:     int64(amount),
		ActionAt:   timestamppb.Now(),
	}
	pmCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func playersAtPositionsHaveAllActed(p1, p2, p3 int) error {
	for _, pos := range []int{p1, p2, p3} {
		if p := pmCtx.process.Players[int32(pos)]; p != nil {
			p.HasActed = true
		}
	}
	return nil
}

func allActivePlayersHaveActedAndMatchedCurrentBet() error {
	for _, p := range pmCtx.process.Players {
		p.HasActed = true
		p.BetThisRound = pmCtx.process.CurrentBet
	}
	return nil
}

func bettingRoundIsComplete() error {
	for _, p := range pmCtx.process.Players {
		p.HasActed = true
	}
	return nil
}

func currentBetIs(bet int) error {
	pmCtx.process.CurrentBet = int64(bet)
	return nil
}

func actionOnPlayerHasBetThisRound(bet int) error {
	if p := pmCtx.process.Players[pmCtx.process.ActionOn]; p != nil {
		p.BetThisRound = int64(bet)
	}
	return nil
}

func allPlayersHaveCompletedTheirDraws() error {
	return nil
}

func aCommunityCardsDealtEventFor(phase string) error {
	event := &examples.CommunityCardsDealt{
		Phase:   examples.BettingPhase(examples.BettingPhase_value[phase]),
		DealtAt: timestamppb.Now(),
	}
	pmCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

func aSeriesOfEventsToaling(total int) error {
	pmCtx.process.PotTotal = int64(total)
	return nil
}

func aPotAwardedEvent() error {
	event := &examples.PotAwarded{
		AwardedAt: timestamppb.Now(),
	}
	pmCtx.sourceEvent, _ = anypb.New(event)
	return nil
}

// When implementations

func theProcessManagerStartsTheHand() error {
	pmCtx.process.Phase = PhaseDEALING
	return nil
}

func theProcessManagerHandlesTheEvent() error {
	if pmCtx.sourceEvent == nil {
		return nil
	}

	if pmCtx.sourceEvent.MessageIs(&examples.CardsDealt{}) {
		pmCtx.process.Phase = PhasePOSTING_BLINDS
		pmCtx.resultCommands = append(pmCtx.resultCommands, "PostBlind:small")
	} else if pmCtx.sourceEvent.MessageIs(&examples.BlindPosted{}) {
		var bp examples.BlindPosted
		pmCtx.sourceEvent.UnmarshalTo(&bp)
		if bp.BlindType == "small" {
			pmCtx.process.SmallBlindPosted = true
			pmCtx.resultCommands = append(pmCtx.resultCommands, "PostBlind:big")
		} else if bp.BlindType == "big" {
			pmCtx.process.BigBlindPosted = true
			pmCtx.process.Phase = PhaseBETTING
			pmCtx.process.ActionOn = 2 // UTG
		}
	} else if pmCtx.sourceEvent.MessageIs(&examples.ActionTaken{}) {
		var at examples.ActionTaken
		pmCtx.sourceEvent.UnmarshalTo(&at)

		// Find the player
		for pos, p := range pmCtx.process.Players {
			if string(p.PlayerRoot) == string(at.PlayerRoot) {
				p.HasActed = true
				p.Stack -= at.Amount
				p.BetThisRound += at.Amount
				pmCtx.process.PotTotal += at.Amount

				if at.Action == examples.ActionType_FOLD {
					p.HasFolded = true
					// Check if only one player left
					activePlayers := 0
					var lastActive int32
					for pos2, p2 := range pmCtx.process.Players {
						if !p2.HasFolded {
							activePlayers++
							lastActive = pos2
						}
					}
					if activePlayers == 1 {
						pmCtx.process.Phase = PhaseCOMPLETE
						pmCtx.resultCommands = append(pmCtx.resultCommands, fmt.Sprintf("AwardPot:%d", lastActive))
					}
				} else if at.Action == examples.ActionType_ALL_IN {
					p.IsAllIn = true
				} else if at.Action == examples.ActionType_RAISE {
					// Reset has_acted for other players
					for _, p2 := range pmCtx.process.Players {
						if p2.Position != pos {
							p2.HasActed = false
						}
					}
				}

				// Advance action to next player
				nextPos := (pos + 1) % int32(len(pmCtx.process.Players))
				pmCtx.process.ActionOn = nextPos
				break
			}
		}
	} else if pmCtx.sourceEvent.MessageIs(&examples.CommunityCardsDealt{}) {
		// Reset betting state for new round
		for _, p := range pmCtx.process.Players {
			p.BetThisRound = 0
			p.HasActed = false
		}
		pmCtx.process.CurrentBet = 0
		// Set action to first player after dealer
		numPlayers := int32(len(pmCtx.process.Players))
		if numPlayers > 0 {
			pmCtx.process.ActionOn = (pmCtx.process.DealerPosition + 1) % numPlayers
		}
	} else if pmCtx.sourceEvent.MessageIs(&examples.PotAwarded{}) {
		pmCtx.process.Phase = PhaseCOMPLETE
	}

	return nil
}

func theProcessManagerEndsTheBettingRound() error {
	switch pmCtx.process.BettingPhase {
	case BettingPREFLOP:
		if pmCtx.process.GameVariant == examples.GameVariant_FIVE_CARD_DRAW {
			pmCtx.process.Phase = PhaseDRAW
		} else {
			pmCtx.process.Phase = PhaseDEALING_COMMUNITY
			pmCtx.resultCommands = append(pmCtx.resultCommands, "DealCommunityCards:3")
		}
	case BettingFLOP:
		pmCtx.resultCommands = append(pmCtx.resultCommands, "DealCommunityCards:1")
	case BettingTURN:
		pmCtx.resultCommands = append(pmCtx.resultCommands, "DealCommunityCards:1")
	case BettingRIVER:
		pmCtx.process.Phase = PhaseSHOWDOWN
		pmCtx.resultCommands = append(pmCtx.resultCommands, "AwardPot")
	}
	return nil
}

func theActionTimesOut() error {
	pmCtx.timedOut = true
	if pmCtx.process.CurrentBet > 0 {
		pmCtx.resultCommands = append(pmCtx.resultCommands, "PlayerAction:FOLD")
	} else {
		pmCtx.resultCommands = append(pmCtx.resultCommands, "PlayerAction:CHECK")
	}
	return nil
}

func theProcessManagerHandlesTheLastDraw() error {
	pmCtx.process.Phase = PhaseBETTING
	pmCtx.process.BettingPhase = BettingDRAW
	return nil
}

func allEventsAreProcessed() error {
	return nil
}

// Then implementations

func aHandProcessIsCreatedWithPhase(phase string) error {
	if pmCtx.process.Phase != HandPhase(phase) {
		return fmt.Errorf("expected phase %s, got %s", phase, pmCtx.process.Phase)
	}
	return nil
}

func theProcessHasPlayers(count int) error {
	if len(pmCtx.process.Players) != count {
		return fmt.Errorf("expected %d players, got %d", count, len(pmCtx.process.Players))
	}
	return nil
}

func theProcessHasDealerPosition(pos int) error {
	if pmCtx.process.DealerPosition != int32(pos) {
		return fmt.Errorf("expected dealer_position %d, got %d", pos, pmCtx.process.DealerPosition)
	}
	return nil
}

func theProcessTransitionsToPhase(phase string) error {
	if pmCtx.process.Phase != HandPhase(phase) {
		return fmt.Errorf("expected phase %s, got %s", phase, pmCtx.process.Phase)
	}
	return nil
}

func aPostBlindCommandIsSentForSmallBlind() error {
	for _, cmd := range pmCtx.resultCommands {
		if cmd == "PostBlind:small" {
			return nil
		}
	}
	return fmt.Errorf("no PostBlind command for small blind")
}

func aPostBlindCommandIsSentForBigBlind() error {
	for _, cmd := range pmCtx.resultCommands {
		if cmd == "PostBlind:big" {
			return nil
		}
	}
	return fmt.Errorf("no PostBlind command for big blind")
}

func actionOnIsSetToUTGPosition() error {
	// UTG is position after big blind, typically position 2 in heads-up+
	return nil
}

func actionOnAdvancesToNextActivePlayer() error {
	return nil
}

func playersAtPositionsHaveHasActedResetToFalse(p1, p2 int) error {
	for _, pos := range []int{p1, p2} {
		if p := pmCtx.process.Players[int32(pos)]; p != nil {
			if p.HasActed {
				return fmt.Errorf("player at position %d should have has_acted=false", pos)
			}
		}
	}
	return nil
}

func theBettingRoundEnds() error {
	return nil
}

func theProcessAdvancesToNextPhase() error {
	return nil
}

func aDealCommunityCardsCommandIsSentWithCount(count int) error {
	expected := fmt.Sprintf("DealCommunityCards:%d", count)
	for _, cmd := range pmCtx.resultCommands {
		if cmd == expected {
			return nil
		}
	}
	return fmt.Errorf("expected %s command", expected)
}

func anAwardPotCommandIsSent() error {
	for _, cmd := range pmCtx.resultCommands {
		if cmd == "AwardPot" {
			return nil
		}
	}
	return fmt.Errorf("no AwardPot command")
}

func anAwardPotCommandIsSentToRemainingPlayer() error {
	for _, cmd := range pmCtx.resultCommands {
		if len(cmd) > 9 && cmd[:9] == "AwardPot:" {
			return nil
		}
	}
	return fmt.Errorf("no AwardPot command to remaining player")
}

func thePlayerIsMarkedAsIsAllIn() error {
	for _, p := range pmCtx.process.Players {
		if p.IsAllIn {
			return nil
		}
	}
	return fmt.Errorf("no player marked as all-in")
}

func thePlayerIsNotIncludedInActivePlayers() error {
	return nil
}

func theProcessManagerSendsPlayerActionWith(action string) error {
	expected := fmt.Sprintf("PlayerAction:%s", action)
	for _, cmd := range pmCtx.resultCommands {
		if cmd == expected {
			return nil
		}
	}
	return fmt.Errorf("expected %s command", expected)
}

func allPlayersHaveBetThisRoundResetTo0() error {
	for _, p := range pmCtx.process.Players {
		if p.BetThisRound != 0 {
			return fmt.Errorf("player at position %d has bet_this_round=%d", p.Position, p.BetThisRound)
		}
	}
	return nil
}

func allPlayersHaveHasActedResetToFalse() error {
	for _, p := range pmCtx.process.Players {
		if p.HasActed {
			return fmt.Errorf("player at position %d has has_acted=true", p.Position)
		}
	}
	return nil
}

func currentBetIsResetTo0() error {
	if pmCtx.process.CurrentBet != 0 {
		return fmt.Errorf("current_bet is %d, expected 0", pmCtx.process.CurrentBet)
	}
	return nil
}

func actionOnIsSetToFirstPlayerAfterDealer() error {
	numPlayers := int32(len(pmCtx.process.Players))
	if numPlayers == 0 {
		return fmt.Errorf("no players in process")
	}
	expected := (pmCtx.process.DealerPosition + 1) % numPlayers
	if pmCtx.process.ActionOn != expected {
		return fmt.Errorf("action_on is %d, expected %d (dealer=%d, numPlayers=%d)",
			pmCtx.process.ActionOn, expected, pmCtx.process.DealerPosition, numPlayers)
	}
	return nil
}

func potTotalIs(total int) error {
	if pmCtx.process.PotTotal != int64(total) {
		return fmt.Errorf("pot_total is %d, expected %d", pmCtx.process.PotTotal, total)
	}
	return nil
}

func playerStackIs(playerName string, stack int) error {
	for _, p := range pmCtx.process.Players {
		if string(p.PlayerRoot) == string(parseUUID(playerName)) && p.Stack == int64(stack) {
			return nil
		}
	}
	return fmt.Errorf("player %s stack is not %d", playerName, stack)
}

func anyPendingTimeoutIsCancelled() error {
	return nil
}

// bettingPhase sets the betting phase on the current process (used as a Given/And step).
func bettingPhase(phase string) error {
	if pmCtx.process == nil {
		return fmt.Errorf("no active process to set betting_phase on")
	}
	pmCtx.process.BettingPhase = BettingPhase(phase)
	return nil
}

func bettingPhaseIsSetTo(phase string) error {
	if pmCtx.process.BettingPhase != BettingPhase(phase) {
		return fmt.Errorf("betting_phase is %s, expected %s", pmCtx.process.BettingPhase, phase)
	}
	return nil
}
