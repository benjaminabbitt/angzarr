#!/usr/bin/env -S uv run --no-project
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "qdrant-client[fastembed]>=1.12",
# ]
# ///
"""Index the angzarr codebase into a local Qdrant vector database.

Walks the repository, chunks source files adaptively, and stores them with
embeddings in a local Qdrant instance for semantic search via the MCP server.

Usage:
    index_codebase.py [--path REPO_ROOT] [--collection NAME] [--db-path PATH]
"""

import argparse
import os
import re
import subprocess
import sys
import time
from pathlib import Path

from qdrant_client import QdrantClient, models

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

EXTENSIONS = {
    ".rs": "rust",
    ".py": "python",
    ".go": "go",
    ".proto": "protobuf",
    ".feature": "gherkin",
    ".yaml": "yaml",
    ".yml": "yaml",
    ".toml": "toml",
    ".md": "markdown",
}

EXCLUDE_DIRS = {
    "target",
    ".venv",
    "node_modules",
    ".git",
    ".vectors",
    "generated",
    "__pycache__",
    ".ruff_cache",
    ".gradle",
    ".terraform",
    "build",
    "obj",
    "bin",
    ".docusaurus",
    "docs/node_modules",
    "docs/build",
}

EXCLUDE_PATTERNS = [
    # Generated proto code in example language directories
    re.compile(r"examples/python/.*/angzarr/"),
    re.compile(r"examples/python/.*/proto/"),
    re.compile(r"examples/go/.*/proto/"),
    re.compile(r"examples/go/proto/"),
    re.compile(r"examples/python/proto/"),
    re.compile(r"examples/csharp/proto/"),
    re.compile(r"examples/kotlin/proto/"),
    re.compile(r"examples/typescript/proto/"),
    re.compile(r"examples/ruby/proto/"),
    re.compile(r"examples/java/proto/"),
    # Lock files
    re.compile(r".*\.lock$"),
    re.compile(r"Cargo\.lock$"),
]

EMBEDDING_MODEL = "sentence-transformers/all-MiniLM-L6-v2"
CHUNK_THRESHOLD = 150  # lines — files larger than this get chunked
CHUNK_SIZE = 100  # lines per chunk
CHUNK_OVERLAP = 20  # overlap between chunks

# ---------------------------------------------------------------------------
# Chunking
# ---------------------------------------------------------------------------


def chunk_lines(
    lines: list[str], chunk_size: int, overlap: int
) -> list[tuple[int, list[str]]]:
    """Split lines into overlapping chunks, returning (start_line, chunk_lines)."""
    chunks = []
    start = 0
    while start < len(lines):
        end = min(start + chunk_size, len(lines))
        chunks.append((start, lines[start:end]))
        if end >= len(lines):
            break
        start += chunk_size - overlap
    return chunks


def chunk_file(content: str) -> list[dict]:
    """Adaptively chunk a file.  Returns list of {text, chunk_index, total_chunks, start_line}."""
    lines = content.split("\n")

    if len(lines) <= CHUNK_THRESHOLD:
        return [{"text": content, "chunk_index": 0, "total_chunks": 1, "start_line": 1}]

    raw_chunks = chunk_lines(lines, CHUNK_SIZE, CHUNK_OVERLAP)
    total = len(raw_chunks)
    return [
        {
            "text": "\n".join(chunk),
            "chunk_index": i,
            "total_chunks": total,
            "start_line": start + 1,  # 1-indexed
        }
        for i, (start, chunk) in enumerate(raw_chunks)
    ]


# ---------------------------------------------------------------------------
# File discovery
# ---------------------------------------------------------------------------


def should_exclude(rel_path: str) -> bool:
    """Check if a relative path should be excluded."""
    parts = Path(rel_path).parts
    for part in parts:
        if part in EXCLUDE_DIRS:
            return True
    for pattern in EXCLUDE_PATTERNS:
        if pattern.search(rel_path):
            return True
    return False


def discover_files(repo_root: Path) -> list[Path]:
    """Walk the repo and return indexable files."""
    files = []
    for ext in EXTENSIONS:
        for path in repo_root.rglob(f"*{ext}"):
            rel = path.relative_to(repo_root)
            if should_exclude(str(rel)):
                continue
            if not path.is_file():
                continue
            files.append(path)
    return sorted(files)


# ---------------------------------------------------------------------------
# Component type detection
# ---------------------------------------------------------------------------

