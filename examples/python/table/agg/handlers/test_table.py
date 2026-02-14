"""Tests for Table aggregate."""

import pytest

from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import types_pb2 as poker_types

from .table import Table


class TestCreate:
    """Test Table.create()."""

    def test_create_table(self):
        t = Table()
        assert not t.exists

        cmd = table.CreateTable(
            table_name="High Stakes",
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            small_blind=50,
            big_blind=100,
            max_players=6,
        )
        event = t.create(cmd)

        assert t.exists
        assert t.table_name == "High Stakes"
        assert t.small_blind == 50
        assert t.big_blind == 100
        assert t.max_players == 6
        assert event.table_name == "High Stakes"

    def test_create_sets_defaults(self):
        t = Table()
        cmd = table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            max_players=6,
        )
        t.create(cmd)

        assert t.min_buy_in == 200  # 10 * 20
        assert t.max_buy_in == 1000  # 10 * 100

    def test_create_rejects_existing_table(self):
        t = Table()
        cmd = table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            max_players=6,
        )
        t.create(cmd)

        with pytest.raises(CommandRejectedError, match="already exists"):
            t.create(cmd)

    def test_create_requires_table_name(self):
        t = Table()
        cmd = table.CreateTable(small_blind=5, big_blind=10, max_players=6)

        with pytest.raises(CommandRejectedError, match="table_name"):
            t.create(cmd)

    def test_create_validates_blinds(self):
        t = Table()

        with pytest.raises(CommandRejectedError, match="small_blind"):
            t.create(table.CreateTable(
                table_name="Test", small_blind=0, big_blind=10, max_players=6
            ))

        t2 = Table()
        with pytest.raises(CommandRejectedError, match="big_blind must be >="):
            t2.create(table.CreateTable(
                table_name="Test", small_blind=20, big_blind=10, max_players=6
            ))


class TestJoin:
    """Test Table.join()."""

    def test_join_adds_player(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))

        player_root = b"\x01\x02\x03\x04"
        event = t.join(table.JoinTable(player_root=player_root, buy_in_amount=500))

        assert t.player_count == 1
        seat = t.find_player_seat(player_root)
        assert seat is not None
        assert seat.stack == 500
        assert event.seat_position == 0

    def test_join_with_preferred_seat(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))

        player_root = b"\x01"
        t.join(table.JoinTable(
            player_root=player_root,
            buy_in_amount=500,
            preferred_seat=3,
        ))

        assert t.get_seat(3) is not None
        assert t.get_seat(3).player_root == player_root

    def test_join_rejects_duplicate_player(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))

        player_root = b"\x01"
        t.join(table.JoinTable(player_root=player_root, buy_in_amount=500))

        with pytest.raises(CommandRejectedError, match="already seated"):
            t.join(table.JoinTable(player_root=player_root, buy_in_amount=500))

    def test_join_validates_buy_in(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))

        with pytest.raises(CommandRejectedError, match="at least"):
            t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=50))

        with pytest.raises(CommandRejectedError, match="cannot exceed"):
            t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=2000))


class TestLeave:
    """Test Table.leave()."""

    def test_leave_removes_player(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))

        player_root = b"\x01"
        t.join(table.JoinTable(player_root=player_root, buy_in_amount=500))
        assert t.player_count == 1

        event = t.leave(table.LeaveTable(player_root=player_root))

        assert t.player_count == 0
        assert t.find_player_seat(player_root) is None
        assert event.chips_cashed_out == 500

    def test_leave_rejects_unknown_player(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))

        with pytest.raises(CommandRejectedError, match="not seated"):
            t.leave(table.LeaveTable(player_root=b"\x99"))


class TestStartHand:
    """Test Table.start_hand()."""

    def test_start_hand_transitions_status(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))

        assert t.status == "waiting"
        event = t.start_hand(table.StartHand())

        assert t.status == "in_hand"
        assert t.hand_count == 1
        assert t.current_hand_root == event.hand_root

    def test_start_hand_requires_two_players(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))

        with pytest.raises(CommandRejectedError, match="Not enough players"):
            t.start_hand(table.StartHand())

    def test_start_hand_rejects_if_hand_in_progress(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        t.start_hand(table.StartHand())

        with pytest.raises(CommandRejectedError, match="already in progress"):
            t.start_hand(table.StartHand())


class TestEndHand:
    """Test Table.end_hand()."""

    def test_end_hand_transitions_to_waiting(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        start_event = t.start_hand(table.StartHand())

        t.end_hand(table.EndHand(hand_root=start_event.hand_root))

        assert t.status == "waiting"
        assert t.current_hand_root == b""

    def test_end_hand_validates_hand_root(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        t.start_hand(table.StartHand())

        with pytest.raises(CommandRejectedError, match="Hand root mismatch"):
            t.end_hand(table.EndHand(hand_root=b"\x99\x99\x99"))


class TestCompleteLifecycle:
    """Test complete table lifecycle."""

    def test_create_join_start_end_leave(self):
        t = Table()

        # Create
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))
        assert t.exists
        assert t.status == "waiting"

        # Player 1 joins
        player1 = b"\x01\x01\x01\x01"
        t.join(table.JoinTable(player_root=player1, buy_in_amount=500))
        assert t.player_count == 1

        # Player 2 joins
        player2 = b"\x02\x02\x02\x02"
        t.join(table.JoinTable(player_root=player2, buy_in_amount=500))
        assert t.player_count == 2

        # Start hand
        start_event = t.start_hand(table.StartHand())
        assert t.status == "in_hand"
        assert t.hand_count == 1

        # End hand
        t.end_hand(table.EndHand(hand_root=start_event.hand_root))
        assert t.status == "waiting"

        # Player 2 leaves
        t.leave(table.LeaveTable(player_root=player2))
        assert t.player_count == 1
        assert t.find_player_seat(player2) is None

    def test_event_book_has_all_events(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            small_blind=5,
            big_blind=10,
            min_buy_in=100,
            max_buy_in=1000,
            max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))

        eb = t.event_book()
        assert len(eb.pages) == 3  # create + 2 joins


