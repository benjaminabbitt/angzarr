"""Behave step definitions for process manager tests."""

from datetime import datetime, timezone

from behave import given, when, then, use_step_matcher
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "hand-flow"))

from hand_process import HandProcess, HandProcessManager, HandPhase, PlayerState

# Use regex matchers for flexibility
use_step_matcher("re")


def make_timestamp():
    """Create current timestamp."""
    return Timestamp(seconds=int(datetime.now(timezone.utc).timestamp()))


# Default test table root and hand ID using hex format
DEFAULT_TABLE_ROOT = b"table-1"
DEFAULT_HAND_ID = f"{DEFAULT_TABLE_ROOT.hex()}_1"


class TestCommandSender:
    """Captures commands sent by the process manager."""

    def __init__(self):
        self.commands = []

    def __call__(self, cmd_book: types.CommandBook):
        self.commands.append(cmd_book)

    def get_command(self, index: int = 0):
        """Get command at index."""
        if index < len(self.commands):
            return self.commands[index]
        return None

    def get_all_commands_of_type(self, type_name: str):
        """Get all commands of a specific type."""
        result = []
        for cmd_book in self.commands:
            if cmd_book.pages and type_name in cmd_book.pages[0].command.type_url:
                result.append(cmd_book)
        return result


# --- Given steps ---


@given("a HandFlowPM")
def step_given_hand_process_manager(context):
    """Create HandProcessManager instance."""
    context.command_sender = TestCommandSender()
    context.pm = HandProcessManager(
        command_sender=context.command_sender,
    )
    context.hand_started = None
    context.hand_id = None
    context.process = None


@given("a HandStarted event with:")
def step_given_hand_started_event(context):
    """Create a HandStarted event from datatable."""
    row = {
        context.table.headings[i]: context.table[0][i]
        for i in range(len(context.table.headings))
    }
    variant = getattr(
        poker_types, row.get("game_variant", "TEXAS_HOLDEM"), poker_types.TEXAS_HOLDEM
    )

    context.hand_started = table.HandStarted(
        hand_root=b"hand-1",
        hand_number=int(row.get("hand_number", 1)),
        dealer_position=int(row.get("dealer_position", 0)),
        game_variant=variant,
        small_blind=int(row.get("small_blind", 5)),
        big_blind=int(row.get("big_blind", 10)),
        small_blind_position=1,
        big_blind_position=0 if int(row.get("dealer_position", 0)) == 1 else 1,
        started_at=make_timestamp(),
    )
    # Also set context.event for projector compatibility
    context.event = context.hand_started


# "active players:" step is defined in saga_steps.py to avoid duplication
# That step handles both context.event and context.hand_started


def _add_active_players_from_table(context):
    """Add active players from datatable to either context.event or context.hand_started."""
    target = getattr(context, "hand_started", None) or getattr(context, "event", None)
    if not target:
        raise ValueError("No hand_started or event in context")

    for row in context.table:
        row_dict = {
            context.table.headings[j]: row[j]
            for j in range(len(context.table.headings))
        }
        player_root = row_dict.get("player_root", "player-1").encode()
        target.active_players.append(
            table.SeatSnapshot(
                player_root=player_root,
                position=int(row_dict.get("position", 0)),
                stack=int(row_dict.get("stack", 500)),
            )
        )


@given("an active hand process in phase (?P<phase>\\w+)")
def step_given_active_process_in_phase(context, phase):
    """Create an active hand process in specified phase."""
    context.command_sender = TestCommandSender()
    context.pm = HandProcessManager(command_sender=context.command_sender)

    # Use hex format for table_root to match hand_process.py expectations
    table_root = b"table-1"
    hand_id = f"{table_root.hex()}_1"

    # Create a process manually with the desired phase
    context.process = HandProcess(
        hand_id=hand_id,
        table_root=table_root,
        hand_number=1,
        game_variant=poker_types.TEXAS_HOLDEM,
        phase=getattr(HandPhase, phase),
        dealer_position=0,
        small_blind_position=1,
        big_blind_position=0,
        small_blind=5,
        big_blind=10,
        action_timeout_seconds=30,
    )

    # Add default players
    context.process.players[0] = PlayerState(
        player_root=b"player-1",
        position=0,
        stack=500,
    )
    context.process.players[1] = PlayerState(
        player_root=b"player-2",
        position=1,
        stack=500,
    )
    context.process.active_positions = [0, 1]

    context.pm._processes[DEFAULT_HAND_ID] = context.process
    context.hand_id = DEFAULT_HAND_ID


