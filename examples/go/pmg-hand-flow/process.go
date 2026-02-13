// Process manager state machine for hand flow orchestration.
package main

import (
	"encoding/hex"
	"sort"

	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/types/known/anypb"
)

// HandPhase represents the internal state machine phase.
type HandPhase int

const (
	PhaseWaitingForStart HandPhase = iota
	PhaseDealing
	PhasePostingBlinds
	PhaseBetting
	PhaseDealingCommunity
	PhaseDraw
	PhaseShowdown
	PhaseAwardingPot
	PhaseComplete
)

// PlayerState tracks a player's state within the process manager.
type PlayerState struct {
	PlayerRoot    []byte
	Position      int32
	Stack         int64
	BetThisRound  int64
	TotalInvested int64
	HasActed      bool
	HasFolded     bool
	IsAllIn       bool
}

// HandProcess manages the orchestration state for a single hand.
type HandProcess struct {
	HandID      string
	TableRoot   []byte
	HandNumber  int64
	GameVariant examples.GameVariant

	// State machine
	Phase        HandPhase
	BettingPhase examples.BettingPhase

	// Player tracking
	Players         map[int32]*PlayerState // position -> PlayerState
	ActivePositions []int32

	// Position tracking
	DealerPosition     int32
	SmallBlindPosition int32
	BigBlindPosition   int32
	ActionOn           int32
	LastAggressor      int32

	// Betting state
	SmallBlind   int64
	BigBlind     int64
	CurrentBet   int64
	MinRaise     int64
	PotTotal     int64

	// Blind posting progress
	SmallBlindPosted bool
	BigBlindPosted   bool

	// Community cards (for phase tracking)
	CommunityCardCount int
}

// NewHandProcess creates a new hand process from a HandStarted event.
func NewHandProcess(event *examples.HandStarted, tableRoot []byte) *HandProcess {
	handID := hex.EncodeToString(tableRoot[:min(len(tableRoot), 8)]) + "_" + string(rune(event.HandNumber))

	process := &HandProcess{
		HandID:             handID,
		TableRoot:          tableRoot,
		HandNumber:         event.HandNumber,
		GameVariant:        event.GameVariant,
		DealerPosition:     event.DealerPosition,
		SmallBlindPosition: event.SmallBlindPosition,
		BigBlindPosition:   event.BigBlindPosition,
		SmallBlind:         event.SmallBlind,
		BigBlind:           event.BigBlind,
		Phase:              PhaseDealing,
		BettingPhase:       examples.BettingPhase_PREFLOP,
		Players:            make(map[int32]*PlayerState),
		ActivePositions:    make([]int32, 0),
		ActionOn:           -1,
		LastAggressor:      -1,
	}

	// Initialize player states
	for _, player := range event.ActivePlayers {
		process.Players[player.Position] = &PlayerState{
			PlayerRoot: player.PlayerRoot,
			Position:   player.Position,
			Stack:      player.Stack,
		}
		process.ActivePositions = append(process.ActivePositions, player.Position)
	}

	sort.Slice(process.ActivePositions, func(i, j int) bool {
		return process.ActivePositions[i] < process.ActivePositions[j]
	})

	return process
}

// HandleCardsDealt processes a CardsDealt event and returns commands.
func (p *HandProcess) HandleCardsDealt(event *examples.CardsDealt) []*pb.CommandBook {
	p.Phase = PhasePostingBlinds
	p.MinRaise = p.BigBlind

	return p.postNextBlind()
}

// HandleBlindPosted processes a BlindPosted event and returns commands.
func (p *HandProcess) HandleBlindPosted(event *examples.BlindPosted) []*pb.CommandBook {
	// Update player state
	for _, player := range p.Players {
		if bytesEqual(player.PlayerRoot, event.PlayerRoot) {
			player.Stack = event.PlayerStack
			player.BetThisRound = event.Amount
			player.TotalInvested = event.Amount
			break
		}
	}

	p.PotTotal = event.PotTotal

	if event.BlindType == "small" {
		p.SmallBlindPosted = true
		p.CurrentBet = event.Amount
		return p.postNextBlind()
	} else if event.BlindType == "big" {
		p.BigBlindPosted = true
		p.CurrentBet = event.Amount
		return p.startBetting()
	}

	return nil
}

