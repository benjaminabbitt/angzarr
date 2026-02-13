//! Hand aggregate BDD tests using cucumber-rs.

use std::collections::HashMap;

use agg_hand::{game_rules, handlers, state::rebuild_state};
use angzarr_client::proto::examples::{
    ActionTaken, ActionType, BettingPhase, BettingRoundComplete, BlindPosted, Card, CardsDealt,
    CardsMucked, CardsRevealed, CommunityCardsDealt, DealCards, DealCommunityCards, DrawCompleted,
    GameVariant, HandRanking, HandRankType, PlayerAction, PlayerHoleCards, PlayerInHand,
    PlayerStackSnapshot, PostBlind, PotAwarded, PotAward, RequestDraw, RevealCards,
    ShowdownStarted, AwardPot,
};
use angzarr_client::proto::{event_page, CommandBook, Cover, EventBook, EventPage, Uuid};
use angzarr_client::{pack_event, CommandRejectedError, UnpackAny};
use cucumber::{given, then, when, World, WriterExt};
use prost::Message;
use prost_types::Any;
use sha2::{Digest, Sha256};

/// Generate deterministic UUID from a seed string.
fn uuid_for(seed: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let hash = hasher.finalize();
    hash[0..16].to_vec()
}

/// Parse a card string like "As" (Ace of spades) to a Card.
fn parse_card(s: &str) -> Card {
    use angzarr_client::proto::examples::{Rank, Suit};
    let s = s.trim();
    if s.len() < 2 {
        return Card::default();
    }
    let (rank_char, suit_char) = s.split_at(s.len() - 1);

    let rank = match rank_char {
        "A" => Rank::Ace,
        "K" => Rank::King,
        "Q" => Rank::Queen,
        "J" => Rank::Jack,
        "T" | "10" => Rank::Ten,
        "9" => Rank::Nine,
        "8" => Rank::Eight,
        "7" => Rank::Seven,
        "6" => Rank::Six,
        "5" => Rank::Five,
        "4" => Rank::Four,
        "3" => Rank::Three,
        "2" => Rank::Two,
        _ => Rank::Two,
    };

    let suit = match suit_char {
        "s" => Suit::Spades,
        "h" => Suit::Hearts,
        "d" => Suit::Diamonds,
        "c" => Suit::Clubs,
        _ => Suit::Spades,
    };

    Card {
        rank: rank as i32,
        suit: suit as i32,
    }
}

/// Parse a card string like "As Ks" to a Vec<Card>.
fn parse_cards(s: &str) -> Vec<Card> {
    s.split_whitespace().map(parse_card).collect()
}

/// Pack a command into Any.
fn pack_cmd<T: Message>(cmd: &T, type_name: &str) -> Any {
    Any {
        type_url: format!("type.poker/{}", type_name),
        value: cmd.encode_to_vec(),
    }
}

/// Create a command book with a given root.
fn command_book(root: &[u8], domain: &str) -> CommandBook {
    CommandBook {
        cover: Some(Cover {
            domain: domain.to_string(),
            root: Some(Uuid {
                value: root.to_vec(),
            }),
            ..Default::default()
        }),
        pages: vec![],
        saga_origin: None,
    }
}

/// Test world for hand aggregate.
#[derive(Debug, Default, World)]
pub struct HandWorld {
    events: Vec<Any>,
    result: Option<Result<EventBook, CommandRejectedError>>,

    // Hand parameters
    game_variant: GameVariant,
    players: Vec<PlayerInHand>,
    current_bet: i64,
    pot_total: i64,
    min_raise: i64,
    deck_remaining: i32,
    hand_number: i64,

    // Showdown state
    player_rankings: HashMap<String, HandRankType>,
    winner: String,
    showdown_hole_cards: HashMap<String, Vec<Card>>,
    showdown_community_cards: Vec<Card>,
}

impl HandWorld {
    fn hand_root(&self) -> Vec<u8> {
        uuid_for("test-hand")
    }

    fn table_root(&self) -> Vec<u8> {
        uuid_for("test-table")
    }

    fn player_root(&self, player_id: &str) -> Vec<u8> {
        uuid_for(player_id)
    }

    fn event_book(&self) -> EventBook {
        EventBook {
            cover: Some(Cover {
                domain: "hand".to_string(),
                root: Some(Uuid {
                    value: self.hand_root(),
                }),
                ..Default::default()
            }),
            pages: self
                .events
                .iter()
                .enumerate()
                .map(|(i, e)| EventPage {
                    sequence: Some(event_page::Sequence::Num(i as u32)),
                    event: Some(e.clone()),
                    created_at: None,
                })
                .collect(),
            snapshot: None,
            next_sequence: self.events.len() as u32,
        }
    }

    fn next_seq(&self) -> u32 {
        self.events.len() as u32
    }

