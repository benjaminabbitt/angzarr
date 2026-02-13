"""AI Sidecar gRPC client for PyTorch model inference."""

import sys
from pathlib import Path
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Optional
import os

import grpc

# Add path for proto imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.examples import ai_sidecar_pb2 as ai
from angzarr_client.proto.examples import ai_sidecar_pb2_grpc as ai_grpc
from angzarr_client.proto.examples import types_pb2 as types


@dataclass
class ActionDecision:
    """Result of an action decision."""

    action: int  # ActionType enum
    amount: int


class ActionDecider(ABC):
    """Abstract base class for action decision-making."""

    @abstractmethod
    async def decide_action(
        self,
        hole_cards: list,
        community_cards: list,
        pot_size: int,
        amount_to_call: int,
        min_raise: int,
        max_raise: int,
        phase: int,
        model_id: str = "",
    ) -> ActionDecision:
        """Decide on an action given the game state."""
        pass


class HumanActionDecider(ActionDecider):
    """Human player action decider.

    Humans don't decide via this interface - they submit actions via commands.
    This raises an error if called directly.
    """

    async def decide_action(self, **kwargs) -> ActionDecision:
        raise NotImplementedError(
            "Human players submit actions via PlayerAction command"
        )


class AiActionDecider(ActionDecider):
    """AI player action decider.

    Calls the AI sidecar service to get action decisions from PyTorch models.
    """

    def __init__(self, sidecar_url: str):
        self.sidecar_url = sidecar_url
        self._channel: Optional[grpc.aio.Channel] = None
        self._stub: Optional[ai_grpc.AiSidecarStub] = None

    @classmethod
    def from_env(cls) -> Optional["AiActionDecider"]:
        """Create from AI_SIDECAR_URL environment variable."""
        url = os.environ.get("AI_SIDECAR_URL")
        if url:
            return cls(url)
        return None

    async def _ensure_connected(self):
        """Ensure gRPC channel is connected."""
        if self._channel is None:
            self._channel = grpc.aio.insecure_channel(self.sidecar_url)
            self._stub = ai_grpc.AiSidecarStub(self._channel)

    async def decide_action(
        self,
        hole_cards: list,
        community_cards: list,
        pot_size: int,
        amount_to_call: int,
        min_raise: int,
        max_raise: int,
        phase: int,
        model_id: str = "",
    ) -> ActionDecision:
        """Get action decision from AI sidecar."""
        await self._ensure_connected()

        request = ai.ActionRequest(
            model_id=model_id,
            game_variant=types.GameVariant.TEXAS_HOLDEM,
            phase=phase,
            hole_cards=hole_cards,
            community_cards=community_cards,
            pot_size=pot_size,
            stack_size=max_raise,  # max_raise is typically the stack
            amount_to_call=amount_to_call,
            min_raise=min_raise,
            max_raise=max_raise,
        )

        response = await self._stub.GetAction(request)

        return ActionDecision(
            action=response.recommended_action,
            amount=response.amount,
        )

    async def close(self):
        """Close the gRPC channel."""
        if self._channel:
            await self._channel.close()
            self._channel = None
            self._stub = None


def get_decider(player_type: int, sidecar_url: Optional[str] = None) -> ActionDecider:
    """Get the appropriate action decider for a player type."""
    from angzarr_client.proto.examples import player_pb2 as player

    if player_type == player.PlayerType.AI:
        url = sidecar_url or os.environ.get("AI_SIDECAR_URL")
        if url:
            return AiActionDecider(url)

    return HumanActionDecider()
