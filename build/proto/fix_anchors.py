#!/usr/bin/env python3
"""
Post-process protoc-gen-doc markdown to use Docusaurus-compatible heading IDs.

Converts:
    <a name="angzarr-BusinessResponse"></a>

    ### BusinessResponse

To:
    ### BusinessResponse {#angzarr-BusinessResponse}

Also handles:
- Scalar type links (#bool, #string, etc.) → removed (browser resolves inline anchors)
- google.protobuf links → external documentation URLs

This fixes Docusaurus broken anchor warnings since <a name=""> isn't recognized
as valid anchor targets during build-time validation.
"""

import re
import sys
from pathlib import Path

# Map scalar type names to their anchor (no fix needed - inline anchors work at runtime)
SCALAR_TYPES = {
    "double",
    "float",
    "int32",
    "int64",
    "uint32",
    "uint64",
    "sint32",
    "sint64",
    "fixed32",
    "fixed64",
    "sfixed32",
    "sfixed64",
    "bool",
    "string",
    "bytes",
}

# Map google.protobuf types to external documentation
GOOGLE_PROTOBUF_DOCS = {
    "google-protobuf-Any": "https://protobuf.dev/reference/protobuf/google.protobuf/#any",
    "google-protobuf-Empty": "https://protobuf.dev/reference/protobuf/google.protobuf/#empty",
    "google-protobuf-Timestamp": "https://protobuf.dev/reference/protobuf/google.protobuf/#timestamp",
}


def fix_heading_anchors(content: str) -> str:
    """Convert <a name=""> anchors to Docusaurus heading IDs."""
    lines = content.split("\n")
    result = []
    pending_anchor = None

    for line in lines:
        # Check for <a name="..."></a> pattern (standalone anchor tags)
        anchor_match = re.match(r'^<a name="([^"]+)"></a>$', line.strip())

        if anchor_match:
            pending_anchor = anchor_match.group(1)
            # Don't output the anchor line - we'll add ID to heading instead
            continue

        # Check if this is a heading and we have a pending anchor
        if pending_anchor:
            heading_match = re.match(r"^(#{1,6})\s+(.+)$", line)
            if heading_match:
                level = heading_match.group(1)
                title = heading_match.group(2)
                # Add the anchor ID to the heading
                line = f"{level} {title} {{#{pending_anchor}}}"
                pending_anchor = None
            elif line.strip() and not line.strip().startswith("<p align"):
                # Non-empty, non-heading, non-paragraph line - something unexpected
                # Keep the anchor as-is for safety (shouldn't happen)
                result.append(f'<a name="{pending_anchor}"></a>')
                pending_anchor = None

        result.append(line)

    # Handle any remaining pending anchor
    if pending_anchor:
        result.append(f'<a name="{pending_anchor}"></a>')

    return "\n".join(result)


def fix_external_links(content: str) -> str:
    """Fix links to google.protobuf types to point to external docs."""
    for anchor, url in GOOGLE_PROTOBUF_DOCS.items():
        # Replace internal anchor links with external URLs
        # Pattern: [google.protobuf.Any](#google-protobuf-Any)
        # Becomes: [google.protobuf.Any](https://protobuf.dev/...)
        pattern = rf"\(#{re.escape(anchor)}\)"
        replacement = f"({url})"
        content = re.sub(pattern, replacement, content)
    return content


def fix_scalar_links(content: str) -> str:
    """Remove anchor from scalar type links - they work at runtime via inline <a name>.

    The inline <a name="bool" /> in table cells work in browsers but Docusaurus
    doesn't recognize them during build. We keep the display text but remove the link.
    """
    for scalar in SCALAR_TYPES:
        # Pattern: [bool](#bool) → bool
        pattern = rf"\[({scalar})\]\(#{scalar}\)"
        content = re.sub(pattern, r"\1", content)
    return content


def fix_anchors(content: str) -> str:
    """Apply all anchor fixes."""
    content = fix_heading_anchors(content)
    content = fix_external_links(content)
    content = fix_scalar_links(content)
    return content


def main():
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <file.md>", file=sys.stderr)
        sys.exit(1)

    filepath = Path(sys.argv[1])
    if not filepath.exists():
        print(f"Error: {filepath} not found", file=sys.stderr)
        sys.exit(1)

    content = filepath.read_text()
    fixed = fix_anchors(content)
    filepath.write_text(fixed)
    print(f"Fixed anchors in {filepath}")


if __name__ == "__main__":
    main()
