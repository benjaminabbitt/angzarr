"""Projector: Output (OO Pattern)

Subscribes to player, table, and hand domain events.
Writes formatted game logs to a file.

This is the OO-style implementation using Projector base class with
@projects decorated methods. Contrasts with prj-output/ which uses
the functional pattern with explicit event type mapping.
"""

import os
from datetime import datetime

from angzarr_client import Projector, projects, run_projector_server
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table


_log_file = None


def get_log_file():
    """Get or create log file handle."""
    global _log_file
    if _log_file is None:
        path = os.environ.get("HAND_LOG_FILE", "hand_log_oo.txt")
        _log_file = open(path, "a")
    return _log_file


def write_log(msg: str) -> None:
    """Write timestamped message to log file."""
    f = get_log_file()
    timestamp = datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%S.%f")[:-3]
    f.write(f"[{timestamp}] {msg}\n")
    f.flush()


def truncate_id(player_root: bytes) -> str:
    """Truncate player root to first 8 hex chars."""
    return player_root[:4].hex() if len(player_root) >= 4 else player_root.hex()


# docs:start:projector_oo
class OutputProjector(Projector):
    """Output projector using OO-style decorators."""

    name = "output"
    input_domains = ["player", "table", "hand"]

    @projects(player.PlayerRegistered)
    def project_registered(self, event: player.PlayerRegistered) -> types.Projection:
        write_log(f"PLAYER registered: {event.display_name} ({event.email})")
        return types.Projection(projector=self.name)

    @projects(player.FundsDeposited)
    def project_deposited(self, event: player.FundsDeposited) -> types.Projection:
        amount = event.amount.amount if event.HasField("amount") else 0
        new_balance = event.new_balance.amount if event.HasField("new_balance") else 0
        write_log(f"PLAYER deposited {amount}, balance: {new_balance}")
        return types.Projection(projector=self.name)

    @projects(table.TableCreated)
    def project_table_created(self, event: table.TableCreated) -> types.Projection:
        write_log(f"TABLE created: {event.table_name} ({event.game_variant})")
        return types.Projection(projector=self.name)

    @projects(table.PlayerJoined)
    def project_player_joined(self, event: table.PlayerJoined) -> types.Projection:
        player_id = truncate_id(event.player_root)
        write_log(f"TABLE player {player_id} joined with {event.stack} chips")
        return types.Projection(projector=self.name)

    @projects(table.HandStarted)
    def project_hand_started(self, event: table.HandStarted) -> types.Projection:
        write_log(
            f"TABLE hand #{event.hand_number} started, "
            f"{len(event.active_players)} players, dealer at position {event.dealer_position}"
        )
        return types.Projection(projector=self.name)

    @projects(hand.CardsDealt)
    def project_cards_dealt(self, event: hand.CardsDealt) -> types.Projection:
        write_log(f"HAND cards dealt to {len(event.player_cards)} players")
        return types.Projection(projector=self.name)

    @projects(hand.BlindPosted)
    def project_blind_posted(self, event: hand.BlindPosted) -> types.Projection:
        player_id = truncate_id(event.player_root)
        write_log(f"HAND player {player_id} posted {event.blind_type} blind: {event.amount}")
        return types.Projection(projector=self.name)

    @projects(hand.ActionTaken)
    def project_action_taken(self, event: hand.ActionTaken) -> types.Projection:
        player_id = truncate_id(event.player_root)
        write_log(f"HAND player {player_id}: {event.action} {event.amount}")
        return types.Projection(projector=self.name)

    @projects(hand.PotAwarded)
    def project_pot_awarded(self, event: hand.PotAwarded) -> types.Projection:
        winners = [
            f"{truncate_id(w.player_root)} wins {w.amount}" for w in event.winners
        ]
        write_log(f"HAND pot awarded: {', '.join(winners)}")
        return types.Projection(projector=self.name)

    @projects(hand.HandComplete)
    def project_hand_complete(self, event: hand.HandComplete) -> types.Projection:
        write_log(f"HAND #{event.hand_number} complete")
        return types.Projection(projector=self.name)


# docs:end:projector_oo


def main():
    """Run the output projector server."""
    # Clear log file at startup
    path = os.environ.get("HAND_LOG_FILE", "hand_log_oo.txt")
    if os.path.exists(path):
        os.remove(path)

    print("Starting Output projector (OO pattern)")
    run_projector_server("output", 50391, OutputProjector.handle)


if __name__ == "__main__":
    main()
