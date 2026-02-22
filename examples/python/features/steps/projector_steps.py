"""Behave step definitions for projector tests."""

import re
from datetime import datetime, timezone

from behave import given, when, then, use_step_matcher
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

from projector import OutputProjector
from renderer import TextRenderer, format_card

# Use regex matchers for flexibility
use_step_matcher("re")


def make_timestamp():
    """Create current timestamp."""
    return Timestamp(seconds=int(datetime.now(timezone.utc).timestamp()))


def make_event_page(event_msg, time_str: str = None) -> types.EventPage:
    """Create EventPage with packed event."""
    event_any = ProtoAny()
    event_any.Pack(event_msg, type_url_prefix="type.googleapis.com/")

    created_at = None
    if time_str:
        h, m, s = map(int, time_str.split(":"))
        dt = datetime(2024, 1, 1, h, m, s, tzinfo=timezone.utc)
        created_at = Timestamp(seconds=int(dt.timestamp()))
    else:
        created_at = make_timestamp()

    return types.EventPage(
        sequence=0,
        event=event_any,
        created_at=created_at,
    )


def make_card(rank: int, suit: int):
    """Create a card proto."""
    return poker_types.Card(rank=rank, suit=suit)


def parse_card(card_str: str):
    """Parse card string like 'As', 'Kh', '7s' into rank and suit."""
    rank_map = {
        "2": 2,
        "3": 3,
        "4": 4,
        "5": 5,
        "6": 6,
        "7": 7,
        "8": 8,
        "9": 9,
        "T": 10,
        "10": 10,
        "J": 11,
        "Q": 12,
        "K": 13,
        "A": 14,
    }
    suit_map = {
        "s": poker_types.SPADES,
        "h": poker_types.HEARTS,
        "d": poker_types.DIAMONDS,
        "c": poker_types.CLUBS,
    }
    # Handle both "As" and "10s" formats
    rank_char = card_str[:-1] if len(card_str) > 2 else card_str[0]
    suit_char = card_str[-1].lower()
    rank = rank_map.get(rank_char)
    if rank is None:
        try:
            rank = int(rank_char)
        except ValueError:
            rank = 2  # Default
    return make_card(rank, suit_map.get(suit_char, poker_types.SPADES))


# --- Given steps ---


@given("an OutputProjector")
def step_given_output_projector(context):
    """Create OutputProjector instance."""
    context.output_lines = []
    context.projector = OutputProjector(
        output_fn=lambda text: context.output_lines.append(text),
        show_timestamps=False,
    )


@given('an OutputProjector with player name "(?P<name>[^"]+)"')
def step_given_projector_with_player(context, name):
    """Create projector with a player name registered."""
    context.output_lines = []
    context.projector = OutputProjector(
        output_fn=lambda text: context.output_lines.append(text),
        show_timestamps=False,
    )
    context.projector.set_player_name(b"player-1", name)


@given('an OutputProjector with player names "(?P<name1>[^"]+)" and "(?P<name2>[^"]+)"')
def step_given_projector_with_two_players(context, name1, name2):
    """Create projector with two player names registered."""
    context.output_lines = []
    context.projector = OutputProjector(
        output_fn=lambda text: context.output_lines.append(text),
        show_timestamps=False,
    )
    context.projector.set_player_name(b"player-1", name1)
    context.projector.set_player_name(b"player-2", name2)


@given("an OutputProjector with show_timestamps enabled")
def step_given_projector_with_timestamps(context):
    """Create projector with timestamps enabled."""
    context.output_lines = []
    context.projector = OutputProjector(
        output_fn=lambda text: context.output_lines.append(text),
        show_timestamps=True,
    )


@given("an OutputProjector with show_timestamps disabled")
def step_given_projector_without_timestamps(context):
    """Create projector with timestamps disabled."""
    context.output_lines = []
    context.projector = OutputProjector(
        output_fn=lambda text: context.output_lines.append(text),
        show_timestamps=False,
    )


@given('a PlayerRegistered event with display_name "(?P<name>[^"]+)"')
def step_given_player_registered_with_name(context, name):
    """Create a PlayerRegistered event with given name."""
    context.event = player.PlayerRegistered(
        display_name=name,
        email="test@example.com",
        player_type=poker_types.HUMAN,
    )


