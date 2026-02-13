// Package handlers provides poker game rules and hand evaluation.
package handlers

import (
	"sort"

	"github.com/benjaminabbitt/angzarr/client/go/proto/examples"
)

// HandRank represents a hand ranking result with score for comparison.
type HandRank struct {
	RankType examples.HandRankType
	Score    int32
	Kickers  []examples.Rank
}

// GameRules defines variant-specific poker logic.
type GameRules interface {
	// HoleCardCount returns the number of hole cards dealt to each player.
	HoleCardCount() int

	// EvaluateHand evaluates a player's hand against community cards.
	EvaluateHand(holeCards, communityCards []*examples.Card) *HandRank

	// UsesCommunityCards returns true if the variant uses community cards.
	UsesCommunityCards() bool
}

// GetRules returns the game rules for a variant.
func GetRules(variant examples.GameVariant) GameRules {
	switch variant {
	case examples.GameVariant_TEXAS_HOLDEM:
		return &TexasHoldemRules{}
	case examples.GameVariant_OMAHA:
		return &OmahaRules{}
	case examples.GameVariant_FIVE_CARD_DRAW:
		return &FiveCardDrawRules{}
	default:
		return &TexasHoldemRules{}
	}
}

// TexasHoldemRules implements Texas Hold'em rules.
type TexasHoldemRules struct{}

func (r *TexasHoldemRules) HoleCardCount() int { return 2 }

func (r *TexasHoldemRules) EvaluateHand(holeCards, communityCards []*examples.Card) *HandRank {
	allCards := append(holeCards, communityCards...)
	if len(allCards) < 5 {
		return &HandRank{RankType: examples.HandRankType_HIGH_CARD, Score: 0}
	}
	return findBestFive(allCards)
}

func (r *TexasHoldemRules) UsesCommunityCards() bool { return true }

// OmahaRules implements Omaha rules (must use exactly 2 hole + 3 community).
type OmahaRules struct{}

func (r *OmahaRules) HoleCardCount() int { return 4 }

func (r *OmahaRules) EvaluateHand(holeCards, communityCards []*examples.Card) *HandRank {
	if len(holeCards) < 2 || len(communityCards) < 3 {
		return &HandRank{RankType: examples.HandRankType_HIGH_CARD, Score: 0}
	}

	var bestRank *HandRank

	// Must use exactly 2 from hole, 3 from community
	holeCombos := combinations(holeCards, 2)
	commCombos := combinations(communityCards, 3)

	for _, holeCombo := range holeCombos {
		for _, commCombo := range commCombos {
			five := append(holeCombo, commCombo...)
			rank := evaluateFive(five)
			if bestRank == nil || rank.Score > bestRank.Score {
				bestRank = rank
			}
		}
	}

	if bestRank == nil {
		return &HandRank{RankType: examples.HandRankType_HIGH_CARD, Score: 0}
	}
	return bestRank
}

func (r *OmahaRules) UsesCommunityCards() bool { return true }

// FiveCardDrawRules implements Five Card Draw rules.
type FiveCardDrawRules struct{}

func (r *FiveCardDrawRules) HoleCardCount() int { return 5 }

func (r *FiveCardDrawRules) EvaluateHand(holeCards, _ []*examples.Card) *HandRank {
	if len(holeCards) < 5 {
		return &HandRank{RankType: examples.HandRankType_HIGH_CARD, Score: 0}
	}
	return evaluateFive(holeCards[:5])
}

func (r *FiveCardDrawRules) UsesCommunityCards() bool { return false }

// findBestFive finds the best 5-card hand from any number of cards.
func findBestFive(cards []*examples.Card) *HandRank {
	if len(cards) < 5 {
		return &HandRank{RankType: examples.HandRankType_HIGH_CARD, Score: 0}
	}

	var bestRank *HandRank

	combos := combinations(cards, 5)
	for _, combo := range combos {
		rank := evaluateFive(combo)
		if bestRank == nil || rank.Score > bestRank.Score {
			bestRank = rank
		}
	}

	if bestRank == nil {
		return &HandRank{RankType: examples.HandRankType_HIGH_CARD, Score: 0}
	}
	return bestRank
}