class TestStateAccessors:
    """Test state accessor properties."""

    def test_table_id_format(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="High Stakes",
            small_blind=50,
            big_blind=100,
            max_players=6,
        ))
        assert t.table_id == "table_High Stakes"

    def test_game_variant_returns_enum(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test",
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            small_blind=5,
            big_blind=10,
            max_players=6,
        ))
        assert t.game_variant == poker_types.GameVariant.TEXAS_HOLDEM

    def test_dealer_position_after_hand(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        event = t.start_hand(table.StartHand())
        assert t.dealer_position == event.dealer_position

    def test_is_full_when_max_players_reached(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=2,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        assert not t.is_full
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        assert t.is_full

    def test_active_player_count_excludes_sitting_out(self):
        """active_player_count tested via start_hand minimum players check."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        # Build table with a sat-out player
        event_book = types.EventBook()

        # TableCreated
        created = table.TableCreated(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        )
        created_any = Any()
        created_any.Pack(created, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=created_any))

        # Two players joined
        for i, root in enumerate([b"\x01", b"\x02"]):
            joined = table.PlayerJoined(
                player_root=root, seat_position=i, stack=500,
            )
            joined_any = Any()
            joined_any.Pack(joined, type_url_prefix="type.googleapis.com/")
            event_book.pages.append(types.EventPage(event=joined_any))

        # Player 1 sat out
        sat_out = table.PlayerSatOut(player_root=b"\x01")
        sat_out_any = Any()
        sat_out_any.Pack(sat_out, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=sat_out_any))

        t = Table(event_book)
        assert t.player_count == 2
        assert t.active_player_count == 1


class TestEventHandlers:
    """Test event handlers for saga-generated events."""

    def test_player_sat_in_after_sat_out(self):
        """PlayerSatIn event restores player to active."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()

        # TableCreated
        created = table.TableCreated(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        )
        created_any = Any()
        created_any.Pack(created, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=created_any))

        # Player joined
        joined = table.PlayerJoined(
            player_root=b"\x01", seat_position=0, stack=500,
        )
        joined_any = Any()
        joined_any.Pack(joined, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=joined_any))

        # Player sat out
        sat_out = table.PlayerSatOut(player_root=b"\x01")
        sat_out_any = Any()
        sat_out_any.Pack(sat_out, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=sat_out_any))

        # Player sat back in
        sat_in = table.PlayerSatIn(player_root=b"\x01")
        sat_in_any = Any()
        sat_in_any.Pack(sat_in, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=sat_in_any))

        t = Table(event_book)
        seat = t.find_player_seat(b"\x01")
        assert seat is not None
        assert not seat.is_sitting_out

    def test_chips_added_updates_stack(self):
        """ChipsAdded event from re-buy/add-on."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()

        # TableCreated
        created = table.TableCreated(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        )
        created_any = Any()
        created_any.Pack(created, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=created_any))

        # Player joined with 500
        joined = table.PlayerJoined(
            player_root=b"\x01", seat_position=0, stack=500,
        )
        joined_any = Any()
        joined_any.Pack(joined, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=joined_any))

        # Chips added (re-buy)
        chips_added = table.ChipsAdded(player_root=b"\x01", new_stack=800)
        chips_any = Any()
        chips_any.Pack(chips_added, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=chips_any))

        t = Table(event_book)
        seat = t.find_player_seat(b"\x01")
        assert seat.stack == 800

    def test_hand_ended_updates_stacks(self):
        """HandEnded event applies stack_changes."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()

        # TableCreated
        created = table.TableCreated(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        )
        created_any = Any()
        created_any.Pack(created, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=created_any))

        # Two players joined
        for i, root in enumerate([b"\x01", b"\x02"]):
            joined = table.PlayerJoined(
                player_root=root, seat_position=i, stack=500,
            )
            joined_any = Any()
            joined_any.Pack(joined, type_url_prefix="type.googleapis.com/")
            event_book.pages.append(types.EventPage(event=joined_any))

        # HandStarted
        started = table.HandStarted(
            hand_root=b"\xaa\xbb\xcc", hand_number=1, dealer_position=0,
        )
        started_any = Any()
        started_any.Pack(started, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=started_any))

        # HandEnded with stack changes
        player1_hex = b"\x01".hex()
        player2_hex = b"\x02".hex()
        ended = table.HandEnded(
            hand_root=b"\xaa\xbb\xcc",
            stack_changes={player1_hex: 100, player2_hex: -100},
        )
        ended_any = Any()
        ended_any.Pack(ended, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=ended_any))

        t = Table(event_book)
        assert t.find_player_seat(b"\x01").stack == 600
        assert t.find_player_seat(b"\x02").stack == 400
        assert t.status == "waiting"