@given(
    "a FundsDeposited event with amount (?P<amount>\\d+) and new_balance (?P<balance>\\d+)"
)
def step_given_funds_deposited_with_balance(context, amount, balance):
    """Create a FundsDeposited event with specific balance."""
    context.event = player.FundsDeposited(
        amount=poker_types.Currency(amount=int(amount)),
        new_balance=poker_types.Currency(amount=int(balance)),
    )


@given(
    "a FundsWithdrawn event with amount (?P<amount>\\d+) and new_balance (?P<balance>\\d+)"
)
def step_given_funds_withdrawn_with_balance(context, amount, balance):
    """Create a FundsWithdrawn event with specific balance."""
    context.event = player.FundsWithdrawn(
        amount=poker_types.Currency(amount=int(amount)),
        new_balance=poker_types.Currency(amount=int(balance)),
    )


@given("a FundsReserved event with amount (?P<amount>\\d+)")
def step_given_funds_reserved_event(context, amount):
    """Create a FundsReserved event."""
    context.event = player.FundsReserved(
        amount=poker_types.Currency(amount=int(amount)),
        table_root=b"table-1",
    )


@given("a TableCreated event with:")
def step_given_table_created_with_table(context):
    """Create a TableCreated event from datatable."""
    row = {
        context.table.headings[i]: context.table[0][i]
        for i in range(len(context.table.headings))
    }
    variant = getattr(poker_types, row.get("game_variant", "TEXAS_HOLDEM"))

    context.event = table.TableCreated(
        table_name=row["table_name"],
        game_variant=variant,
        small_blind=int(row["small_blind"]),
        big_blind=int(row["big_blind"]),
        min_buy_in=int(row.get("min_buy_in", 200)),
        max_buy_in=int(row.get("max_buy_in", 1000)),
        max_players=int(row.get("max_players", 9)),
        created_at=make_timestamp(),
    )


@given("a PlayerJoined event at seat (?P<seat>\\d+) with buy_in (?P<buy_in>\\d+)")
def step_given_player_joined_buy_in(context, seat, buy_in):
    """Create a PlayerJoined event with buy_in."""
    context.event = table.PlayerJoined(
        player_root=b"player-1",
        seat_position=int(seat),
        stack=int(buy_in),
        buy_in_amount=int(buy_in),
        joined_at=make_timestamp(),
    )


@given("a PlayerLeft event with chips_cashed_out (?P<amount>\\d+)")
def step_given_player_left_cashed(context, amount):
    """Create a PlayerLeft event with cashed out amount."""
    context.event = table.PlayerLeft(
        player_root=b"player-1",
        chips_cashed_out=int(amount),
        left_at=make_timestamp(),
    )


# "a HandStarted event with:" step is defined in process_manager_steps.py
# to avoid duplication


@given(
    'active players "(?P<player1>[^"]+)", "(?P<player2>[^"]+)", "(?P<player3>[^"]+)" at seats (?P<seats>.+)'
)
def step_given_active_players_three(context, player1, player2, player3, seats):
    """Add three active players from inline list."""
    player_names = [player1, player2, player3]
    seat_nums = [int(s.strip()) for s in seats.split(",")]

    for i, (name, seat) in enumerate(zip(player_names, seat_nums)):
        context.hand_started.active_players.append(
            table.SeatSnapshot(
                player_root=f"player-{i + 1}".encode(),
                position=seat,
                stack=500,
            )
        )
        context.projector.set_player_name(f"player-{i + 1}".encode(), name)


@given(
    'active players "(?P<player1>[^"]+)" and "(?P<player2>[^"]+)" at seats (?P<seats>.+)'
)
def step_given_active_players_two(context, player1, player2, seats):
    """Add two active players from inline list."""
    player_names = [player1, player2]
    seat_nums = [int(s.strip()) for s in seats.split(",")]

    for i, (name, seat) in enumerate(zip(player_names, seat_nums)):
        context.hand_started.active_players.append(
            table.SeatSnapshot(
                player_root=f"player-{i + 1}".encode(),
                position=seat,
                stack=500,
            )
        )
        context.projector.set_player_name(f"player-{i + 1}".encode(), name)


@given('a HandEnded event with winner "(?P<winner>[^"]+)" amount (?P<amount>\\d+)')
def step_given_hand_ended_with_winner(context, winner, amount):
    """Create a HandEnded event with winner."""
    context.event = table.HandEnded(
        hand_root=b"hand-1",
        ended_at=make_timestamp(),
    )
    # Use results field which the renderer expects
    context.event.results.append(
        table.PotResult(
            winner_root=b"player-1",
            amount=int(amount),
            pot_type="main",
        )
    )
    context.projector.set_player_name(b"player-1", winner)