// evaluateFive evaluates exactly 5 cards.
func evaluateFive(cards []*examples.Card) *HandRank {
	if len(cards) != 5 {
		return &HandRank{RankType: examples.HandRankType_HIGH_CARD, Score: 0}
	}

	// Count ranks and suits
	rankCounts := make(map[int32]int)
	suitCounts := make(map[int32]int)
	ranks := make([]int32, 5)

	for i, card := range cards {
		rankCounts[int32(card.Rank)]++
		suitCounts[int32(card.Suit)]++
		ranks[i] = int32(card.Rank)
	}

	// Sort ranks descending
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

	// Build count pattern for pair/trip/quad detection
	type rankCount struct {
		rank  int32
		count int
	}
	var counts []rankCount
	for rank, count := range rankCounts {
		counts = append(counts, rankCount{rank, count})
	}
	// Sort by count desc, then by rank desc
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

	// Determine hand type and score
	if isFlush && (isStraight || isWheel) {
		high := ranks[0]
		if isWheel {
			high = 5 // Wheel straight flush high is 5
		}
		if high == int32(examples.Rank_ACE) && !isWheel {
			// Royal flush
			return &HandRank{
				RankType: examples.HandRankType_ROYAL_FLUSH,
				Score:    10_000_000,
				Kickers:  nil,
			}
		}
		return &HandRank{
			RankType: examples.HandRankType_STRAIGHT_FLUSH,
			Score:    9_000_000 + high,
			Kickers:  []examples.Rank{examples.Rank(high)},
		}
	}

	if len(countPattern) >= 2 && countPattern[0] == 4 && countPattern[1] == 1 {
		quadRank := counts[0].rank
		kicker := counts[1].rank
		return &HandRank{
			RankType: examples.HandRankType_FOUR_OF_A_KIND,
			Score:    8_000_000 + quadRank*100 + kicker,
			Kickers:  []examples.Rank{examples.Rank(quadRank), examples.Rank(kicker)},
		}
	}

	if len(countPattern) >= 2 && countPattern[0] == 3 && countPattern[1] == 2 {
		tripsRank := counts[0].rank
		pairRank := counts[1].rank
		return &HandRank{
			RankType: examples.HandRankType_FULL_HOUSE,
			Score:    7_000_000 + tripsRank*100 + pairRank,
			Kickers:  []examples.Rank{examples.Rank(tripsRank), examples.Rank(pairRank)},
		}
	}

	if isFlush {
		score := rankScore(ranks)
		kickers := make([]examples.Rank, len(ranks))
		for i, r := range ranks {
			kickers[i] = examples.Rank(r)
		}
		return &HandRank{
			RankType: examples.HandRankType_FLUSH,
			Score:    6_000_000 + score,
			Kickers:  kickers,
		}
	}

	if isStraight || isWheel {
		high := ranks[0]
		if isWheel {
			high = 5
		}
		return &HandRank{
			RankType: examples.HandRankType_STRAIGHT,
			Score:    5_000_000 + high,
			Kickers:  []examples.Rank{examples.Rank(high)},
		}
	}

	if len(countPattern) >= 3 && countPattern[0] == 3 && countPattern[1] == 1 && countPattern[2] == 1 {
		tripsRank := counts[0].rank
		kicker1 := counts[1].rank
		kicker2 := counts[2].rank
		kickerScore := kicker1*50 + kicker2
		return &HandRank{
			RankType: examples.HandRankType_THREE_OF_A_KIND,
			Score:    4_000_000 + tripsRank*1000 + kickerScore,
			Kickers:  []examples.Rank{examples.Rank(tripsRank), examples.Rank(kicker1), examples.Rank(kicker2)},
		}
	}

	if len(countPattern) >= 3 && countPattern[0] == 2 && countPattern[1] == 2 && countPattern[2] == 1 {
		highPair := counts[0].rank
		lowPair := counts[1].rank
		kicker := counts[2].rank
		return &HandRank{
			RankType: examples.HandRankType_TWO_PAIR,
			Score:    3_000_000 + highPair*1000 + lowPair*50 + kicker,
			Kickers:  []examples.Rank{examples.Rank(highPair), examples.Rank(lowPair), examples.Rank(kicker)},
		}
	}

	if len(countPattern) >= 4 && countPattern[0] == 2 && countPattern[1] == 1 && countPattern[2] == 1 && countPattern[3] == 1 {
		pairRank := counts[0].rank
		kicker1 := counts[1].rank
		kicker2 := counts[2].rank
		kicker3 := counts[3].rank
		kickerScore := kicker1*1000 + kicker2*50 + kicker3
		return &HandRank{
			RankType: examples.HandRankType_PAIR,
			Score:    2_000_000 + pairRank*10000 + kickerScore,
			Kickers:  []examples.Rank{examples.Rank(pairRank), examples.Rank(kicker1), examples.Rank(kicker2), examples.Rank(kicker3)},
		}
	}

	// High card
	score := rankScore(ranks)
	kickers := make([]examples.Rank, len(ranks))
	for i, r := range ranks {
		kickers[i] = examples.Rank(r)
	}
	return &HandRank{
		RankType: examples.HandRankType_HIGH_CARD,
		Score:    1_000_000 + score,
		Kickers:  kickers,
	}
}

// checkStraight checks if sorted ranks (descending) form a straight.
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

// checkWheel checks for wheel straight (A-2-3-4-5).
func checkWheel(ranks []int32) bool {
	sorted := make([]int32, len(ranks))
	copy(sorted, ranks)
	sort.Slice(sorted, func(i, j int) bool { return sorted[i] < sorted[j] })

	// A-2-3-4-5 means sorted is [2, 3, 4, 5, 14]
	return len(sorted) == 5 &&
		sorted[0] == 2 &&
		sorted[1] == 3 &&
		sorted[2] == 4 &&
		sorted[3] == 5 &&
		sorted[4] == int32(examples.Rank_ACE)
}

// rankScore calculates rank-based score for high card comparison.
func rankScore(ranks []int32) int32 {
	var score int32
	multiplier := int32(10000)
	for i := 0; i < 5 && i < len(ranks); i++ {
		score += ranks[i] * multiplier
		multiplier /= 15 // Ensure no overlap
	}
	return score
}

// combinations generates all k-combinations of cards.
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
