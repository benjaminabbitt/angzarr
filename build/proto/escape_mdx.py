#!/usr/bin/env python3
"""Escape curly braces in MDX files that aren't inside fenced code blocks."""

import re
import sys


def escape_mdx_braces(filepath: str) -> None:
    with open(filepath, 'r') as f:
        content = f.read()

    lines = content.split('\n')
    result = []
    in_fence = False

    for line in lines:
        if line.startswith('```'):
            in_fence = not in_fence
            result.append(line)
        elif in_fence:
            result.append(line)
        else:
            # Escape unescaped curly braces (not already escaped)
            line = re.sub(r'(?<!\\){', r'\\{', line)
            line = re.sub(r'(?<!\\)}', r'\\}', line)
            result.append(line)

    with open(filepath, 'w') as f:
        f.write('\n'.join(result))


if __name__ == '__main__':
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} <filepath>", file=sys.stderr)
        sys.exit(1)
    escape_mdx_braces(sys.argv[1])