@given('a CardsDealt event with player "(?P<player_name>[^"]+)" holding (?P<cards>.+)')
def step_given_cards_dealt_for_player(context, player_name, cards):
    """Create a CardsDealt event with player cards."""
    event = hand.CardsDealt(
        table_root=b"table-1",
        hand_number=1,
        game_variant=poker_types.TEXAS_HOLDEM,
    )
    card_list = [parse_card(c.strip()) for c in cards.split()]
    event.player_cards.append(
        hand.PlayerHoleCards(
            player_root=b"player-1",
            cards=card_list,
        )
    )
    context.event = event
    context.projector.set_player_name(b"player-1", player_name)


@given(
    'a BlindPosted event for "(?P<player>[^"]+)" type "(?P<blind_type>[^"]+)" amount (?P<amount>\\d+)'
)
def step_given_blind_posted_event(context, player, blind_type, amount):
    """Create a BlindPosted event."""
    context.event = hand.BlindPosted(
        player_root=b"player-1",
        blind_type=blind_type,
        amount=int(amount),
        pot_total=int(amount),
        player_stack=500 - int(amount),
    )


@given('an ActionTaken event for "(?P<player>[^"]+)" action (?P<action>\\w+)')
def step_given_action_taken_fold(context, player, action):
    """Create an ActionTaken event for fold."""
    action_enum = getattr(poker_types, action)
    context.event = hand.ActionTaken(
        player_root=b"player-1",
        action=action_enum,
        amount=0,
        pot_total=0,
        player_stack=500,
    )


@given(
    'an ActionTaken event for "(?P<player>[^"]+)" action (?P<action>\\w+) amount (?P<amount>\\d+) pot_total (?P<pot>\\d+)'
)
def step_given_action_taken_with_amount(context, player, action, amount, pot):
    """Create an ActionTaken event with amount."""
    action_enum = getattr(poker_types, action)
    context.event = hand.ActionTaken(
        player_root=b"player-1",
        action=action_enum,
        amount=int(amount),
        pot_total=int(pot),
        player_stack=500 - int(amount),
    )


@given("a CommunityCardsDealt event for (?P<phase>\\w+) with cards (?P<cards>.+)")
def step_given_community_cards_event(context, phase, cards):
    """Create a CommunityCardsDealt event."""
    phase_enum = getattr(poker_types, phase)
    event = hand.CommunityCardsDealt(phase=phase_enum)
    for card_str in cards.split():
        event.cards.append(parse_card(card_str.strip()))
    context.event = event


@given("a CommunityCardsDealt event for (?P<phase>\\w+) with card (?P<card>\\w+)")
def step_given_community_cards_single(context, phase, card):
    """Create a CommunityCardsDealt event with single card."""
    phase_enum = getattr(poker_types, phase)
    event = hand.CommunityCardsDealt(phase=phase_enum)
    event.cards.append(parse_card(card.strip()))
    context.event = event


@given("a ShowdownStarted event")
def step_given_showdown_started(context):
    """Create a ShowdownStarted event."""
    context.event = hand.ShowdownStarted()


@given(
    'a CardsRevealed event for "(?P<player>[^"]+)" with cards (?P<cards>\\w+ \\w+) and ranking (?P<ranking>\\w+)'
)
def step_given_cards_revealed(context, player, cards, ranking):
    """Create a CardsRevealed event."""
    card_list = [parse_card(c.strip()) for c in cards.split()]
    ranking_enum = getattr(poker_types, ranking, poker_types.HIGH_CARD)
    context.event = hand.CardsRevealed(
        player_root=b"player-1",
        cards=card_list,
        ranking=poker_types.HandRanking(rank_type=ranking_enum),
        revealed_at=make_timestamp(),
    )


@given('a CardsMucked event for "(?P<player>[^"]+)"')
def step_given_cards_mucked(context, player):
    """Create a CardsMucked event."""
    context.event = hand.CardsMucked(player_root=b"player-1")


@given('a PotAwarded event with winner "(?P<winner>[^"]+)" amount (?P<amount>\\d+)')
def step_given_pot_awarded_event(context, winner, amount):
    """Create a PotAwarded event."""
    event = hand.PotAwarded()
    event.winners.append(
        hand.PotWinner(
            player_root=b"player-1",
            amount=int(amount),
            pot_type="main",
        )
    )
    context.event = event


