// Package handlers implements hand aggregate command handlers for testing.
//
// These functional handlers mirror the OO handlers in the main package,
// enabling unit testing without importing the main package.
package handlers

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	mathrand "math/rand"
	"sort"
	"time"

	angzarr "github.com/benjaminabbitt/angzarr/client/go"
	pb "github.com/benjaminabbitt/angzarr/client/go/proto/angzarr"
	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
	"google.golang.org/protobuf/types/known/anypb"
	"google.golang.org/protobuf/types/known/timestamppb"
)

// HandleDealCards handles the DealCards command.
func HandleDealCards(_ *pb.EventBook, cmdAny *anypb.Any, state HandState) (*anypb.Any, error) {
	var cmd examples.DealCards
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Hand already dealt")
	}

	// Validate
	if len(cmd.Players) < 2 {
		return nil, angzarr.NewCommandRejectedError("Need at least 2 players")
	}

	// Compute
	deck := createDeck()
	seed := cmd.DeckSeed
	if len(seed) == 0 {
		seed = make([]byte, 32)
		rand.Read(seed)
	}
	shuffleDeck(deck, seed)

	cardsPerPlayer := getCardsPerPlayer(cmd.GameVariant)
	playerCards := make([]*examples.PlayerHoleCards, len(cmd.Players))

	for i, player := range cmd.Players {
		cards := make([]*examples.Card, cardsPerPlayer)
		for j := 0; j < cardsPerPlayer; j++ {
			cards[j] = deck[0]
			deck = deck[1:]
		}
		playerCards[i] = &examples.PlayerHoleCards{
			PlayerRoot: player.PlayerRoot,
			Cards:      cards,
		}
	}

	event := &examples.CardsDealt{
		TableRoot:      cmd.TableRoot,
		HandNumber:     cmd.HandNumber,
		GameVariant:    cmd.GameVariant,
		PlayerCards:    playerCards,
		DealerPosition: cmd.DealerPosition,
		Players:        cmd.Players,
		RemainingDeck:  deck,
		DealtAt:        timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// HandlePostBlind handles the PostBlind command.
func HandlePostBlind(_ *pb.EventBook, cmdAny *anypb.Any, state HandState) (*anypb.Any, error) {
	var cmd examples.PostBlind
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}

	// Validate
	player := state.GetPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if cmd.Amount <= 0 {
		return nil, angzarr.NewCommandRejectedError("Amount must be positive")
	}

	// Compute
	actualAmount := cmd.Amount
	if actualAmount > player.Stack {
		actualAmount = player.Stack
	}

	newStack := player.Stack - actualAmount
	newPot := state.TotalPot() + actualAmount

	event := &examples.BlindPosted{
		PlayerRoot:  cmd.PlayerRoot,
		BlindType:   cmd.BlindType,
		Amount:      actualAmount,
		PlayerStack: newStack,
		PotTotal:    newPot,
		PostedAt:    timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// HandlePlayerAction handles the PlayerAction command.
func HandlePlayerAction(_ *pb.EventBook, cmdAny *anypb.Any, state HandState) (*anypb.Any, error) {
	var cmd examples.PlayerAction
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}
	if state.Status != "betting" {
		return nil, angzarr.NewCommandRejectedError("Not in betting phase")
	}

	// Validate
	player := state.GetPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if player.HasFolded {
		return nil, angzarr.NewCommandRejectedError("Player has folded")
	}
	if player.IsAllIn {
		return nil, angzarr.NewCommandRejectedError("Player is all-in")
	}

	amountToCall := state.CurrentBet - player.BetThisRound
	actualAmount := int64(0)

	switch cmd.Action {
	case examples.ActionType_FOLD:
		// Always valid

	case examples.ActionType_CHECK:
		if amountToCall > 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot check, must call or fold")
		}

	case examples.ActionType_CALL:
		if amountToCall <= 0 {
			return nil, angzarr.NewCommandRejectedError("Nothing to call")
		}
		actualAmount = amountToCall
		if actualAmount > player.Stack {
			actualAmount = player.Stack
		}

	case examples.ActionType_BET:
		if state.CurrentBet > 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot bet, use raise")
		}
		minBet := state.MinRaise
		if minBet == 0 {
			minBet = 10
		}
		if cmd.Amount < minBet {
			return nil, angzarr.NewCommandRejectedError(fmt.Sprintf("Bet must be at least %d", minBet))
		}
		actualAmount = cmd.Amount
		if actualAmount > player.Stack {
			actualAmount = player.Stack
		}

	case examples.ActionType_RAISE:
		if state.CurrentBet <= 0 {
			return nil, angzarr.NewCommandRejectedError("Cannot raise, use bet")
		}
		totalBet := cmd.Amount
		raiseAmount := totalBet - state.CurrentBet
		if raiseAmount < state.MinRaise {
			return nil, angzarr.NewCommandRejectedError("Raise below minimum")
		}
		actualAmount = totalBet - player.BetThisRound
		if actualAmount > player.Stack {
			actualAmount = player.Stack
		}

	case examples.ActionType_ALL_IN:
		actualAmount = player.Stack

	default:
		return nil, angzarr.NewCommandRejectedError("Unknown action")
	}

	// Compute
	newStack := player.Stack - actualAmount
	newPot := state.TotalPot() + actualAmount

	newCurrentBet := state.CurrentBet
	playerTotalBet := player.BetThisRound + actualAmount
	if playerTotalBet > newCurrentBet {
		newCurrentBet = playerTotalBet
	}

	action := cmd.Action
	if actualAmount == player.Stack && actualAmount > 0 {
		action = examples.ActionType_ALL_IN
	}

	amountToEmit := actualAmount
	if cmd.Action == examples.ActionType_BET || cmd.Action == examples.ActionType_RAISE {
		amountToEmit = cmd.Amount
	}

	event := &examples.ActionTaken{
		PlayerRoot:   cmd.PlayerRoot,
		Action:       action,
		Amount:       amountToEmit,
		PlayerStack:  newStack,
		PotTotal:     newPot,
		AmountToCall: newCurrentBet,
		ActionAt:     timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// HandleDealCommunityCards handles the DealCommunityCards command.
func HandleDealCommunityCards(_ *pb.EventBook, cmdAny *anypb.Any, state HandState) (*anypb.Any, error) {
	var cmd examples.DealCommunityCards
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}
	if state.GameVariant == examples.GameVariant_FIVE_CARD_DRAW {
		return nil, angzarr.NewCommandRejectedError("Five Card Draw does not use community cards")
	}

	// Validate
	var newPhase examples.BettingPhase
	var cardsToDeal int

	switch state.CurrentPhase {
	case examples.BettingPhase_PREFLOP:
		newPhase = examples.BettingPhase_FLOP
		cardsToDeal = 3
	case examples.BettingPhase_FLOP:
		newPhase = examples.BettingPhase_TURN
		cardsToDeal = 1
	case examples.BettingPhase_TURN:
		newPhase = examples.BettingPhase_RIVER
		cardsToDeal = 1
	default:
		return nil, angzarr.NewCommandRejectedError("Cannot deal more community cards")
	}

	if cmd.Count > 0 && int(cmd.Count) != cardsToDeal {
		return nil, angzarr.NewCommandRejectedError("Invalid card count for phase")
	}

	if len(state.RemainingDeck) < cardsToDeal {
		return nil, angzarr.NewCommandRejectedError("Not enough cards in deck")
	}

	// Compute
	newCards := state.RemainingDeck[:cardsToDeal]
	allCommunity := append(state.CommunityCards, newCards...)

	event := &examples.CommunityCardsDealt{
		Cards:             newCards,
		Phase:             newPhase,
		AllCommunityCards: allCommunity,
		DealtAt:           timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// HandleRequestDraw handles the RequestDraw command.
func HandleRequestDraw(_ *pb.EventBook, cmdAny *anypb.Any, state HandState) (*anypb.Any, error) {
	var cmd examples.RequestDraw
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}
	if state.GameVariant != examples.GameVariant_FIVE_CARD_DRAW {
		return nil, angzarr.NewCommandRejectedError("Draw is not supported in this game variant")
	}

	// Validate
	player := state.GetPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if player.HasFolded {
		return nil, angzarr.NewCommandRejectedError("Player has folded")
	}

	cardsToDiscard := len(cmd.CardIndices)
	for _, idx := range cmd.CardIndices {
		if idx < 0 || int(idx) >= len(player.HoleCards) {
			return nil, angzarr.NewCommandRejectedError("Invalid card index")
		}
	}

	seen := make(map[int32]bool)
	for _, idx := range cmd.CardIndices {
		if seen[idx] {
			return nil, angzarr.NewCommandRejectedError("Duplicate card index")
		}
		seen[idx] = true
	}

	if len(state.RemainingDeck) < cardsToDiscard {
		return nil, angzarr.NewCommandRejectedError("Not enough cards in deck")
	}

	// Compute
	newCards := make([]*examples.Card, cardsToDiscard)
	for i := 0; i < cardsToDiscard; i++ {
		newCards[i] = state.RemainingDeck[i]
	}

	event := &examples.DrawCompleted{
		PlayerRoot:     cmd.PlayerRoot,
		CardsDiscarded: int32(cardsToDiscard),
		CardsDrawn:     int32(cardsToDiscard),
		NewCards:       newCards,
		DrawnAt:        timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// HandleRevealCards handles the RevealCards command.
func HandleRevealCards(_ *pb.EventBook, cmdAny *anypb.Any, state HandState) (*anypb.Any, error) {
	var cmd examples.RevealCards
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}

	// Validate
	player := state.GetPlayerByRoot(cmd.PlayerRoot)
	if player == nil {
		return nil, angzarr.NewCommandRejectedError("Player not in hand")
	}
	if player.HasFolded {
		return nil, angzarr.NewCommandRejectedError("Player has folded")
	}

	// Compute - muck results in empty cards
	if cmd.Muck {
		event := &examples.CardsRevealed{
			PlayerRoot: cmd.PlayerRoot,
			Cards:      nil,
			Ranking:    nil,
			RevealedAt: timestamppb.New(time.Now()),
		}
		return anypb.New(event)
	}

	handRank := evaluateHand(state.GameVariant, player.HoleCards, state.CommunityCards)
	ranking := &examples.HandRanking{
		RankType: handRank.rankType,
		Score:    handRank.score,
	}

	event := &examples.CardsRevealed{
		PlayerRoot: cmd.PlayerRoot,
		Cards:      player.HoleCards,
		Ranking:    ranking,
		RevealedAt: timestamppb.New(time.Now()),
	}

	return anypb.New(event)
}

// HandleAwardPot handles the AwardPot command.
// Returns two events: PotAwarded and HandComplete
func HandleAwardPot(_ *pb.EventBook, cmdAny *anypb.Any, state HandState) ([]*anypb.Any, error) {
	var cmd examples.AwardPot
	if err := cmdAny.UnmarshalTo(&cmd); err != nil {
		return nil, err
	}

	// Guard
	if !state.Exists() {
		return nil, angzarr.NewCommandRejectedError("Hand does not exist")
	}
	if state.IsComplete() {
		return nil, angzarr.NewCommandRejectedError("Hand already complete")
	}

	// Validate
	if len(cmd.Awards) == 0 {
		return nil, angzarr.NewCommandRejectedError("No awards specified")
	}

	totalAwarded := int64(0)
	for _, award := range cmd.Awards {
		player := state.GetPlayerByRoot(award.PlayerRoot)
		if player == nil {
			return nil, angzarr.NewCommandRejectedError("Award to player not in hand")
		}
		if player.HasFolded {
			return nil, angzarr.NewCommandRejectedError("Cannot award to folded player")
		}
		totalAwarded += award.Amount
	}

	if totalAwarded > state.TotalPot() {
		return nil, angzarr.NewCommandRejectedError("Awards exceed pot total")
	}

	// Compute - return both events
	now := time.Now()
	winners := make([]*examples.PotWinner, len(cmd.Awards))
	for i, award := range cmd.Awards {
		winners[i] = &examples.PotWinner{
			PlayerRoot: award.PlayerRoot,
			Amount:     award.Amount,
			PotType:    award.PotType,
		}
	}

	finalStacks := make([]*examples.PlayerStackSnapshot, 0, len(state.Players))
	for _, player := range state.Players {
		finalStack := player.Stack
		for _, award := range cmd.Awards {
			if hex.EncodeToString(award.PlayerRoot) == hex.EncodeToString(player.PlayerRoot) {
				finalStack += award.Amount
			}
		}
		finalStacks = append(finalStacks, &examples.PlayerStackSnapshot{
			PlayerRoot: player.PlayerRoot,
			Stack:      finalStack,
			IsAllIn:    player.IsAllIn,
			HasFolded:  player.HasFolded,
		})
	}

	potAwarded := &examples.PotAwarded{
		Winners:   winners,
		AwardedAt: timestamppb.New(now),
	}
	handComplete := &examples.HandComplete{
		TableRoot:   state.TableRoot,
		HandNumber:  state.HandNumber,
		Winners:     winners,
		FinalStacks: finalStacks,
		CompletedAt: timestamppb.New(now),
	}

	potAwardedAny, err := anypb.New(potAwarded)
	if err != nil {
		return nil, err
	}
	handCompleteAny, err := anypb.New(handComplete)
	if err != nil {
		return nil, err
	}

	return []*anypb.Any{potAwardedAny, handCompleteAny}, nil
}

// Helper functions

func createDeck() []*examples.Card {
	suits := []examples.Suit{
		examples.Suit_CLUBS,
		examples.Suit_DIAMONDS,
		examples.Suit_HEARTS,
		examples.Suit_SPADES,
	}
	ranks := []examples.Rank{
		examples.Rank_TWO, examples.Rank_THREE, examples.Rank_FOUR,
		examples.Rank_FIVE, examples.Rank_SIX, examples.Rank_SEVEN,
		examples.Rank_EIGHT, examples.Rank_NINE, examples.Rank_TEN,
		examples.Rank_JACK, examples.Rank_QUEEN, examples.Rank_KING,
		examples.Rank_ACE,
	}

	deck := make([]*examples.Card, 0, 52)
	for _, suit := range suits {
		for _, rank := range ranks {
			deck = append(deck, &examples.Card{Suit: suit, Rank: rank})
		}
	}
	return deck
}

func shuffleDeck(deck []*examples.Card, seed []byte) {
	hash := sha256.Sum256(seed)
	seedInt := int64(binary.BigEndian.Uint64(hash[:8]))
	rng := mathrand.New(mathrand.NewSource(seedInt))

	for i := len(deck) - 1; i > 0; i-- {
		j := rng.Intn(i + 1)
		deck[i], deck[j] = deck[j], deck[i]
	}
}

func getCardsPerPlayer(variant examples.GameVariant) int {
	switch variant {
	case examples.GameVariant_TEXAS_HOLDEM:
		return 2
	case examples.GameVariant_OMAHA:
		return 4
	case examples.GameVariant_FIVE_CARD_DRAW:
		return 5
	case examples.GameVariant_SEVEN_CARD_STUD:
		return 2
	default:
		return 2
	}
}

// Hand evaluation helpers

type handRank struct {
	rankType examples.HandRankType
	score    int32
}

func evaluateHand(variant examples.GameVariant, holeCards, communityCards []*examples.Card) *handRank {
	switch variant {
	case examples.GameVariant_TEXAS_HOLDEM:
		allCards := append(holeCards, communityCards...)
		if len(allCards) < 5 {
			return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
		}
		return findBestFive(allCards)
	case examples.GameVariant_FIVE_CARD_DRAW:
		if len(holeCards) < 5 {
			return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
		}
		return evaluateFive(holeCards[:5])
	default:
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}
}

func findBestFive(cards []*examples.Card) *handRank {
	if len(cards) < 5 {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}

	var bestRank *handRank

	combos := combinations(cards, 5)
	for _, combo := range combos {
		rank := evaluateFive(combo)
		if bestRank == nil || rank.score > bestRank.score {
			bestRank = rank
		}
	}

	if bestRank == nil {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}
	return bestRank
}

func evaluateFive(cards []*examples.Card) *handRank {
	if len(cards) != 5 {
		return &handRank{rankType: examples.HandRankType_HIGH_CARD, score: 0}
	}

	rankCounts := make(map[int32]int)
	suitCounts := make(map[int32]int)
	ranks := make([]int32, 5)

	for i, card := range cards {
		rankCounts[int32(card.Rank)]++
		suitCounts[int32(card.Suit)]++
		ranks[i] = int32(card.Rank)
	}

	sort.Slice(ranks, func(i, j int) bool { return ranks[i] > ranks[j] })

	isFlush := false
	for _, count := range suitCounts {
		if count == 5 {
			isFlush = true
			break
		}
	}

	isStraight := checkStraight(ranks)
	isWheel := checkWheel(ranks)

	type rankCount struct {
		rank  int32
		count int
	}
	var counts []rankCount
	for rank, count := range rankCounts {
		counts = append(counts, rankCount{rank, count})
	}
	sort.Slice(counts, func(i, j int) bool {
		if counts[i].count != counts[j].count {
			return counts[i].count > counts[j].count
		}
		return counts[i].rank > counts[j].rank
	})

	countPattern := make([]int, len(counts))
	for i, c := range counts {
		countPattern[i] = c.count
	}

	if isFlush && (isStraight || isWheel) {
		high := ranks[0]
		if isWheel {
			high = 5
		}
		if high == int32(examples.Rank_ACE) && !isWheel {
			return &handRank{
				rankType: examples.HandRankType_ROYAL_FLUSH,
				score:    10_000_000,
			}
		}
		return &handRank{
			rankType: examples.HandRankType_STRAIGHT_FLUSH,
			score:    9_000_000 + high,
		}
	}

	if len(countPattern) >= 2 && countPattern[0] == 4 && countPattern[1] == 1 {
		quadRank := counts[0].rank
		kicker := counts[1].rank
		return &handRank{
			rankType: examples.HandRankType_FOUR_OF_A_KIND,
			score:    8_000_000 + quadRank*100 + kicker,
		}
	}

	if len(countPattern) >= 2 && countPattern[0] == 3 && countPattern[1] == 2 {
		tripsRank := counts[0].rank
		pairRank := counts[1].rank
		return &handRank{
			rankType: examples.HandRankType_FULL_HOUSE,
			score:    7_000_000 + tripsRank*100 + pairRank,
		}
	}

	if isFlush {
		score := rankScore(ranks)
		return &handRank{
			rankType: examples.HandRankType_FLUSH,
			score:    6_000_000 + score,
		}
	}

	if isStraight || isWheel {
		high := ranks[0]
		if isWheel {
			high = 5
		}
		return &handRank{
			rankType: examples.HandRankType_STRAIGHT,
			score:    5_000_000 + high,
		}
	}

	if len(countPattern) >= 3 && countPattern[0] == 3 && countPattern[1] == 1 && countPattern[2] == 1 {
		tripsRank := counts[0].rank
		return &handRank{
			rankType: examples.HandRankType_THREE_OF_A_KIND,
			score:    4_000_000 + tripsRank*1000,
		}
	}

	if len(countPattern) >= 3 && countPattern[0] == 2 && countPattern[1] == 2 && countPattern[2] == 1 {
		highPair := counts[0].rank
		lowPair := counts[1].rank
		kicker := counts[2].rank
		return &handRank{
			rankType: examples.HandRankType_TWO_PAIR,
			score:    3_000_000 + highPair*1000 + lowPair*50 + kicker,
		}
	}

	if len(countPattern) >= 4 && countPattern[0] == 2 {
		pairRank := counts[0].rank
		return &handRank{
			rankType: examples.HandRankType_PAIR,
			score:    2_000_000 + pairRank*10000,
		}
	}

	score := rankScore(ranks)
	return &handRank{
		rankType: examples.HandRankType_HIGH_CARD,
		score:    1_000_000 + score,
	}
}

func checkStraight(ranks []int32) bool {
	if len(ranks) != 5 {
		return false
	}
	for i := 0; i < 4; i++ {
		if ranks[i]-ranks[i+1] != 1 {
			return false
		}
	}
	return true
}

func checkWheel(ranks []int32) bool {
	sorted := make([]int32, len(ranks))
	copy(sorted, ranks)
	sort.Slice(sorted, func(i, j int) bool { return sorted[i] < sorted[j] })

	return len(sorted) == 5 &&
		sorted[0] == 2 &&
		sorted[1] == 3 &&
		sorted[2] == 4 &&
		sorted[3] == 5 &&
		sorted[4] == int32(examples.Rank_ACE)
}

func rankScore(ranks []int32) int32 {
	var score int32
	multiplier := int32(10000)
	for i := 0; i < 5 && i < len(ranks); i++ {
		score += ranks[i] * multiplier
		multiplier /= 15
	}
	return score
}

func combinations(cards []*examples.Card, k int) [][]*examples.Card {
	if k > len(cards) {
		return nil
	}
	if k == 0 {
		return [][]*examples.Card{{}}
	}
	if k == len(cards) {
		result := make([]*examples.Card, len(cards))
		copy(result, cards)
		return [][]*examples.Card{result}
	}

	var result [][]*examples.Card
	var helper func(start int, current []*examples.Card)
	helper = func(start int, current []*examples.Card) {
		if len(current) == k {
			combo := make([]*examples.Card, k)
			copy(combo, current)
			result = append(result, combo)
			return
		}
		for i := start; i < len(cards); i++ {
			helper(i+1, append(current, cards[i]))
		}
	}
	helper(0, nil)
	return result
}