class TestEdgeCases:
    """Test edge cases for full coverage."""

    def test_create_validates_big_blind_positive(self):
        t = Table()
        with pytest.raises(CommandRejectedError, match="big_blind must be positive"):
            t.create(table.CreateTable(
                table_name="Test", small_blind=5, big_blind=0, max_players=6
            ))

    def test_create_validates_max_players_bounds(self):
        t = Table()
        with pytest.raises(CommandRejectedError, match="max_players must be between"):
            t.create(table.CreateTable(
                table_name="Test", small_blind=5, big_blind=10, max_players=1
            ))
        t2 = Table()
        with pytest.raises(CommandRejectedError, match="max_players must be between"):
            t2.create(table.CreateTable(
                table_name="Test", small_blind=5, big_blind=10, max_players=11
            ))

    def test_join_requires_table_exists(self):
        t = Table()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))

    def test_join_requires_player_root(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        with pytest.raises(CommandRejectedError, match="player_root"):
            t.join(table.JoinTable(buy_in_amount=500))

    def test_join_rejects_full_table(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=2,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))

        with pytest.raises(CommandRejectedError, match="Table is full"):
            t.join(table.JoinTable(player_root=b"\x03", buy_in_amount=500))

    def test_join_rejects_occupied_preferred_seat(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500, preferred_seat=3))

        with pytest.raises(CommandRejectedError, match="Seat is occupied"):
            t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500, preferred_seat=3))

    def test_leave_requires_table_exists(self):
        t = Table()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            t.leave(table.LeaveTable(player_root=b"\x01"))

    def test_leave_requires_player_root(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        with pytest.raises(CommandRejectedError, match="player_root"):
            t.leave(table.LeaveTable())

    def test_leave_rejects_during_hand(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        t.start_hand(table.StartHand())

        with pytest.raises(CommandRejectedError, match="Cannot leave table during a hand"):
            t.leave(table.LeaveTable(player_root=b"\x01"))

    def test_start_hand_requires_table_exists(self):
        t = Table()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            t.start_hand(table.StartHand())

    def test_end_hand_requires_table_exists(self):
        t = Table()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            t.end_hand(table.EndHand(hand_root=b"\x01"))

    def test_end_hand_requires_hand_in_progress(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        with pytest.raises(CommandRejectedError, match="No hand in progress"):
            t.end_hand(table.EndHand(hand_root=b"\x01"))

    def test_end_hand_with_results(self):
        """EndHand with pot results calculates stack_changes."""
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        start_event = t.start_hand(table.StartHand())

        # End with result showing player 1 won 100 from pot
        event = t.end_hand(table.EndHand(
            hand_root=start_event.hand_root,
            results=[table.PotResult(winner_root=b"\x01", amount=100)],
        ))

        assert event.stack_changes[b"\x01".hex()] == 100

    def test_start_hand_heads_up_blind_positions(self):
        """In heads-up, dealer posts small blind."""
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        event = t.start_hand(table.StartHand())

        # In 2-player game, dealer posts SB
        assert event.small_blind_position == event.dealer_position

    def test_start_hand_three_way_blind_positions(self):
        """In 3+ player, SB is left of dealer."""
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x03", buy_in_amount=500))
        event = t.start_hand(table.StartHand())

        # SB is to left of dealer
        assert event.small_blind_position != event.dealer_position

    def test_find_player_seat_not_found(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        assert t.find_player_seat(b"\x99") is None

    def test_get_seat_returns_none_for_empty(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        assert t.get_seat(5) is None

    def test_seats_property_returns_dict(self):
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        seats = t.seats
        assert isinstance(seats, dict)
        assert len(seats) == 1

    def test_find_available_seat_returns_none_when_full(self):
        """Test internal method _find_available_seat when table is full."""
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=2,
        ))
        t.join(table.JoinTable(player_root=b"\x01", buy_in_amount=500))
        t.join(table.JoinTable(player_root=b"\x02", buy_in_amount=500))
        # Directly call internal method
        assert t._find_available_seat() is None

    def test_next_dealer_position_empty_table(self):
        """Test internal method _next_dealer_position with no seats."""
        t = Table()
        t.create(table.CreateTable(
            table_name="Test", small_blind=5, big_blind=10,
            min_buy_in=100, max_buy_in=1000, max_players=6,
        ))
        # Directly call internal method on empty table
        assert t._next_dealer_position() == 0