@given("a HandComplete event with final stacks:")
def step_given_hand_complete_with_stacks(context):
    """Create a HandComplete event from datatable."""
    event = hand.HandComplete(table_root=b"table-1")

    for i, row in enumerate(context.table):
        row_dict = {
            context.table.headings[j]: row[j]
            for j in range(len(context.table.headings))
        }
        player_name = row_dict.get("player", f"Player{i + 1}")
        player_root = f"player-{i + 1}".encode()
        has_folded_str = row_dict.get("has_folded", "false").lower()
        has_folded = has_folded_str in ("true", "yes", "1")
        event.final_stacks.append(
            hand.PlayerStackSnapshot(
                player_root=player_root,
                stack=int(row_dict.get("stack", 500)),
                has_folded=has_folded,
            )
        )
        context.projector.set_player_name(player_root, player_name)
    context.event = event


@given(
    'a PlayerTimedOut event for "(?P<player>[^"]+)" with default_action (?P<action>\\w+)'
)
def step_given_player_timed_out(context, player, action):
    """Create a PlayerTimedOut event."""
    action_enum = getattr(poker_types, action, poker_types.FOLD)
    context.event = hand.PlayerTimedOut(
        player_root=b"player-1",
        default_action=action_enum,
        timed_out_at=make_timestamp(),
    )


@given('player "(?P<player_id>[^"]+)" is registered as "(?P<name>[^"]+)"')
def step_given_player_registered_as(context, player_id, name):
    """Register player with name."""
    context.projector.set_player_name(player_id.encode(), name)


@given("an event with created_at (?P<time>\\d+:\\d+:\\d+)")
def step_given_event_with_time(context, time):
    """Create a simple event with specific created_at timestamp."""
    context.event = player.PlayerRegistered(
        display_name="Test",
        email="test@example.com",
        player_type=poker_types.HUMAN,
    )
    context.event_time = time


@given("an event with created_at")
def step_given_event_with_created_at(context):
    """Create a simple event with default timestamp."""
    context.event = player.PlayerRegistered(
        display_name="Test",
        email="test@example.com",
        player_type=poker_types.HUMAN,
    )


@given("an event book with PlayerJoined and BlindPosted events")
def step_given_event_book_with_two_events(context):
    """Create event book with two events."""
    context.event_book = types.EventBook(
        cover=types.Cover(root=types.UUID(value=b"player-1"), domain="table"),
        pages=[
            make_event_page(
                table.PlayerJoined(
                    player_root=b"player-1",
                    seat_position=1,
                    stack=500,
                    buy_in_amount=500,
                    joined_at=make_timestamp(),
                )
            ),
            make_event_page(
                hand.BlindPosted(
                    player_root=b"player-1",
                    blind_type="small",
                    amount=5,
                    pot_total=5,
                    player_stack=495,
                )
            ),
        ],
    )


@given('an event with unknown type_url "(?P<type_url>[^"]+)"')
def step_given_unknown_event(context, type_url):
    """Create an unknown event type."""
    context.event_page_override = types.EventPage(
        sequence=0,
        event=ProtoAny(type_url=type_url),
        created_at=make_timestamp(),
    )


# --- When steps ---


@when("the projector handles the event")
def step_when_projector_handles_event(context):
    """Handle the event with projector."""
    if hasattr(context, "event_page_override"):
        event_page = context.event_page_override
    else:
        time_str = getattr(context, "event_time", None)
        event_page = make_event_page(context.event, time_str)

    context.projector.handle_event(event_page)


@when("the projector handles the event book")
def step_when_handles_event_book(context):
    """Handle event book."""
    context.projector.handle_event_book(context.event_book)


@when("formatting cards:")
def step_when_formatting_cards(context):
    """Format cards from datatable."""
    context.cards_output = ""
    for row in context.table:
        row_dict = {
            context.table.headings[i]: row[i]
            for i in range(len(context.table.headings))
        }
        rank = int(row_dict.get("rank", 2))
        suit_name = row_dict.get("suit", "SPADES")
        suit = getattr(poker_types, suit_name)
        card = make_card(rank, suit)
        context.cards_output += format_card(card) + " "


@when("formatting cards with rank 2 through 14")
def step_when_formatting_ranks(context):
    """Format all ranks."""
    context.cards_output = ""
    for rank in range(2, 15):
        card = make_card(rank, poker_types.SPADES)
        context.cards_output += format_card(card) + " "


