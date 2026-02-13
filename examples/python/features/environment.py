"""Behave environment configuration."""

import sys
from pathlib import Path

# Add project paths
root = Path(__file__).parent.parent
sys.path.insert(0, str(root))
sys.path.insert(0, str(root / "prj-output"))
sys.path.insert(0, str(root / "hand-flow"))

# Add aggregate paths
for agg in ["agg-player", "agg-table", "agg-hand"]:
    sys.path.insert(0, str(root / agg))


def before_scenario(context, scenario):
    """Reset context before each scenario."""
    context.events = []
    context.output_lines = []
    context.cards_output = ""
    context.result = None
    context.error = None
    context.state = None
    context.commands_sent = []