COMPONENT_PATTERNS = [
    (re.compile(r"(^|/)agg/"), "aggregate"),
    (re.compile(r"(^|/)saga-"), "saga"),
    (re.compile(r"(^|/)prj-"), "projector"),
    (re.compile(r"(^|/)pmg-"), "process-manager"),
    (re.compile(r"(^|/)e2e/"), "test"),
    (re.compile(r"(^|/)tests?/"), "test"),
    (re.compile(r"(^|/)proto/"), "proto"),
    (re.compile(r"(^|/)deploy/"), "deploy"),
    (re.compile(r"(^|/)scripts/"), "script"),
    (re.compile(r"(^|/)client/"), "client"),
    (re.compile(r"(^|/)src/"), "framework"),
]


def detect_component_type(rel_path: str) -> str:
    """Detect the angzarr component type from the file path."""
    for pattern, component in COMPONENT_PATTERNS:
        if pattern.search(rel_path):
            return component
    return "other"


def detect_domain(rel_path: str) -> str:
    """Try to detect the business domain from the file path."""
    # examples/{lang}/{domain}/...
    m = re.match(r"examples/\w+/(\w+)/", rel_path)
    if m:
        name = m.group(1)
        # Filter out non-domain directories
        if name not in {"e2e", "tests", "proto", "build"}:
            return name
    return ""


# ---------------------------------------------------------------------------
# Indexing
# ---------------------------------------------------------------------------


def index_codebase(repo_root: Path, collection_name: str, db_path: str) -> int:
    """Index all discovered files into Qdrant.  Returns document count."""
    db_abs = str((repo_root / db_path).resolve())
    client = QdrantClient(path=db_abs)
    client.set_model(EMBEDDING_MODEL)

    # Recreate collection
    if client.collection_exists(collection_name):
        client.delete_collection(collection_name)
        print(f"  Deleted existing collection '{collection_name}'")

    files = discover_files(repo_root)
    print(f"  Found {len(files)} files to index")

    documents: list[str] = []
    metadata: list[dict] = []
    ids: list[int] = []
    doc_id = 0

    for path in files:
        rel = str(path.relative_to(repo_root))
        lang = EXTENSIONS.get(path.suffix, "unknown")
        component = detect_component_type(rel)
        domain = detect_domain(rel)

        try:
            content = path.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:
            print(f"  Warning: could not read {rel}: {exc}", file=sys.stderr)
            continue

        if not content.strip():
            continue

        chunks = chunk_file(content)

        for chunk in chunks:
            # Build a rich document string: path header + content
            header = f"# {rel}"
            if chunk["total_chunks"] > 1:
                header += f" (chunk {chunk['chunk_index'] + 1}/{chunk['total_chunks']}, line {chunk['start_line']})"
            doc_text = f"{header}\n{chunk['text']}"

            documents.append(doc_text)
            metadata.append(
                {
                    "file_path": rel,
                    "language": lang,
                    "component_type": component,
                    "domain": domain,
                    "chunk_index": chunk["chunk_index"],
                    "total_chunks": chunk["total_chunks"],
                    "start_line": chunk["start_line"],
                }
            )
            ids.append(doc_id)
            doc_id += 1

    if not documents:
        print("  No documents to index")
        return 0

    # Batch add — qdrant-client handles embedding + collection creation via fastembed
    import warnings

    batch_size = 64
    with warnings.catch_warnings():
        warnings.filterwarnings("ignore", message="`add` method has been deprecated")
        for i in range(0, len(documents), batch_size):
            batch_end = min(i + batch_size, len(documents))
            client.add(
                collection_name=collection_name,
                documents=documents[i:batch_end],
                metadata=metadata[i:batch_end],
                ids=ids[i:batch_end],
            )
        if batch_end < len(documents):
            print(f"  Indexed {batch_end}/{len(documents)} chunks...")

    return len(documents)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def find_repo_root() -> Path:
    """Find the git repo root."""
    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return Path(result.stdout.strip())
    return Path.cwd()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Index the angzarr codebase into a local Qdrant vector database."
    )
    parser.add_argument(
        "--path",
        default=None,
        help="Repository root (default: git root)",
    )
    parser.add_argument(
        "--collection",
        default="angzarr-codebase",
        help="Qdrant collection name (default: angzarr-codebase)",
    )
    parser.add_argument(
        "--db-path",
        default=".vectors/qdrant",
        help="Path to local Qdrant database relative to repo root (default: .vectors/qdrant)",
    )
    args = parser.parse_args()

    repo_root = Path(args.path) if args.path else find_repo_root()
    if not repo_root.is_dir():
        print(f"Error: {repo_root} is not a directory", file=sys.stderr)
        return 1

    print(f"Indexing codebase at {repo_root}")
    print(f"  Collection: {args.collection}")
    print(f"  Database: {repo_root / args.db_path}")

    start = time.monotonic()
    count = index_codebase(repo_root, args.collection, args.db_path)
    elapsed = time.monotonic() - start

    print(f"  Indexed {count} chunks in {elapsed:.1f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
