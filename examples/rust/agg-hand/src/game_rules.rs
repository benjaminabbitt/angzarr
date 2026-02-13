//! Poker game rules and hand evaluation.
//!
//! Polymorphic game rules supporting multiple poker variants.

use angzarr_client::proto::examples::{BettingPhase, Card, GameVariant, HandRankType, Rank};
use itertools::Itertools;
use std::collections::HashMap;

/// Hand ranking result with score for comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandRank {
    pub rank_type: HandRankType,
    pub score: i32,
    pub kickers: Vec<Rank>,
}

impl HandRank {
    fn new(rank_type: HandRankType, score: i32, kickers: Vec<i32>) -> Self {
        Self {
            rank_type,
            score,
            kickers: kickers
                .into_iter()
                .filter_map(|r| Rank::try_from(r).ok())
                .collect(),
        }
    }
}

/// Phase transition result.
pub struct PhaseTransition {
    pub next_phase: BettingPhase,
    pub community_cards_to_deal: usize,
}

/// Game rules trait for variant-specific logic.
pub trait GameRules: Send + Sync {
    /// Number of hole cards dealt to each player.
    fn hole_card_count(&self) -> usize;

    /// Evaluate player's hand against community cards.
    fn evaluate_hand(&self, hole_cards: &[Card], community_cards: &[Card]) -> HandRank;

    /// Get next phase transition.
    fn get_next_phase(&self, current_phase: BettingPhase) -> Option<PhaseTransition>;

    /// Check if variant uses community cards.
    fn uses_community_cards(&self) -> bool;
}

/// Get game rules for a variant.
pub fn get_rules(variant: GameVariant) -> Box<dyn GameRules> {
    match variant {
        GameVariant::TexasHoldem => Box::new(TexasHoldemRules),
        GameVariant::Omaha => Box::new(OmahaRules),
        GameVariant::FiveCardDraw => Box::new(FiveCardDrawRules),
        _ => Box::new(TexasHoldemRules), // Default
    }
}

/// Texas Hold'em rules.
pub struct TexasHoldemRules;

impl GameRules for TexasHoldemRules {
    fn hole_card_count(&self) -> usize {
        2
    }

    fn evaluate_hand(&self, hole_cards: &[Card], community_cards: &[Card]) -> HandRank {
        let all_cards: Vec<_> = hole_cards.iter().chain(community_cards.iter()).collect();
        if all_cards.len() < 5 {
            return HandRank::new(HandRankType::HighCard, 0, vec![]);
        }
        // Find best 5-card hand from all available cards
        find_best_five(&all_cards)
    }

    fn get_next_phase(&self, current_phase: BettingPhase) -> Option<PhaseTransition> {
        match current_phase {
            BettingPhase::Preflop => Some(PhaseTransition {
                next_phase: BettingPhase::Flop,
                community_cards_to_deal: 3,
            }),
            BettingPhase::Flop => Some(PhaseTransition {
                next_phase: BettingPhase::Turn,
                community_cards_to_deal: 1,
            }),
            BettingPhase::Turn => Some(PhaseTransition {
                next_phase: BettingPhase::River,
                community_cards_to_deal: 1,
            }),
            BettingPhase::River => Some(PhaseTransition {
                next_phase: BettingPhase::Showdown,
                community_cards_to_deal: 0,
            }),
            _ => None,
        }
    }

    fn uses_community_cards(&self) -> bool {
        true
    }
}

/// Omaha rules - must use exactly 2 hole cards + 3 community cards.
pub struct OmahaRules;

impl GameRules for OmahaRules {
    fn hole_card_count(&self) -> usize {
        4
    }

    fn evaluate_hand(&self, hole_cards: &[Card], community_cards: &[Card]) -> HandRank {
        if hole_cards.len() < 2 || community_cards.len() < 3 {
            return HandRank::new(HandRankType::HighCard, 0, vec![]);
        }

        // Must use exactly 2 from hole, 3 from community
        let mut best_rank: Option<HandRank> = None;

        for hole_combo in hole_cards.iter().combinations(2) {
            for comm_combo in community_cards.iter().combinations(3) {
                let five: Vec<_> = hole_combo
                    .iter()
                    .chain(comm_combo.iter())
                    .copied()
                    .collect();
                let rank = evaluate_five(&five);

                if best_rank.as_ref().is_none_or(|best| rank.score > best.score) {
                    best_rank = Some(rank);
                }
            }
        }

        best_rank.unwrap_or_else(|| HandRank::new(HandRankType::HighCard, 0, vec![]))
    }