// HandleActionTaken processes an ActionTaken event and returns commands.
func (p *HandProcess) HandleActionTaken(event *examples.ActionTaken) []*pb.CommandBook {
	// Update player state
	for pos, player := range p.Players {
		if bytesEqual(player.PlayerRoot, event.PlayerRoot) {
			player.Stack = event.PlayerStack
			player.HasActed = true

			switch event.Action {
			case examples.ActionType_FOLD:
				player.HasFolded = true
			case examples.ActionType_ALL_IN:
				player.IsAllIn = true
				player.BetThisRound += event.Amount
				player.TotalInvested += event.Amount
			case examples.ActionType_CALL, examples.ActionType_BET, examples.ActionType_RAISE:
				player.BetThisRound += event.Amount
				player.TotalInvested += event.Amount
			}

			if event.Action == examples.ActionType_BET ||
				event.Action == examples.ActionType_RAISE ||
				event.Action == examples.ActionType_ALL_IN {
				if player.BetThisRound > p.CurrentBet {
					raiseAmount := player.BetThisRound - p.CurrentBet
					p.CurrentBet = player.BetThisRound
					if raiseAmount > p.MinRaise {
						p.MinRaise = raiseAmount
					}
					p.LastAggressor = pos
					// Reset has_acted for all other active players
					for _, other := range p.Players {
						if other.Position != pos && !other.HasFolded && !other.IsAllIn {
							other.HasActed = false
						}
					}
				}
			}
			break
		}
	}

	p.PotTotal = event.PotTotal

	// Check if betting round is complete
	if p.isBettingComplete() {
		return p.endBettingRound()
	}

	// Move to next player
	return p.advanceAction()
}

// HandleCommunityDealt processes a CommunityCardsDealt event and returns commands.
func (p *HandProcess) HandleCommunityDealt(event *examples.CommunityCardsDealt) []*pb.CommandBook {
	p.CommunityCardCount = len(event.AllCommunityCards)
	p.BettingPhase = event.Phase
	return p.startBetting()
}

// HandlePotAwarded marks the hand as complete.
func (p *HandProcess) HandlePotAwarded(event *examples.PotAwarded) {
	p.Phase = PhaseComplete
}

func (p *HandProcess) postNextBlind() []*pb.CommandBook {
	if !p.SmallBlindPosted {
		player := p.Players[p.SmallBlindPosition]
		if player != nil {
			return []*pb.CommandBook{p.buildPostBlindCommand(player, "small", p.SmallBlind)}
		}
	} else if !p.BigBlindPosted {
		player := p.Players[p.BigBlindPosition]
		if player != nil {
			return []*pb.CommandBook{p.buildPostBlindCommand(player, "big", p.BigBlind)}
		}
	}
	return nil
}

func (p *HandProcess) buildPostBlindCommand(player *PlayerState, blindType string, amount int64) *pb.CommandBook {
	cmd := &examples.PostBlind{
		PlayerRoot: player.PlayerRoot,
		BlindType:  blindType,
		Amount:     amount,
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return nil
	}

	handRoot := p.TableRoot // Hand root is derived from table root + hand number
	return &pb.CommandBook{
		Cover: &pb.Cover{
			Domain: "hand",
			Root:   &pb.UUID{Value: handRoot},
		},
		Pages: []*pb.CommandPage{
			{
				Sequence: 0, // Will be set by caller with destination state
				Command:  cmdAny,
			},
		},
	}
}

func (p *HandProcess) startBetting() []*pb.CommandBook {
	p.Phase = PhaseBetting

	// Reset betting state for new round
	for _, player := range p.Players {
		player.BetThisRound = 0
		player.HasActed = false
	}

	p.CurrentBet = 0

	// Determine first to act
	if p.BettingPhase == examples.BettingPhase_PREFLOP {
		// Preflop: UTG (after big blind)
		p.ActionOn = p.findNextActive(p.BigBlindPosition)
	} else {
		// Postflop: first active player after dealer
		p.ActionOn = p.findNextActive(p.DealerPosition)
	}

	// Request action (in a real implementation, would notify the player)
	// For now, we just track who is to act
	return nil
}

func (p *HandProcess) advanceAction() []*pb.CommandBook {
	p.ActionOn = p.findNextActive(p.ActionOn)
	// Request action from next player
	return nil
}

func (p *HandProcess) findNextActive(afterPosition int32) int32 {
	n := len(p.ActivePositions)
	if n == 0 {
		return -1
	}

	// Find starting index
	startIdx := 0
	for i, pos := range p.ActivePositions {
		if pos > afterPosition {
			startIdx = i
			break
		}
	}
	if p.ActivePositions[startIdx] <= afterPosition {
		startIdx = 0 // Wrap around
	}

	// Find next active player
	for i := 0; i < n; i++ {
		idx := (startIdx + i) % n
		pos := p.ActivePositions[idx]
		player := p.Players[pos]
		if player != nil && !player.HasFolded && !player.IsAllIn {
			return pos
		}
	}

	return -1
}

func (p *HandProcess) isBettingComplete() bool {
	activePlayers := make([]*PlayerState, 0)
	for _, player := range p.Players {
		if !player.HasFolded && !player.IsAllIn {
			activePlayers = append(activePlayers, player)
		}
	}

	if len(activePlayers) <= 1 {
		return true
	}

	// All active players must have acted and matched the bet
	for _, player := range activePlayers {
		if !player.HasActed {
			return false
		}
		if player.BetThisRound < p.CurrentBet && !player.IsAllIn {
			return false
		}
	}

	return true
}