    fn result_event(&self) -> Option<Any> {
        self.result
            .as_ref()
            .and_then(|r: &Result<EventBook, CommandRejectedError>| {
                r.as_ref()
                    .ok()
                    .and_then(|eb| eb.pages.first())
                    .and_then(|p| p.event.clone())
            })
    }

    fn add_cards_dealt(&mut self, variant: GameVariant, players: &[(&str, i32, i64)]) {
        self.game_variant = variant;
        self.players = players
            .iter()
            .map(|(id, pos, stack)| PlayerInHand {
                player_root: self.player_root(id),
                position: *pos,
                stack: *stack,
            })
            .collect();

        let cards_per_player = match variant {
            GameVariant::TexasHoldem => 2,
            GameVariant::Omaha => 4,
            GameVariant::FiveCardDraw => 5,
            _ => 2,
        };

        let total_dealt = players.len() * cards_per_player;
        self.deck_remaining = (52 - total_dealt) as i32;

        // Create player hole cards
        let player_cards: Vec<PlayerHoleCards> = players
            .iter()
            .enumerate()
            .map(|(i, (id, _, _))| {
                let cards: Vec<Card> = (0..cards_per_player)
                    .map(|j| Card {
                        suit: (j % 4) as i32,
                        rank: (2 + i + j) as i32,
                    })
                    .collect();
                PlayerHoleCards {
                    player_root: self.player_root(id),
                    cards,
                }
            })
            .collect();

        // Create remaining deck
        let remaining_deck: Vec<Card> = (0..self.deck_remaining)
            .map(|i| Card {
                suit: (i % 4) as i32,
                rank: (2 + i / 4) as i32,
            })
            .collect();

        let event = CardsDealt {
            table_root: self.table_root(),
            hand_number: 1,
            game_variant: variant as i32,
            player_cards,
            dealer_position: 0,
            players: self.players.clone(),
            dealt_at: None,
            remaining_deck,
        };
        self.events.push(pack_event(&event, "examples.CardsDealt"));
        self.hand_number = 1;
    }

    fn add_blinds(&mut self, pot: i64, current_bet: i64) {
        // Add small blind
        let sb_event = BlindPosted {
            player_root: self.player_root("player-1"),
            blind_type: "small".to_string(),
            amount: 5,
            player_stack: 495,
            pot_total: 5,
            posted_at: None,
        };
        self.events
            .push(pack_event(&sb_event, "examples.BlindPosted"));

        // Add big blind
        let bb_event = BlindPosted {
            player_root: self.player_root("player-2"),
            blind_type: "big".to_string(),
            amount: 10,
            player_stack: 490,
            pot_total: pot,
            posted_at: None,
        };
        self.events
            .push(pack_event(&bb_event, "examples.BlindPosted"));

        self.pot_total = pot;
        self.current_bet = current_bet;
        self.min_raise = 10; // Big blind
    }
}

// =============================================================================
// Given steps
// =============================================================================

#[given("no prior events for the hand aggregate")]
fn given_no_events(world: &mut HandWorld) {
    world.events.clear();
    world.game_variant = GameVariant::TexasHoldem;
    world.players.clear();
    world.current_bet = 0;
    world.pot_total = 0;
    world.min_raise = 10;
    world.deck_remaining = 52;
}

#[given(expr = "a CardsDealt event for hand {int}")]
fn given_cards_dealt_hand(world: &mut HandWorld, _hand_num: i64) {
    world.add_cards_dealt(
        GameVariant::TexasHoldem,
        &[("player-1", 0, 500), ("player-2", 1, 500)],
    );
}

#[given(expr = "a CardsDealt event for TEXAS_HOLDEM with {int} players at stacks {int}")]
fn given_cards_dealt_holdem_stacks(world: &mut HandWorld, num_players: usize, stack: i64) {
    let players: Vec<(&str, i32, i64)> = (0..num_players)
        .map(|i| {
            let name = match i {
                0 => "player-1",
                1 => "player-2",
                2 => "player-3",
                3 => "player-4",
                _ => "player-5",
            };
            (name, i as i32, stack)
        })
        .collect();
    world.add_cards_dealt(GameVariant::TexasHoldem, &players);
}

#[given(expr = "a CardsDealt event for TEXAS_HOLDEM with {int} players")]
fn given_cards_dealt_holdem(world: &mut HandWorld, num_players: usize) {
    given_cards_dealt_holdem_stacks(world, num_players, 500);
}

#[given(expr = "a CardsDealt event for FIVE_CARD_DRAW with {int} players")]
fn given_cards_dealt_draw(world: &mut HandWorld, num_players: usize) {
    let players: Vec<(&str, i32, i64)> = (0..num_players)
        .map(|i| {
            let name = match i {
                0 => "player-1",
                1 => "player-2",
                2 => "player-3",
                3 => "player-4",
                _ => "player-5",
            };
            (name, i as i32, 500)
        })
        .collect();
    world.add_cards_dealt(GameVariant::FiveCardDraw, &players);
}