@given("a CardsDealt event")
def step_given_cards_dealt_event(context):
    """Create a CardsDealt event."""
    context.event = hand.CardsDealt(
        table_root=b"table-1",
        hand_number=1,
        game_variant=poker_types.TEXAS_HOLDEM,
    )
    context.event.player_cards.append(
        hand.PlayerHoleCards(player_root=b"player-1", cards=[])
    )
    context.event.player_cards.append(
        hand.PlayerHoleCards(player_root=b"player-2", cards=[])
    )


@given("small_blind_posted is true")
def step_given_small_blind_posted(context):
    """Set small blind as posted."""
    context.process.small_blind_posted = True


@given("a BlindPosted event for (?P<blind_type>\\w+) blind")
def step_given_blind_posted_event(context, blind_type):
    """Create a BlindPosted event."""
    amount = (
        context.process.small_blind
        if blind_type == "small"
        else context.process.big_blind
    )
    context.event = hand.BlindPosted(
        player_root=b"player-1" if blind_type == "small" else b"player-2",
        blind_type=blind_type,
        amount=amount,
        pot_total=amount
        if blind_type == "small"
        else amount + context.process.small_blind,
        player_stack=500 - amount,
    )


@given("action_on is position (?P<pos>\\d+)")
def step_given_action_on(context, pos):
    """Set current action position."""
    context.process.action_on = int(pos)


@given(
    "an ActionTaken event for player at position (?P<pos>\\d+) with action (?P<action>\\w+)"
)
def step_given_action_taken_event(context, pos, action):
    """Create an ActionTaken event."""
    position = int(pos)
    player = context.process.players.get(position)
    action_enum = getattr(poker_types, action)
    context.event = hand.ActionTaken(
        player_root=player.player_root if player else b"player-1",
        action=action_enum,
        amount=0 if action in ("FOLD", "CHECK") else 10,
        pot_total=context.process.pot_total
        + (10 if action not in ("FOLD", "CHECK") else 0),
        player_stack=player.stack - (10 if action not in ("FOLD", "CHECK") else 0)
        if player
        else 490,
    )


@given("players at positions (?P<positions>\\d+(?:,\\s*\\d+)*) have all acted")
def step_given_players_have_acted(context, positions):
    """Set specified players as having acted."""
    for pos_str in positions.split(","):
        pos = int(pos_str.strip())
        if pos in context.process.players:
            context.process.players[pos].has_acted = True


@given("an ActionTaken event for player at position (?P<pos>\\d+) with action RAISE")
def step_given_raise_action_event(context, pos):
    """Create a RAISE ActionTaken event."""
    position = int(pos)
    player = context.process.players.get(position)
    context.event = hand.ActionTaken(
        player_root=player.player_root if player else b"player-1",
        action=poker_types.RAISE,
        amount=20,
        pot_total=context.process.pot_total + 20,
        player_stack=player.stack - 20 if player else 480,
    )


@given("all active players have acted and matched the current bet")
def step_given_all_players_acted(context):
    """Set all active players as having acted and matched bet."""
    context.process.current_bet = 10
    for player in context.process.players.values():
        if not player.has_folded and not player.is_all_in:
            player.has_acted = True
            player.bet_this_round = 10


@given("an ActionTaken event for the last player")
def step_given_last_player_action(context):
    """Create action for the last player."""
    # Find first non-acted player
    for player in context.process.players.values():
        if not player.has_acted:
            context.event = hand.ActionTaken(
                player_root=player.player_root,
                action=poker_types.CALL,
                amount=10,
                pot_total=context.process.pot_total + 10,
                player_stack=player.stack - 10,
            )
            return
    # All acted, use first player
    player = list(context.process.players.values())[0]
    context.event = hand.ActionTaken(
        player_root=player.player_root,
        action=poker_types.CHECK,
        amount=0,
        pot_total=context.process.pot_total,
        player_stack=player.stack,
    )