func (p *HandProcess) endBettingRound() []*pb.CommandBook {
	// Count active players
	playersInHand := make([]*PlayerState, 0)
	activePlayers := make([]*PlayerState, 0)
	for _, player := range p.Players {
		if !player.HasFolded {
			playersInHand = append(playersInHand, player)
			if !player.IsAllIn {
				activePlayers = append(activePlayers, player)
			}
		}
	}

	// If only one player left, award pot
	if len(playersInHand) == 1 {
		return p.awardPotToLastPlayer(playersInHand[0])
	}

	// Determine next phase based on game variant
	switch p.GameVariant {
	case examples.GameVariant_TEXAS_HOLDEM, examples.GameVariant_OMAHA:
		return p.advanceHoldemPhase(len(activePlayers))
	case examples.GameVariant_FIVE_CARD_DRAW:
		return p.advanceDrawPhase(len(activePlayers))
	}

	return nil
}

func (p *HandProcess) advanceHoldemPhase(activeCount int) []*pb.CommandBook {
	switch p.BettingPhase {
	case examples.BettingPhase_PREFLOP:
		p.Phase = PhaseDealingCommunity
		return p.dealCommunity(3) // Flop
	case examples.BettingPhase_FLOP:
		p.Phase = PhaseDealingCommunity
		return p.dealCommunity(1) // Turn
	case examples.BettingPhase_TURN:
		p.Phase = PhaseDealingCommunity
		return p.dealCommunity(1) // River
	case examples.BettingPhase_RIVER:
		return p.startShowdown()
	}
	return nil
}

func (p *HandProcess) advanceDrawPhase(activeCount int) []*pb.CommandBook {
	if p.BettingPhase == examples.BettingPhase_PREFLOP {
		p.Phase = PhaseDraw
		// Draw phase handled by player commands
		return nil
	} else if p.BettingPhase == examples.BettingPhase_DRAW {
		if p.Phase == PhaseDraw {
			// Coming from draw phase, start final betting
			p.BettingPhase = examples.BettingPhase_DRAW
			return p.startBetting()
		}
		// Coming from final betting, go to showdown
		return p.startShowdown()
	}
	return nil
}

func (p *HandProcess) dealCommunity(count int) []*pb.CommandBook {
	cmd := &examples.DealCommunityCards{
		Count: int32(count),
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return nil
	}

	return []*pb.CommandBook{
		{
			Cover: &pb.Cover{
				Domain: "hand",
				Root:   &pb.UUID{Value: p.TableRoot},
			},
			Pages: []*pb.CommandPage{
				{
					Sequence: 0,
					Command:  cmdAny,
				},
			},
		},
	}
}

func (p *HandProcess) startShowdown() []*pb.CommandBook {
	p.Phase = PhaseShowdown
	p.BettingPhase = examples.BettingPhase_SHOWDOWN

	// Auto-award to best hand (simplified)
	return p.autoAwardPot()
}

func (p *HandProcess) awardPotToLastPlayer(winner *PlayerState) []*pb.CommandBook {
	p.Phase = PhaseComplete

	cmd := &examples.AwardPot{
		Awards: []*examples.PotAward{
			{
				PlayerRoot: winner.PlayerRoot,
				Amount:     p.PotTotal,
				PotType:    "main",
			},
		},
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return nil
	}

	return []*pb.CommandBook{
		{
			Cover: &pb.Cover{
				Domain: "hand",
				Root:   &pb.UUID{Value: p.TableRoot},
			},
			Pages: []*pb.CommandPage{
				{
					Sequence: 0,
					Command:  cmdAny,
				},
			},
		},
	}
}

func (p *HandProcess) autoAwardPot() []*pb.CommandBook {
	playersInHand := make([]*PlayerState, 0)
	for _, player := range p.Players {
		if !player.HasFolded {
			playersInHand = append(playersInHand, player)
		}
	}

	if len(playersInHand) == 0 {
		return nil
	}

	// Split pot evenly (real implementation would evaluate hands)
	split := p.PotTotal / int64(len(playersInHand))
	remainder := p.PotTotal % int64(len(playersInHand))

	awards := make([]*examples.PotAward, len(playersInHand))
	for i, player := range playersInHand {
		amount := split
		if int64(i) < remainder {
			amount++
		}
		awards[i] = &examples.PotAward{
			PlayerRoot: player.PlayerRoot,
			Amount:     amount,
			PotType:    "main",
		}
	}

	cmd := &examples.AwardPot{
		Awards: awards,
	}

	cmdAny, err := anypb.New(cmd)
	if err != nil {
		return nil
	}

	return []*pb.CommandBook{
		{
			Cover: &pb.Cover{
				Domain: "hand",
				Root:   &pb.UUID{Value: p.TableRoot},
			},
			Pages: []*pb.CommandPage{
				{
					Sequence: 0,
					Command:  cmdAny,
				},
			},
		},
	}
}

func bytesEqual(a, b []byte) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