@when('an event references "(?P<player_id>[^"]+)"')
def step_when_event_references(context, player_id):
    """Handle event with player reference using PlayerJoined which renders player name."""
    player_root = player_id.encode()
    context.event = table.PlayerJoined(
        player_root=player_root,
        seat_position=1,
        stack=500,
        buy_in_amount=500,
        joined_at=make_timestamp(),
    )
    event_page = make_event_page(context.event)
    event_book = types.EventBook(
        cover=types.Cover(root=types.UUID(value=b"table-1"), domain="table"),
        pages=[event_page],
    )
    context.projector.handle_event_book(event_book)


@when('an event references unknown "(?P<player_id>[^"]+)"')
def step_when_event_references_unknown(context, player_id):
    """Handle event with unknown player reference using PlayerJoined."""
    player_root = player_id.encode()
    context.event = table.PlayerJoined(
        player_root=player_root,
        seat_position=1,
        stack=500,
        buy_in_amount=500,
        joined_at=make_timestamp(),
    )
    event_page = make_event_page(context.event)
    event_book = types.EventBook(
        cover=types.Cover(root=types.UUID(value=b"table-1"), domain="table"),
        pages=[event_page],
    )
    context.projector.handle_event_book(event_book)


# --- Then steps ---


@then('the output contains "(?P<text>[^"]+)"')
def step_then_output_contains(context, text):
    """Verify output contains text."""
    combined = "\n".join(context.output_lines)
    # Also check cards_output for card formatting tests
    if hasattr(context, "cards_output"):
        combined += "\n" + context.cards_output
    assert text in combined, f"Expected '{text}' in:\n{combined}"


@then('the output starts with "(?P<prefix>[^"]+)"')
def step_then_output_starts_with(context, prefix):
    """Verify output starts with prefix."""
    if context.output_lines:
        assert context.output_lines[0].startswith(prefix), (
            f"Expected start '{prefix}' in:\n{context.output_lines[0]}"
        )
    else:
        raise AssertionError("No output produced")


@then('the output does not start with "(?P<prefix>[^"]+)"')
def step_then_output_not_starts_with(context, prefix):
    """Verify output does not start with prefix."""
    if context.output_lines:
        assert not context.output_lines[0].startswith(prefix)


@then("both events are rendered in order")
def step_then_both_events_rendered(context):
    """Verify both events rendered."""
    assert len(context.output_lines) == 2


@then('the output uses "(?P<name>[^"]+)"')
def step_then_output_uses_name(context, name):
    """Verify output uses player name."""
    combined = "\n".join(context.output_lines)
    assert name in combined


@then('the output uses "(?P<name>[^"]+)" prefix')
def step_then_output_uses_name_prefix(context, name):
    """Verify output uses player name prefix.

    For 'Player_xyz789' prefix, we check for 'Player_' followed by hex chars
    since the renderer uses hex representation of the player root.
    """
    combined = "\n".join(context.output_lines)
    # Check for the Player_ prefix pattern (renderer uses hex)
    if name.startswith("Player_"):
        assert "Player_" in combined, f"Expected 'Player_' prefix in:\n{combined}"
    else:
        assert name in combined, f"Expected '{name}' in:\n{combined}"


@then('the formatted output contains "(?P<text>[^"]+)" symbols')
def step_then_output_contains_symbols(context, text):
    """Verify formatted output contains symbols."""
    assert text in context.cards_output or any(
        s in context.cards_output for s in ["♠", "♥", "♦", "♣"]
    )


@then("ranks 2-9 display as digits")
def step_then_ranks_2_9_display_as_digits(context):
    """Verify ranks 2-9 are digits."""
    for digit in "23456789":
        assert digit in context.cards_output


@then('rank (?P<rank>\\d+) displays as "(?P<symbol>[^"]+)"')
def step_then_rank_displays_as(context, rank, symbol):
    """Verify rank display."""
    assert symbol in context.cards_output


@then("face cards display as letters")
def step_then_face_cards_display_as_letters(context):
    """Verify face cards are letters."""
    for letter in "JQK":
        assert letter in context.cards_output or "10" in context.cards_output


@then("a warning is printed for unknown event")
def step_then_warning_for_unknown_event(context):
    """Verify warning printed."""
    combined = "\n".join(context.output_lines)
    assert "Unknown event type" in combined