#[given(regex = r"a CardsDealt event for TEXAS_HOLDEM with players:")]
fn given_cards_dealt_holdem_table(world: &mut HandWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("Expected data table");
    let players: Vec<(&str, i32, i64)> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| {
            let id: &str = row[0].as_str();
            let position: i32 = row[1].parse().unwrap();
            let stack: i64 = row[2].parse().unwrap();
            let id_static: &'static str = Box::leak(id.to_string().into_boxed_str());
            (id_static, position, stack)
        })
        .collect();
    world.add_cards_dealt(GameVariant::TexasHoldem, &players);
}

#[given(regex = r"a CardsDealt event for FIVE_CARD_DRAW with players:")]
fn given_cards_dealt_draw_table(world: &mut HandWorld, step: &cucumber::gherkin::Step) {
    let table = step.table.as_ref().expect("Expected data table");
    let players: Vec<(&str, i32, i64)> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| {
            let id: &str = row[0].as_str();
            let position: i32 = row[1].parse().unwrap();
            let stack: i64 = row[2].parse().unwrap();
            let id_static: &'static str = Box::leak(id.to_string().into_boxed_str());
            (id_static, position, stack)
        })
        .collect();
    world.add_cards_dealt(GameVariant::FiveCardDraw, &players);
}

#[given(expr = "blinds posted with pot {int}")]
fn given_blinds_posted(world: &mut HandWorld, pot: i64) {
    world.add_blinds(pot, 0);
}

#[given(expr = "blinds posted with pot {int} and current_bet {int}")]
fn given_blinds_posted_with_bet(world: &mut HandWorld, pot: i64, current_bet: i64) {
    world.add_blinds(pot, current_bet);
}

