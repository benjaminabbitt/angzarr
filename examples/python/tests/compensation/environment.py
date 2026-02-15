"""Behave environment configuration for compensation tests."""

import sys
from pathlib import Path

# Add project root
root = Path(__file__).parent.parent.parent
sys.path.insert(0, str(root))


def before_scenario(context, scenario):
    """Reset context before each scenario."""
    context.events = []
    context.revoke_cmd = None
    context.response = None
    context.aggregate = None
    context.pm = None
    context.router = None
    context.routed_to = []
    context.called_handlers = []
