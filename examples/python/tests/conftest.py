"""Pytest configuration for integration tests."""

import os
import sys
from pathlib import Path

# Add generated proto path to sys.path
examples_python = Path(__file__).parent.parent
sys.path.insert(0, str(examples_python / "generated"))

import pytest


@pytest.fixture
def gateway_address() -> str:
    """Get the gateway address from environment or default."""
    return os.environ.get("ANGZARR_GATEWAY", "localhost:50051")


@pytest.fixture
def test_mode() -> str:
    """Get test mode from environment."""
    return os.environ.get("ANGZARR_TEST_MODE", "container")