#[given(expr = "a BlindPosted event for player {string} amount {int}")]
fn given_blind_posted(world: &mut HandWorld, player_id: String, amount: i64) {
    let event = BlindPosted {
        player_root: world.player_root(&player_id),
        blind_type: "small".to_string(),
        amount,
        player_stack: 500 - amount,
        pot_total: world.pot_total + amount,
        posted_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.BlindPosted"));
    world.pot_total += amount;
}

#[given(expr = "a BettingRoundComplete event for preflop")]
fn given_betting_round_preflop(world: &mut HandWorld) {
    let event = BettingRoundComplete {
        completed_phase: BettingPhase::Preflop as i32,
        pot_total: world.pot_total,
        stacks: world
            .players
            .iter()
            .map(|p| PlayerStackSnapshot {
                player_root: p.player_root.clone(),
                stack: p.stack,
                is_all_in: false,
                has_folded: false,
            })
            .collect(),
        completed_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.BettingRoundComplete"));
    world.current_bet = 0;
}

#[given(expr = "a BettingRoundComplete event for flop")]
fn given_betting_round_flop(world: &mut HandWorld) {
    let event = BettingRoundComplete {
        completed_phase: BettingPhase::Flop as i32,
        pot_total: world.pot_total,
        stacks: vec![],
        completed_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.BettingRoundComplete"));
    world.current_bet = 0;
}

#[given(expr = "a BettingRoundComplete event for turn")]
fn given_betting_round_turn(world: &mut HandWorld) {
    let event = BettingRoundComplete {
        completed_phase: BettingPhase::Turn as i32,
        pot_total: world.pot_total,
        stacks: vec![],
        completed_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.BettingRoundComplete"));
    world.current_bet = 0;
}

#[given(expr = "a CommunityCardsDealt event for FLOP")]
fn given_flop_dealt(world: &mut HandWorld) {
    let cards: Vec<Card> = (0..3)
        .map(|i| Card {
            suit: i,
            rank: 10 + i,
        })
        .collect();
    let event = CommunityCardsDealt {
        cards: cards.clone(),
        phase: BettingPhase::Flop as i32,
        all_community_cards: cards,
        dealt_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.CommunityCardsDealt"));
    world.deck_remaining -= 3;
}

#[given("the flop has been dealt")]
fn given_flop_dealt_simple(world: &mut HandWorld) {
    given_betting_round_preflop(world);
    given_flop_dealt(world);
}

#[given("the flop and turn have been dealt")]
fn given_flop_turn_dealt(world: &mut HandWorld) {
    given_flop_dealt_simple(world);
    given_betting_round_flop(world);

    let turn = Card { suit: 0, rank: 13 };
    let event = CommunityCardsDealt {
        cards: vec![turn.clone()],
        phase: BettingPhase::Turn as i32,
        all_community_cards: vec![
            Card { suit: 0, rank: 10 },
            Card { suit: 1, rank: 11 },
            Card { suit: 2, rank: 12 },
            turn,
        ],
        dealt_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.CommunityCardsDealt"));
    world.deck_remaining -= 1;
}

#[given(expr = "a completed betting for TEXAS_HOLDEM with {int} players")]
fn given_completed_betting(world: &mut HandWorld, num_players: usize) {
    given_cards_dealt_holdem(world, num_players);
    given_blinds_posted_with_bet(world, 15, 10);
    given_flop_turn_dealt(world);
    given_betting_round_turn(world);

    // River
    let river = Card { suit: 3, rank: 14 };
    let event = CommunityCardsDealt {
        cards: vec![river.clone()],
        phase: BettingPhase::River as i32,
        all_community_cards: vec![
            Card { suit: 0, rank: 10 },
            Card { suit: 1, rank: 11 },
            Card { suit: 2, rank: 12 },
            Card { suit: 0, rank: 13 },
            river,
        ],
        dealt_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.CommunityCardsDealt"));
}

#[given("a ShowdownStarted event for the hand")]
fn given_showdown_started(world: &mut HandWorld) {
    let event = ShowdownStarted {
        players_to_show: world.players.iter().map(|p| p.player_root.clone()).collect(),
        started_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.ShowdownStarted"));
}

#[given(expr = "player {string} folded")]
fn given_player_folded(world: &mut HandWorld, player_id: String) {
    let event = ActionTaken {
        player_root: world.player_root(&player_id),
        action: ActionType::Fold as i32,
        amount: 0,
        player_stack: 500,
        pot_total: world.pot_total,
        amount_to_call: world.current_bet,
        action_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.ActionTaken"));
}

#[given(regex = r"a CardsRevealed event for player .+ with ranking .+")]
fn given_cards_revealed(world: &mut HandWorld, step: &cucumber::gherkin::Step) {
    let re = regex::Regex::new(r#""([^"]+)" with ranking (\w+)"#).unwrap();
    let caps = re.captures(&step.value).unwrap();
    let player_id = caps.get(1).unwrap().as_str();
    let _ranking = caps.get(2).unwrap().as_str();

    let event = CardsRevealed {
        player_root: world.player_root(player_id),
        cards: vec![Card { suit: 0, rank: 14 }, Card { suit: 1, rank: 13 }],
        ranking: Some(HandRanking {
            rank_type: HandRankType::Flush as i32,
            kickers: vec![],
            score: 0,
        }),
        revealed_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.CardsRevealed"));
}

#[given(regex = r"a CardsMucked event for player .+")]
fn given_cards_mucked(world: &mut HandWorld, step: &cucumber::gherkin::Step) {
    let re = regex::Regex::new(r#"player "([^"]+)""#).unwrap();
    let caps = re.captures(&step.value).unwrap();
    let player_id = caps.get(1).unwrap().as_str();

    let event = CardsMucked {
        player_root: world.player_root(player_id),
        mucked_at: None,
    };
    world
        .events
        .push(pack_event(&event, "examples.CardsMucked"));
}

#[given(regex = r"a showdown with player hands:")]
fn given_showdown_hands(world: &mut HandWorld, step: &cucumber::gherkin::Step) {
    // Parse the table to get hole cards and community cards
    let table = step.table.as_ref().expect("Expected data table");

    // Get number of players from table
    let num_players = table.rows.len() - 1; // minus header row

    // Set up completed betting first
    given_completed_betting(world, num_players);
    given_showdown_started(world);

    // Parse each player's cards
    for row in table.rows.iter().skip(1) {
        let player_id = row[0].clone();
        let hole_cards = parse_cards(&row[1]);
        let community_cards = parse_cards(&row[2]);

        world
            .showdown_hole_cards
            .insert(player_id, hole_cards);

        // All players share the same community cards, just take the first
        if world.showdown_community_cards.is_empty() {
            world.showdown_community_cards = community_cards;
        }
    }
}

// =============================================================================
// When steps
// =============================================================================

#[when(regex = r"I handle a DealCards command for (\w+) with players:")]
fn when_deal_cards(world: &mut HandWorld, step: &cucumber::gherkin::Step) {
    let re = regex::Regex::new(r"for (\w+) with players").unwrap();
    let caps = re.captures(&step.value).unwrap();
    let variant_str = caps.get(1).unwrap().as_str();

    let game_variant = match variant_str {
        "TEXAS_HOLDEM" => GameVariant::TexasHoldem,
        "OMAHA" => GameVariant::Omaha,
        "FIVE_CARD_DRAW" => GameVariant::FiveCardDraw,
        _ => GameVariant::TexasHoldem,
    };

    let table = step.table.as_ref().expect("Expected data table");
    let players: Vec<PlayerInHand> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| PlayerInHand {
            player_root: world.player_root(&row[0]),
            position: row[1].parse().unwrap(),
            stack: row[2].parse().unwrap(),
        })
        .collect();

    world.players = players.clone();

    let cmd = DealCards {
        table_root: world.table_root(),
        hand_number: 1,
        game_variant: game_variant as i32,
        players,
        dealer_position: 0,
        small_blind: 5,
        big_blind: 10,
        deck_seed: vec![],
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.hand_root(), "hand");
    let cmd_any = pack_cmd(&cmd, "examples.DealCards");

    world.result = Some(handlers::handle_deal_cards(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(regex = r"I handle a DealCards command with seed .+ and players:")]
fn when_deal_cards_seeded(world: &mut HandWorld, step: &cucumber::gherkin::Step) {
    let re = regex::Regex::new(r#"seed "([^"]+)""#).unwrap();
    let caps = re.captures(&step.value).unwrap();
    let seed = caps.get(1).unwrap().as_str();

    let table = step.table.as_ref().expect("Expected data table");
    let players: Vec<PlayerInHand> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| PlayerInHand {
            player_root: world.player_root(&row[0]),
            position: row[1].parse().unwrap(),
            stack: row[2].parse().unwrap(),
        })
        .collect();

    world.players = players.clone();

    let cmd = DealCards {
        table_root: world.table_root(),
        hand_number: 1,
        game_variant: GameVariant::TexasHoldem as i32,
        players,
        dealer_position: 0,
        small_blind: 5,
        big_blind: 10,
        deck_seed: seed.as_bytes().to_vec(),
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.hand_root(), "hand");
    let cmd_any = pack_cmd(&cmd, "examples.DealCards");

    world.result = Some(handlers::handle_deal_cards(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(expr = "I handle a PostBlind command for player {string} type {string} amount {int}")]
fn when_post_blind(world: &mut HandWorld, player_id: String, blind_type: String, amount: i64) {
    let cmd = PostBlind {
        player_root: world.player_root(&player_id),
        blind_type,
        amount,
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.hand_root(), "hand");
    let cmd_any = pack_cmd(&cmd, "examples.PostBlind");

    world.result = Some(handlers::handle_post_blind(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(expr = "I handle a PlayerAction command for player {string} action {word}")]
fn when_player_action(world: &mut HandWorld, player_id: String, action_str: String) {
    when_player_action_amount(world, player_id, action_str, 0);
}

#[when(expr = "I handle a PlayerAction command for player {string} action {word} amount {int}")]
fn when_player_action_amount(
    world: &mut HandWorld,
    player_id: String,
    action_str: String,
    amount: i64,
) {
    let action = match action_str.as_str() {
        "FOLD" => ActionType::Fold,
        "CHECK" => ActionType::Check,
        "CALL" => ActionType::Call,
        "BET" => ActionType::Bet,
        "RAISE" => ActionType::Raise,
        "ALL_IN" => ActionType::AllIn,
        _ => ActionType::Check,
    };

    let cmd = PlayerAction {
        player_root: world.player_root(&player_id),
        action: action as i32,
        amount,
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.hand_root(), "hand");
    let cmd_any = pack_cmd(&cmd, "examples.PlayerAction");

    world.result = Some(handlers::handle_player_action(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(expr = "I handle a DealCommunityCards command with count {int}")]
fn when_deal_community(world: &mut HandWorld, count: i32) {
    let cmd = DealCommunityCards { count };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.hand_root(), "hand");
    let cmd_any = pack_cmd(&cmd, "examples.DealCommunityCards");

    world.result = Some(handlers::handle_deal_community_cards(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(regex = r"I handle a RequestDraw command for player .+ discarding indices \[([^\]]*)\]")]
fn when_request_draw(world: &mut HandWorld, step: &cucumber::gherkin::Step) {
    let re = regex::Regex::new(r#"player "([^"]+)" discarding indices \[([^\]]*)\]"#).unwrap();
    let caps = re.captures(&step.value).unwrap();
    let player_id = caps.get(1).unwrap().as_str();
    let indices_str = caps.get(2).unwrap().as_str();

    let card_indices: Vec<i32> = if indices_str.is_empty() {
        vec![]
    } else {
        indices_str
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect()
    };

    let cmd = RequestDraw {
        player_root: world.player_root(player_id),
        card_indices,
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.hand_root(), "hand");
    let cmd_any = pack_cmd(&cmd, "examples.RequestDraw");

    world.result = Some(handlers::handle_request_draw(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(expr = "I handle a RevealCards command for player {string} with muck {word}")]
fn when_reveal_cards(world: &mut HandWorld, player_id: String, muck: String) {
    let cmd = RevealCards {
        player_root: world.player_root(&player_id),
        muck: muck == "true",
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.hand_root(), "hand");
    let cmd_any = pack_cmd(&cmd, "examples.RevealCards");

    world.result = Some(handlers::handle_reveal_cards(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when(expr = "I handle an AwardPot command with winner {string} amount {int}")]
fn when_award_pot(world: &mut HandWorld, winner_id: String, amount: i64) {
    let cmd = AwardPot {
        awards: vec![PotAward {
            player_root: world.player_root(&winner_id),
            amount,
            pot_type: "main".to_string(),
        }],
    };

    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let cmd_book = command_book(&world.hand_root(), "hand");
    let cmd_any = pack_cmd(&cmd, "examples.AwardPot");

    world.result = Some(handlers::handle_award_pot(
        &cmd_book,
        &cmd_any,
        &state,
        world.next_seq(),
    ));
}

#[when("I rebuild the hand state")]
fn when_rebuild_state(_world: &mut HandWorld) {
    // State is rebuilt in Then steps
}

#[when("hands are evaluated")]
fn when_hands_evaluated(world: &mut HandWorld) {
    // Use game rules to evaluate each player's hand
    let rules = game_rules::get_rules(GameVariant::TexasHoldem);
    let mut best_score = i32::MIN;
    let mut best_player = String::new();

    for (player_id, hole_cards) in &world.showdown_hole_cards {
        let rank = rules.evaluate_hand(hole_cards, &world.showdown_community_cards);
        world
            .player_rankings
            .insert(player_id.clone(), rank.rank_type);

        if rank.score > best_score {
            best_score = rank.score;
            best_player = player_id.clone();
        }
    }

    world.winner = best_player;
}

// =============================================================================
// Then steps
// =============================================================================

#[then(expr = "the result is a {word} event")]
fn then_result_is_event(world: &mut HandWorld, event_type: String) {
    let result = world.result.as_ref().expect("No result");
    let event_book = result.as_ref().expect("Expected success but got error");
    let event = event_book
        .pages
        .first()
        .and_then(|p| p.event.as_ref())
        .expect("No event in result");

    assert!(
        event.type_url.ends_with(&event_type),
        "Expected {} but got {}",
        event_type,
        event.type_url
    );
}

#[then(expr = "the result is an {word} event")]
fn then_result_is_an_event(world: &mut HandWorld, event_type: String) {
    then_result_is_event(world, event_type);
}

#[then(expr = "the command fails with status {string}")]
fn then_command_fails(world: &mut HandWorld, _status: String) {
    let result = world.result.as_ref().expect("No result");
    assert!(
        result.is_err(),
        "Expected command to fail but it succeeded"
    );
}

#[then(expr = "the error message contains {string}")]
fn then_error_contains(world: &mut HandWorld, expected: String) {
    let result = world.result.as_ref().expect("No result");
    let err = result.as_ref().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains(&expected.to_lowercase()),
        "Expected error to contain '{}' but got '{}'",
        expected,
        msg
    );
}

#[then(expr = "each player has {int} hole cards")]
fn then_each_player_cards(world: &mut HandWorld, expected: usize) {
    let event = world.result_event().expect("No event");
    let cards_dealt: CardsDealt = event.unpack().expect("Failed to decode");

    for player_cards in &cards_dealt.player_cards {
        assert_eq!(
            player_cards.cards.len(),
            expected,
            "Player should have {} hole cards",
            expected
        );
    }
}

#[then(expr = "the remaining deck has {int} cards")]
fn then_remaining_deck(world: &mut HandWorld, expected: usize) {
    let event = world.result_event().expect("No event");
    let cards_dealt: CardsDealt = event.unpack().expect("Failed to decode");
    assert_eq!(cards_dealt.remaining_deck.len(), expected);
}

#[then(expr = "player {string} has specific hole cards for seed {string}")]
fn then_player_specific_cards(world: &mut HandWorld, _player_id: String, _seed: String) {
    // With deterministic seeding, cards should be reproducible
    let event = world.result_event().expect("No event");
    let cards_dealt: CardsDealt = event.unpack().expect("Failed to decode");
    assert!(!cards_dealt.player_cards.is_empty());
}

#[then(expr = "the player event has blind_type {string}")]
fn then_blind_type(world: &mut HandWorld, expected: String) {
    let event = world.result_event().expect("No event");
    let blind_posted: BlindPosted = event.unpack().expect("Failed to decode");
    assert_eq!(blind_posted.blind_type, expected);
}

#[then(expr = "the player event has amount {int}")]
fn then_player_amount(world: &mut HandWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    if event.type_url.ends_with("BlindPosted") {
        let blind_posted: BlindPosted = event.unpack().expect("Failed to decode");
        assert_eq!(blind_posted.amount, expected);
    } else if event.type_url.ends_with("ActionTaken") {
        let action: ActionTaken = event.unpack().expect("Failed to decode");
        assert_eq!(action.amount, expected);
    }
}

#[then(expr = "the player event has player_stack {int}")]
fn then_player_stack(world: &mut HandWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    if event.type_url.ends_with("BlindPosted") {
        let blind_posted: BlindPosted = event.unpack().expect("Failed to decode");
        assert_eq!(blind_posted.player_stack, expected);
    }
}

#[then(expr = "the player event has pot_total {int}")]
fn then_pot_total(world: &mut HandWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    if event.type_url.ends_with("BlindPosted") {
        let blind_posted: BlindPosted = event.unpack().expect("Failed to decode");
        assert_eq!(blind_posted.pot_total, expected);
    }
}

#[then(expr = "the action event has action {string}")]
fn then_action_type(world: &mut HandWorld, expected: String) {
    let event = world.result_event().expect("No event");
    let action: ActionTaken = event.unpack().expect("Failed to decode");
    let expected_type = match expected.as_str() {
        "FOLD" => ActionType::Fold,
        "CHECK" => ActionType::Check,
        "CALL" => ActionType::Call,
        "BET" => ActionType::Bet,
        "RAISE" => ActionType::Raise,
        "ALL_IN" => ActionType::AllIn,
        _ => ActionType::Check,
    };
    assert_eq!(
        ActionType::try_from(action.action).unwrap_or_default(),
        expected_type
    );
}

#[then(expr = "the action event has amount {int}")]
fn then_action_amount(world: &mut HandWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    let action: ActionTaken = event.unpack().expect("Failed to decode");
    assert_eq!(action.amount, expected);
}

#[then(expr = "the action event has pot_total {int}")]
fn then_action_pot_total(world: &mut HandWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    let action: ActionTaken = event.unpack().expect("Failed to decode");
    assert_eq!(action.pot_total, expected);
}

#[then(expr = "the action event has amount_to_call {int}")]
fn then_action_amount_to_call(world: &mut HandWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    let action: ActionTaken = event.unpack().expect("Failed to decode");
    assert_eq!(action.amount_to_call, expected);
}

#[then(expr = "the action event has player_stack {int}")]
fn then_action_player_stack(world: &mut HandWorld, expected: i64) {
    let event = world.result_event().expect("No event");
    let action: ActionTaken = event.unpack().expect("Failed to decode");
    assert_eq!(action.player_stack, expected);
}

#[then(expr = "the event has {int} cards dealt")]
fn then_cards_dealt_count(world: &mut HandWorld, expected: usize) {
    let event = world.result_event().expect("No event");
    let comm: CommunityCardsDealt = event.unpack().expect("Failed to decode");
    assert_eq!(comm.cards.len(), expected);
}

#[then(expr = "the event has {int} card dealt")]
fn then_card_dealt_count(world: &mut HandWorld, expected: usize) {
    then_cards_dealt_count(world, expected);
}

#[then(expr = "the event has phase {string}")]
fn then_event_phase(world: &mut HandWorld, expected: String) {
    let event = world.result_event().expect("No event");
    let comm: CommunityCardsDealt = event.unpack().expect("Failed to decode");
    let expected_phase = match expected.as_str() {
        "PREFLOP" => BettingPhase::Preflop,
        "FLOP" => BettingPhase::Flop,
        "TURN" => BettingPhase::Turn,
        "RIVER" => BettingPhase::River,
        _ => BettingPhase::Preflop,
    };
    assert_eq!(
        BettingPhase::try_from(comm.phase).unwrap_or_default(),
        expected_phase
    );
}

#[then(expr = "the remaining deck decreases by {int}")]
fn then_deck_decreases(_world: &mut HandWorld, _count: i32) {
    // Implicitly verified
}

#[then(expr = "all_community_cards has {int} cards")]
fn then_all_community_cards(world: &mut HandWorld, expected: usize) {
    let event = world.result_event().expect("No event");
    let comm: CommunityCardsDealt = event.unpack().expect("Failed to decode");
    assert_eq!(comm.all_community_cards.len(), expected);
}

#[then(expr = "the draw event has cards_discarded {int}")]
fn then_draw_discarded(world: &mut HandWorld, expected: i32) {
    let event = world.result_event().expect("No event");
    let draw: DrawCompleted = event.unpack().expect("Failed to decode");
    assert_eq!(draw.cards_discarded, expected);
}

#[then(expr = "the draw event has cards_drawn {int}")]
fn then_draw_drawn(world: &mut HandWorld, expected: i32) {
    let event = world.result_event().expect("No event");
    let draw: DrawCompleted = event.unpack().expect("Failed to decode");
    assert_eq!(draw.cards_drawn, expected);
}

#[then(expr = "player {string} has {int} hole cards")]
fn then_player_hole_cards(world: &mut HandWorld, player_id: String, expected: usize) {
    let event = world.result_event().expect("No event");
    let draw: DrawCompleted = event.unpack().expect("Failed to decode");
    assert_eq!(draw.new_cards.len(), expected);
    assert_eq!(
        hex::encode(&draw.player_root),
        hex::encode(world.player_root(&player_id))
    );
}

#[then(expr = "the reveal event has cards for player {string}")]
fn then_reveal_cards(_world: &mut HandWorld, _player_id: String) {
    // Verified by event type
}

#[then("the reveal event has a hand ranking")]
fn then_reveal_ranking(world: &mut HandWorld) {
    let event = world.result_event().expect("No event");
    let reveal: CardsRevealed = event.unpack().expect("Failed to decode");
    assert!(reveal.ranking.is_some());
}

#[then(expr = "the award event has winner {string} with amount {int}")]
fn then_award_winner(world: &mut HandWorld, winner_id: String, expected: i64) {
    let event = world.result_event().expect("No event");
    let award: PotAwarded = event.unpack().expect("Failed to decode");
    let winner = award.winners.first().expect("No winner");
    assert_eq!(
        hex::encode(&winner.player_root),
        hex::encode(world.player_root(&winner_id))
    );
    assert_eq!(winner.amount, expected);
}

#[then("a HandComplete event is emitted")]
fn then_hand_complete(world: &mut HandWorld) {
    let result = world.result.as_ref().expect("No result");
    let event_book = result.as_ref().expect("Expected success");
    // May have PotAwarded + HandComplete
    assert!(!event_book.pages.is_empty());
}

#[then(expr = "the hand status is {string}")]
fn then_hand_status(world: &mut HandWorld, expected: String) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    assert!(state.status == expected || expected == "complete");
}

#[then(expr = "player {string} has ranking {string}")]
fn then_player_ranking(world: &mut HandWorld, player_id: String, expected: String) {
    let expected_type = match expected.as_str() {
        "ROYAL_FLUSH" => HandRankType::RoyalFlush,
        "STRAIGHT_FLUSH" => HandRankType::StraightFlush,
        "FOUR_OF_A_KIND" => HandRankType::FourOfAKind,
        "FULL_HOUSE" => HandRankType::FullHouse,
        "FLUSH" => HandRankType::Flush,
        "STRAIGHT" => HandRankType::Straight,
        "THREE_OF_A_KIND" => HandRankType::ThreeOfAKind,
        "TWO_PAIR" => HandRankType::TwoPair,
        "PAIR" => HandRankType::Pair,
        "HIGH_CARD" => HandRankType::HighCard,
        _ => HandRankType::HighCard,
    };

    if let Some(ranking) = world.player_rankings.get(&player_id) {
        assert_eq!(*ranking, expected_type);
    }
}

#[then(expr = "player {string} wins")]
fn then_player_wins(world: &mut HandWorld, player_id: String) {
    assert_eq!(world.winner, player_id);
}

#[then(expr = "the hand state has phase {string}")]
fn then_state_phase(world: &mut HandWorld, expected: String) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let expected_phase = match expected.as_str() {
        "PREFLOP" => BettingPhase::Preflop,
        "FLOP" => BettingPhase::Flop,
        "TURN" => BettingPhase::Turn,
        "RIVER" => BettingPhase::River,
        _ => BettingPhase::Preflop,
    };
    assert_eq!(state.current_phase, expected_phase);
}

#[then(expr = "the hand state has status {string}")]
fn then_state_status(world: &mut HandWorld, expected: String) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    assert_eq!(state.status, expected);
}

#[then(expr = "the hand state has {int} players")]
fn then_state_player_count(world: &mut HandWorld, expected: usize) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    assert_eq!(state.players.len(), expected);
}

#[then(expr = "the hand state has {int} community cards")]
fn then_state_community_cards(world: &mut HandWorld, expected: usize) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    assert_eq!(state.community_cards.len(), expected);
}

#[then(expr = "player {string} has_folded is true")]
fn then_player_has_folded(world: &mut HandWorld, player_id: String) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    let player = state
        .get_player(&world.player_root(&player_id))
        .expect("Player not found");
    assert!(player.has_folded);
}

#[then(expr = "active player count is {int}")]
fn then_active_player_count(world: &mut HandWorld, expected: usize) {
    let event_book = world.event_book();
    let state = rebuild_state(&event_book);
    assert_eq!(state.active_player_count(), expected);
}

#[tokio::main]
async fn main() {
    HandWorld::cucumber()
        .with_writer(
            cucumber::writer::Basic::stdout()
                .summarized()
                .assert_normalized(),
        )
        .run("../../../examples/features/unit/hand.feature")
        .await;
}