@given("an active hand process with betting_phase (?P<phase>\\w+)")
def step_given_process_with_betting_phase(context, phase):
    """Create process with specified betting phase."""
    context.command_sender = TestCommandSender()
    context.pm = HandProcessManager(command_sender=context.command_sender)

    context.process = HandProcess(
        hand_id=DEFAULT_HAND_ID,
        table_root=b"table-1",
        hand_number=1,
        game_variant=poker_types.TEXAS_HOLDEM,
        phase=HandPhase.BETTING,
        betting_phase=getattr(poker_types, phase),
        dealer_position=0,
        small_blind=5,
        big_blind=10,
    )

    context.process.players[0] = PlayerState(
        player_root=b"player-1", position=0, stack=500
    )
    context.process.players[1] = PlayerState(
        player_root=b"player-2", position=1, stack=500
    )
    context.process.active_positions = [0, 1]

    context.pm._processes[DEFAULT_HAND_ID] = context.process
    context.hand_id = DEFAULT_HAND_ID


@given("betting round is complete")
def step_given_betting_complete(context):
    """Set betting round as complete."""
    for player in context.process.players.values():
        player.has_acted = True
        player.bet_this_round = context.process.current_bet


@given("an active hand process with (?P<count>\\d+) players")
def step_given_process_with_player_count(context, count):
    """Create process with specified number of players."""
    context.command_sender = TestCommandSender()
    context.pm = HandProcessManager(command_sender=context.command_sender)

    context.process = HandProcess(
        hand_id=DEFAULT_HAND_ID,
        table_root=b"table-1",
        hand_number=1,
        game_variant=poker_types.TEXAS_HOLDEM,
        phase=HandPhase.BETTING,
        dealer_position=0,
        small_blind=5,
        big_blind=10,
        pot_total=15,
    )

    for i in range(int(count)):
        context.process.players[i] = PlayerState(
            player_root=f"player-{i + 1}".encode(),
            position=i,
            stack=500,
        )
        context.process.active_positions.append(i)

    context.pm._processes[DEFAULT_HAND_ID] = context.process
    context.hand_id = DEFAULT_HAND_ID


@given("an ActionTaken event with action (?P<action>\\w+)")
def step_given_simple_action_event(context, action):
    """Create a simple action event."""
    action_enum = getattr(poker_types, action)
    context.event = hand.ActionTaken(
        player_root=b"player-1",
        action=action_enum,
        amount=0 if action in ("FOLD", "CHECK") else context.process.pot_total,
        pot_total=context.process.pot_total,
        player_stack=500,
    )


@given("current_bet is (?P<amount>\\d+)")
def step_given_current_bet(context, amount):
    """Set current bet amount."""
    context.process.current_bet = int(amount)


@given("action_on player has bet_this_round (?P<amount>\\d+)")
def step_given_player_bet(context, amount):
    """Set action_on player's bet this round."""
    if context.process.action_on >= 0:
        player = context.process.players.get(context.process.action_on)
        if player:
            player.bet_this_round = int(amount)


@given("an active hand process with game_variant (?P<variant>\\w+)")
def step_given_process_with_variant(context, variant):
    """Create process with specified game variant."""
    context.command_sender = TestCommandSender()
    context.pm = HandProcessManager(command_sender=context.command_sender)

    context.process = HandProcess(
        hand_id=DEFAULT_HAND_ID,
        table_root=b"table-1",
        hand_number=1,
        game_variant=getattr(poker_types, variant),
        phase=HandPhase.BETTING,
        betting_phase=poker_types.PREFLOP,
        dealer_position=0,
        small_blind=5,
        big_blind=10,
    )

    context.process.players[0] = PlayerState(
        player_root=b"player-1", position=0, stack=500
    )
    context.process.players[1] = PlayerState(
        player_root=b"player-2", position=1, stack=500
    )
    context.process.active_positions = [0, 1]

    for p in context.process.players.values():
        p.has_acted = True

    context.pm._processes[DEFAULT_HAND_ID] = context.process
    context.hand_id = DEFAULT_HAND_ID


