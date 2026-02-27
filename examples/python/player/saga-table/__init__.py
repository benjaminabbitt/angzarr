"""Player-to-Table saga for fact propagation.

Translates player intent events to table facts:
- PlayerSittingOut (player) → PlayerSatOut fact (table)
- PlayerReturningToPlay (player) → PlayerSatIn fact (table)
"""

from .saga import PlayerTableSaga

__all__ = ["PlayerTableSaga"]