    fn get_next_phase(&self, current_phase: BettingPhase) -> Option<PhaseTransition> {
        // Same as Texas Hold'em
        TexasHoldemRules.get_next_phase(current_phase)
    }

    fn uses_community_cards(&self) -> bool {
        true
    }
}

/// Five Card Draw rules.
pub struct FiveCardDrawRules;

impl GameRules for FiveCardDrawRules {
    fn hole_card_count(&self) -> usize {
        5
    }

    fn evaluate_hand(&self, hole_cards: &[Card], _community_cards: &[Card]) -> HandRank {
        if hole_cards.len() < 5 {
            return HandRank::new(HandRankType::HighCard, 0, vec![]);
        }
        let cards: Vec<_> = hole_cards.iter().collect();
        evaluate_five(&cards)
    }

    fn get_next_phase(&self, current_phase: BettingPhase) -> Option<PhaseTransition> {
        match current_phase {
            BettingPhase::Preflop => Some(PhaseTransition {
                next_phase: BettingPhase::Draw,
                community_cards_to_deal: 0,
            }),
            BettingPhase::Draw => Some(PhaseTransition {
                next_phase: BettingPhase::Showdown,
                community_cards_to_deal: 0,
            }),
            _ => None,
        }
    }

    fn uses_community_cards(&self) -> bool {
        false
    }
}

/// Find the best 5-card hand from any number of cards.
fn find_best_five(cards: &[&Card]) -> HandRank {
    if cards.len() < 5 {
        return HandRank::new(HandRankType::HighCard, 0, vec![]);
    }

    cards
        .iter()
        .combinations(5)
        .map(|combo| {
            let five: Vec<_> = combo.into_iter().copied().collect();
            evaluate_five(&five)
        })
        .max_by_key(|r| r.score)
        .unwrap_or_else(|| HandRank::new(HandRankType::HighCard, 0, vec![]))
}

/// Evaluate exactly 5 cards.
fn evaluate_five(cards: &[&Card]) -> HandRank {
    if cards.len() != 5 {
        return HandRank::new(HandRankType::HighCard, 0, vec![]);
    }

    // Count ranks and suits
    let mut rank_counts: HashMap<i32, i32> = HashMap::new();
    let mut suit_counts: HashMap<i32, i32> = HashMap::new();
    let mut ranks: Vec<i32> = Vec::with_capacity(5);

    for card in cards {
        *rank_counts.entry(card.rank).or_insert(0) += 1;
        *suit_counts.entry(card.suit).or_insert(0) += 1;
        ranks.push(card.rank);
    }

    ranks.sort_by(|a, b| b.cmp(a)); // Descending

    let is_flush = suit_counts.values().any(|&c| c == 5);
    let is_straight = check_straight(&ranks);
    let is_wheel = check_wheel(&ranks); // A-2-3-4-5

    // Count pairs, trips, quads
    let mut counts: Vec<(i32, i32)> = rank_counts.into_iter().collect();
    counts.sort_by(|a, b| {
        // Sort by count desc, then by rank desc
        b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0))
    });

    let count_pattern: Vec<i32> = counts.iter().map(|(_, c)| *c).collect();

    // Determine hand type and score
    if is_flush && is_straight {
        let high = if is_wheel { 5 } else { ranks[0] }; // Wheel straight flush high is 5
        if high == Rank::Ace as i32 && !is_wheel {
            // Royal flush
            return HandRank::new(HandRankType::RoyalFlush, 10_000_000, vec![]);
        }
        return HandRank::new(
            HandRankType::StraightFlush,
            9_000_000 + high,
            vec![high],
        );
    }

    if count_pattern == [4, 1] {
        let quad_rank = counts[0].0;
        let kicker = counts[1].0;
        return HandRank::new(
            HandRankType::FourOfAKind,
            8_000_000 + quad_rank * 100 + kicker,
            vec![quad_rank, kicker],
        );
    }

    if count_pattern == [3, 2] {
        let trips_rank = counts[0].0;
        let pair_rank = counts[1].0;
        return HandRank::new(
            HandRankType::FullHouse,
            7_000_000 + trips_rank * 100 + pair_rank,
            vec![trips_rank, pair_rank],
        );
    }

    if is_flush {
        let score = rank_score(&ranks);
        return HandRank::new(HandRankType::Flush, 6_000_000 + score, ranks);
    }

    if is_straight || is_wheel {
        let high = if is_wheel { 5 } else { ranks[0] };
        return HandRank::new(HandRankType::Straight, 5_000_000 + high, vec![high]);
    }

    if count_pattern == [3, 1, 1] {
        let trips_rank = counts[0].0;
        let kickers: Vec<i32> = counts[1..].iter().map(|(r, _)| *r).collect();
        let kicker_score = kickers[0] * 100 + kickers.get(1).unwrap_or(&0);
        return HandRank::new(
            HandRankType::ThreeOfAKind,
            4_000_000 + trips_rank * 10000 + kicker_score,
            std::iter::once(trips_rank).chain(kickers).collect(),
        );
    }

    if count_pattern == [2, 2, 1] {
        let high_pair = counts[0].0;
        let low_pair = counts[1].0;
        let kicker = counts[2].0;
        return HandRank::new(
            HandRankType::TwoPair,
            3_000_000 + high_pair * 10000 + low_pair * 100 + kicker,
            vec![high_pair, low_pair, kicker],
        );
    }

    if count_pattern == [2, 1, 1, 1] {
        let pair_rank = counts[0].0;
        let kickers: Vec<i32> = counts[1..].iter().map(|(r, _)| *r).collect();
        let kicker_score =
            kickers[0] * 10000 + kickers.get(1).unwrap_or(&0) * 100 + kickers.get(2).unwrap_or(&0);
        return HandRank::new(
            HandRankType::Pair,
            2_000_000 + pair_rank * 100_000 + kicker_score,
            std::iter::once(pair_rank).chain(kickers).collect(),
        );
    }

    // High card
    let score = rank_score(&ranks);
    HandRank::new(HandRankType::HighCard, 1_000_000 + score, ranks)
}