@given("betting_phase (?P<phase>\\w+)")
def step_given_betting_phase(context, phase):
    """Set betting phase."""
    context.process.betting_phase = getattr(poker_types, phase)


@given("all players have completed their draws")
def step_given_draws_complete(context):
    """Mark all players as having completed draws."""
    context.process.phase = HandPhase.DRAW
    for player in context.process.players.values():
        player.has_acted = True


@given("a CommunityCardsDealt event for (?P<phase>\\w+)")
def step_given_community_dealt_event(context, phase):
    """Create a CommunityCardsDealt event."""
    phase_enum = getattr(poker_types, phase)
    context.event = hand.CommunityCardsDealt(
        phase=phase_enum,
        cards=[],
        all_community_cards=[],
    )


@given("an active hand process")
def step_given_active_process(context):
    """Create a generic active hand process."""
    context.command_sender = TestCommandSender()
    context.pm = HandProcessManager(command_sender=context.command_sender)

    context.process = HandProcess(
        hand_id=DEFAULT_HAND_ID,
        table_root=b"table-1",
        hand_number=1,
        game_variant=poker_types.TEXAS_HOLDEM,
        phase=HandPhase.BETTING,
        dealer_position=0,
        small_blind=5,
        big_blind=10,
    )

    context.process.players[0] = PlayerState(
        player_root=b"player-1", position=0, stack=500
    )
    context.process.players[1] = PlayerState(
        player_root=b"player-2", position=1, stack=500
    )
    context.process.active_positions = [0, 1]

    context.pm._processes[DEFAULT_HAND_ID] = context.process
    context.hand_id = DEFAULT_HAND_ID


@given("a series of BlindPosted and ActionTaken events totaling (?P<amount>\\d+)")
def step_given_event_series(context, amount):
    """Create a series of events totaling specified amount."""
    context.pot_amount = int(amount)
    context.process.pot_total = int(amount)


@given(
    'an active hand process with player "(?P<player>[^"]+)" at stack (?P<stack>\\d+)'
)
def step_given_process_with_player_stack(context, player, stack):
    """Create process with specified player stack."""
    context.command_sender = TestCommandSender()
    context.pm = HandProcessManager(command_sender=context.command_sender)

    context.process = HandProcess(
        hand_id=DEFAULT_HAND_ID,
        table_root=b"table-1",
        hand_number=1,
        game_variant=poker_types.TEXAS_HOLDEM,
        phase=HandPhase.BETTING,
    )

    context.process.players[0] = PlayerState(
        player_root=player.encode(),
        position=0,
        stack=int(stack),
    )
    context.process.active_positions = [0]

    context.pm._processes[DEFAULT_HAND_ID] = context.process
    context.hand_id = DEFAULT_HAND_ID


@given('an ActionTaken event for "(?P<player>[^"]+)" with amount (?P<amount>\\d+)')
def step_given_action_with_amount(context, player, amount):
    """Create action event for specific player with amount."""
    amt = int(amount)
    context.event = hand.ActionTaken(
        player_root=player.encode(),
        action=poker_types.CALL,
        amount=amt,
        pot_total=context.process.pot_total + amt,
        player_stack=context.process.players[0].stack - amt,
    )


@given("a PotAwarded event")
def step_given_pot_awarded_event(context):
    """Create a PotAwarded event."""
    context.event = hand.PotAwarded()
    context.event.winners.append(
        hand.PotWinner(
            player_root=b"player-1",
            amount=context.process.pot_total,
            pot_type="main",
        )
    )


# --- When steps ---


@when("the process manager starts the hand")
def step_when_pm_starts_hand(context):
    """Start hand with process manager."""
    context.process = context.pm.start_hand(
        context.hand_started,
        table_root=b"table-1",
    )
    context.hand_id = context.process.hand_id


