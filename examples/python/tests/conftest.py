"""Pytest configuration for integration tests."""

import os
import sys
from pathlib import Path

# Add generated proto path to sys.path
examples_python = Path(__file__).parent.parent
sys.path.insert(0, str(examples_python / "generated"))

import pytest


def get_angzarr_endpoint() -> str:
    """Get the Angzarr gateway endpoint from environment or default.

    Uses ANGZARR_ENDPOINT for full URL, or ANGZARR_HOST:ANGZARR_PORT for components.
    Standard env vars used across all languages/containers.
    """
    if endpoint := os.environ.get("ANGZARR_ENDPOINT"):
        return endpoint
    host = os.environ.get("ANGZARR_HOST", "localhost")
    port = os.environ.get("ANGZARR_PORT", "9084")
    return f"{host}:{port}"


@pytest.fixture
def gateway_address() -> str:
    """Get the gateway address from environment or default."""
    return get_angzarr_endpoint()
