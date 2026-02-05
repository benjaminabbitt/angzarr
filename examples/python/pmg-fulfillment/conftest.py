"""Configure sys.path for angzarr library imports."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))