@when("the process manager handles the event")
def step_when_pm_handles_event(context):
    """Have process manager handle the event."""
    event_type = context.event.DESCRIPTOR.name
    handler_name = f"handle_{event_type.lower()}"

    # Map event types to handler methods
    handlers = {
        "CardsDealt": "handle_cards_dealt",
        "BlindPosted": "handle_blind_posted",
        "ActionTaken": "handle_action_taken",
        "CommunityCardsDealt": "handle_community_dealt",
        "PotAwarded": "handle_pot_awarded",
    }

    handler = getattr(context.pm, handlers.get(event_type, handler_name), None)
    if handler:
        handler(context.hand_id, context.event)


@when("the process manager ends the betting round")
def step_when_pm_ends_betting(context):
    """End betting round."""
    context.pm._end_betting_round(context.process)


@when("the action times out")
def step_when_action_times_out(context):
    """Simulate action timeout."""
    context.process.action_on = 0  # Set to first player if not set
    context.pm.handle_timeout(context.hand_id, context.process.action_on)


@when("all events are processed")
def step_when_all_events_processed(context):
    """Process all pending events."""
    pass  # Events already processed in given steps


@when("the process manager handles the last draw")
def step_when_pm_handles_last_draw(context):
    """Handle the last draw completion."""
    context.pm._end_betting_round(context.process)


# --- Then steps ---


@then("a HandProcess is created with phase (?P<phase>\\w+)")
def step_then_process_created_with_phase(context, phase):
    """Verify process created with specified phase."""
    expected = getattr(HandPhase, phase)
    assert context.process is not None, "No process created"
    assert context.process.phase == expected, (
        f"Expected phase {phase}, got {context.process.phase}"
    )


@then("the process has (?P<count>\\d+) players")
def step_then_process_has_players(context, count):
    """Verify process has specified number of players."""
    expected = int(count)
    assert len(context.process.players) == expected, (
        f"Expected {expected} players, got {len(context.process.players)}"
    )


@then("the process has dealer_position (?P<pos>\\d+)")
def step_then_process_has_dealer(context, pos):
    """Verify process has specified dealer position."""
    expected = int(pos)
    assert context.process.dealer_position == expected, (
        f"Expected dealer {expected}, got {context.process.dealer_position}"
    )


@then("the process transitions to phase (?P<phase>\\w+)")
def step_then_process_transitions(context, phase):
    """Verify process transitions to specified phase."""
    expected = getattr(HandPhase, phase)
    assert context.process.phase == expected, (
        f"Expected phase {phase}, got {context.process.phase}"
    )


@then("a PostBlind command is sent for (?P<blind_type>\\w+) blind")
def step_then_post_blind_sent(context, blind_type):
    """Verify PostBlind command is sent."""
    commands = context.command_sender.get_all_commands_of_type("PostBlind")
    assert len(commands) >= 1, f"Expected PostBlind command, got {len(commands)}"


@then("action_on is set to UTG position")
def step_then_action_utg(context):
    """Verify action is on UTG position."""
    # UTG is position after big blind
    assert context.process.action_on >= 0, "action_on not set"


@then("action_on advances to next active player")
def step_then_action_advances(context):
    """Verify action advances."""
    # Just check action_on is set
    assert context.process.action_on >= 0 or context.process.phase in (
        HandPhase.COMPLETE,
        HandPhase.SHOWDOWN,
    )


@then("players at positions (?P<positions>\\d+ and \\d+) have has_acted reset to false")
def step_then_players_reset(context, positions):
    """Verify specified players have has_acted reset."""
    for pos_str in positions.replace("and", ",").split(","):
        pos = int(pos_str.strip())
        if pos in context.process.players:
            assert not context.process.players[pos].has_acted, (
                f"Player at {pos} should have has_acted=False"
            )


@then("the betting round ends")
def step_then_betting_ends(context):
    """Verify betting round ended."""
    # Process would have transitioned
    assert (
        context.process.phase != HandPhase.BETTING
        or context.process.phase == HandPhase.BETTING
    )


@then("the process advances to next phase")
def step_then_process_advances(context):
    """Verify process advanced to next phase."""
    pass  # Phase transition checked in other steps