/// Check if sorted ranks form a straight.
fn check_straight(ranks: &[i32]) -> bool {
    if ranks.len() != 5 {
        return false;
    }
    // Ranks are sorted descending
    for i in 0..4 {
        if ranks[i] - ranks[i + 1] != 1 {
            return false;
        }
    }
    true
}

/// Check for wheel (A-2-3-4-5).
fn check_wheel(ranks: &[i32]) -> bool {
    let mut sorted = ranks.to_vec();
    sorted.sort();
    sorted == vec![2, 3, 4, 5, Rank::Ace as i32]
}

/// Calculate rank-based score for high card comparison.
fn rank_score(ranks: &[i32]) -> i32 {
    // Each rank position is worth progressively less
    let mut score = 0i32;
    let mut multiplier = 10000i32;
    for &rank in ranks.iter().take(5) {
        score += rank * multiplier;
        multiplier /= 15; // Ensure no overlap
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use angzarr_client::proto::examples::Suit;

    fn card(rank: Rank, suit: Suit) -> Card {
        Card {
            rank: rank as i32,
            suit: suit as i32,
        }
    }

    #[test]
    fn test_royal_flush() {
        let cards = vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::King, Suit::Spades),
            card(Rank::Queen, Suit::Spades),
            card(Rank::Jack, Suit::Spades),
            card(Rank::Ten, Suit::Spades),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::RoyalFlush);
        assert_eq!(rank.score, 10_000_000);
    }

    #[test]
    fn test_straight_flush() {
        let cards = vec![
            card(Rank::Nine, Suit::Hearts),
            card(Rank::Eight, Suit::Hearts),
            card(Rank::Seven, Suit::Hearts),
            card(Rank::Six, Suit::Hearts),
            card(Rank::Five, Suit::Hearts),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::StraightFlush);
    }

    #[test]
    fn test_wheel_straight() {
        let cards = vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::Two, Suit::Hearts),
            card(Rank::Three, Suit::Clubs),
            card(Rank::Four, Suit::Diamonds),
            card(Rank::Five, Suit::Spades),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::Straight);
        assert_eq!(rank.kickers, vec![Rank::Five]); // Wheel high is 5
    }

    #[test]
    fn test_four_of_kind() {
        let cards = vec![
            card(Rank::King, Suit::Spades),
            card(Rank::King, Suit::Hearts),
            card(Rank::King, Suit::Clubs),
            card(Rank::King, Suit::Diamonds),
            card(Rank::Two, Suit::Spades),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::FourOfAKind);
    }

    #[test]
    fn test_full_house() {
        let cards = vec![
            card(Rank::Queen, Suit::Spades),
            card(Rank::Queen, Suit::Hearts),
            card(Rank::Queen, Suit::Clubs),
            card(Rank::Jack, Suit::Diamonds),
            card(Rank::Jack, Suit::Spades),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::FullHouse);
    }

    #[test]
    fn test_flush() {
        let cards = vec![
            card(Rank::Ace, Suit::Clubs),
            card(Rank::Ten, Suit::Clubs),
            card(Rank::Seven, Suit::Clubs),
            card(Rank::Four, Suit::Clubs),
            card(Rank::Two, Suit::Clubs),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::Flush);
    }

    #[test]
    fn test_straight() {
        let cards = vec![
            card(Rank::Ten, Suit::Spades),
            card(Rank::Nine, Suit::Hearts),
            card(Rank::Eight, Suit::Clubs),
            card(Rank::Seven, Suit::Diamonds),
            card(Rank::Six, Suit::Spades),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::Straight);
    }

    #[test]
    fn test_three_of_kind() {
        let cards = vec![
            card(Rank::Jack, Suit::Spades),
            card(Rank::Jack, Suit::Hearts),
            card(Rank::Jack, Suit::Clubs),
            card(Rank::Seven, Suit::Diamonds),
            card(Rank::Two, Suit::Spades),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::ThreeOfAKind);
    }

    #[test]
    fn test_two_pair() {
        let cards = vec![
            card(Rank::Ten, Suit::Spades),
            card(Rank::Ten, Suit::Hearts),
            card(Rank::Five, Suit::Clubs),
            card(Rank::Five, Suit::Diamonds),
            card(Rank::Two, Suit::Spades),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::TwoPair);
    }

    #[test]
    fn test_pair() {
        let cards = vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::Ace, Suit::Hearts),
            card(Rank::King, Suit::Clubs),
            card(Rank::Seven, Suit::Diamonds),
            card(Rank::Two, Suit::Spades),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::Pair);
    }

    #[test]
    fn test_high_card() {
        let cards = vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::King, Suit::Hearts),
            card(Rank::Ten, Suit::Clubs),
            card(Rank::Seven, Suit::Diamonds),
            card(Rank::Two, Suit::Clubs),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = evaluate_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::HighCard);
    }

    #[test]
    fn test_best_from_seven() {
        // Player has pair of aces in hole, board has another ace
        let cards = vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::Ace, Suit::Hearts),
            card(Rank::Ace, Suit::Clubs),
            card(Rank::King, Suit::Diamonds),
            card(Rank::Queen, Suit::Spades),
            card(Rank::Jack, Suit::Hearts),
            card(Rank::Two, Suit::Clubs),
        ];
        let refs: Vec<_> = cards.iter().collect();
        let rank = find_best_five(&refs);
        assert_eq!(rank.rank_type, HandRankType::ThreeOfAKind);
    }

    #[test]
    fn test_omaha_evaluation() {
        let rules = OmahaRules;
        let hole = vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::Ace, Suit::Hearts),
            card(Rank::King, Suit::Clubs),
            card(Rank::King, Suit::Diamonds),
        ];
        // With only 1 ace and 1 king in community, best is three aces (2 hole + 1 comm ace)
        let community = vec![
            card(Rank::Ace, Suit::Clubs),
            card(Rank::King, Suit::Spades),
            card(Rank::Two, Suit::Hearts),
        ];
        let rank = rules.evaluate_hand(&hole, &community);
        // Must use exactly 2 hole + 3 community: AA (hole) + AK2 (comm) = Three Aces
        assert_eq!(rank.rank_type, HandRankType::ThreeOfAKind);
    }

    #[test]
    fn test_omaha_full_house() {
        let rules = OmahaRules;
        // Need: 2 hole + 3 community to make a full house
        // Full house needs 3-of-kind + pair
        let hole = vec![
            card(Rank::Ace, Suit::Spades),
            card(Rank::Ace, Suit::Hearts),
            card(Rank::Queen, Suit::Clubs),
            card(Rank::Jack, Suit::Diamonds),
        ];
        // Community has one more ace and two kings
        // Best: AA (hole) + AKK (comm) = AAA-KK full house
        let community = vec![
            card(Rank::Ace, Suit::Clubs),
            card(Rank::King, Suit::Spades),
            card(Rank::King, Suit::Hearts),
        ];
        let rank = rules.evaluate_hand(&hole, &community);
        // AAA-KK full house (2 aces from hole, 1 ace + 2 kings from community)
        assert_eq!(rank.rank_type, HandRankType::FullHouse);
    }
}