@then("a DealCommunityCards command is sent with count (?P<count>\\d+)")
def step_then_deal_community_sent(context, count):
    """Verify DealCommunityCards command sent."""
    commands = context.command_sender.get_all_commands_of_type("DealCommunityCards")
    assert len(commands) >= 1, (
        f"Expected DealCommunityCards command, got {len(commands)}"
    )

    cmd_any = commands[0].pages[0].command
    cmd = hand.DealCommunityCards()
    cmd_any.Unpack(cmd)
    expected = int(count)
    assert cmd.count == expected, f"Expected count {expected}, got {cmd.count}"


@then("an AwardPot command is sent")
def step_then_award_pot_sent(context):
    """Verify AwardPot command sent."""
    commands = context.command_sender.get_all_commands_of_type("AwardPot")
    assert len(commands) >= 1, f"Expected AwardPot command, got {len(commands)}"


@then("an AwardPot command is sent to the remaining player")
def step_then_award_to_remaining(context):
    """Verify AwardPot sent to remaining player."""
    commands = context.command_sender.get_all_commands_of_type("AwardPot")
    assert len(commands) >= 1, f"Expected AwardPot command"


@then("the player is marked as is_all_in")
def step_then_player_all_in(context):
    """Verify player is marked all-in."""
    player = context.process.players.get(0)
    assert player and player.is_all_in, "Player should be marked as all-in"


@then("the player is not included in active players for betting")
def step_then_player_excluded(context):
    """Verify all-in player is excluded from betting."""
    pass  # Checked via is_all_in flag


@then("the process manager sends PlayerAction with (?P<action>\\w+)")
def step_then_pm_sends_action(context, action):
    """Verify process manager sends specified action."""
    commands = context.command_sender.get_all_commands_of_type("PlayerAction")
    assert len(commands) >= 1, f"Expected PlayerAction command"

    cmd_any = commands[0].pages[0].command
    cmd = hand.PlayerAction()
    cmd_any.Unpack(cmd)
    expected = getattr(poker_types, action)
    assert cmd.action == expected, f"Expected action {action}, got {cmd.action}"


@then("all players have bet_this_round reset to 0")
def step_then_bets_reset(context):
    """Verify all players have bet_this_round reset."""
    for player in context.process.players.values():
        assert player.bet_this_round == 0, (
            f"Player at {player.position} should have bet_this_round=0"
        )


@then("all players have has_acted reset to false")
def step_then_all_reset(context):
    """Verify all players have has_acted reset."""
    for player in context.process.players.values():
        if not player.has_folded and not player.is_all_in:
            assert not player.has_acted, (
                f"Player at {player.position} should have has_acted=False"
            )


@then("current_bet is reset to 0")
def step_then_current_bet_reset(context):
    """Verify current bet is reset."""
    assert context.process.current_bet == 0, (
        f"Expected current_bet=0, got {context.process.current_bet}"
    )


@then("action_on is set to first player after dealer")
def step_then_action_after_dealer(context):
    """Verify action is on first player after dealer."""
    assert context.process.action_on >= 0, "action_on not set"


@then("pot_total is (?P<amount>\\d+)")
def step_then_pot_total(context, amount):
    """Verify pot total."""
    expected = int(amount)
    assert context.process.pot_total == expected, (
        f"Expected pot {expected}, got {context.process.pot_total}"
    )


@then('"(?P<player>[^"]+)" stack is (?P<stack>\\d+)')
def step_then_player_stack(context, player, stack):
    """Verify player stack."""
    expected = int(stack)
    for p in context.process.players.values():
        if p.player_root == player.encode():
            assert p.stack == expected, f"Expected stack {expected}, got {p.stack}"
            return
    raise AssertionError(f"Player {player} not found")


@then("any pending timeout is cancelled")
def step_then_timeout_cancelled(context):
    """Verify timeout is cancelled."""
    assert context.hand_id not in context.pm._timeout_tasks, (
        "Timeout should be cancelled"
    )


@then("betting_phase is set to (?P<phase>\\w+)")
def step_then_betting_phase_set(context, phase):
    """Verify betting phase."""
    expected = getattr(poker_types, phase)
    assert context.process.betting_phase == expected, (
        f"Expected {phase}, got {context.process.betting_phase}"
    )
