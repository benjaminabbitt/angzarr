#!/usr/bin/env python3
"""Render documentation templates with repository introspection data.

Collects data from the repository (LOC counts, etc.) and renders
Mustache templates to generate documentation files.
"""

import argparse
import re
import sys
from pathlib import Path

try:
    import chevron
except ImportError:
    print("Error: chevron not installed. Run: uv add chevron", file=sys.stderr)
    sys.exit(1)


def count_loc_python(file_path: Path) -> int:
    """Count lines of code in a Python file, excluding blanks and comments."""
    loc = 0
    in_docstring = False
    docstring_char = None

    with open(file_path, "r", encoding="utf-8") as f:
        for line in f:
            stripped = line.strip()

            if in_docstring:
                if docstring_char in stripped:
                    in_docstring = False
                continue

            if not stripped:
                continue

            if stripped.startswith("#"):
                continue

            if stripped.startswith('"""') or stripped.startswith("'''"):
                docstring_char = stripped[:3]
                if stripped.count(docstring_char) >= 2:
                    continue
                in_docstring = True
                continue

            loc += 1

    return loc


def count_loc_go(file_path: Path) -> int:
    """Count lines of code in a Go file, excluding blanks and comments."""
    loc = 0
    in_block_comment = False

    with open(file_path, "r", encoding="utf-8") as f:
        for line in f:
            stripped = line.strip()

            if in_block_comment:
                if "*/" in stripped:
                    in_block_comment = False
                continue

            if not stripped:
                continue

            if stripped.startswith("//"):
                continue

            if stripped.startswith("/*"):
                if "*/" not in stripped:
                    in_block_comment = True
                continue

            loc += 1

    return loc


def count_loc_rust(file_path: Path) -> int:
    """Count lines of code in a Rust file, excluding blanks and comments."""
    loc = 0
    in_block_comment = False

    with open(file_path, "r", encoding="utf-8") as f:
        for line in f:
            stripped = line.strip()

            if in_block_comment:
                if "*/" in stripped:
                    in_block_comment = False
                continue

            if not stripped:
                continue

            if stripped.startswith("//"):
                continue

            if stripped.startswith("/*"):
                if "*/" not in stripped:
                    in_block_comment = True
                continue

            loc += 1

    return loc


def count_loc(file_path: Path) -> int:
    """Count lines of code based on file extension."""
    suffix = file_path.suffix.lower()
    if suffix == ".py":
        return count_loc_python(file_path)
    elif suffix == ".go":
        return count_loc_go(file_path)
    elif suffix == ".rs":
        return count_loc_rust(file_path)
    else:
        return 0


def count_scenarios(feature_path: Path) -> int:
    """Count the number of scenarios in a Gherkin feature file."""
    count = 0
    with open(feature_path, "r", encoding="utf-8") as f:
        for line in f:
            stripped = line.strip()
            if stripped.startswith("Scenario:") or stripped.startswith("Scenario Outline:"):
                count += 1
    return count


def get_feature_files(repo_root: Path) -> list[dict]:
    """Get all canonical feature files from examples/features/."""
    features_dir = repo_root / "examples" / "features"
    if not features_dir.exists():
        return []

    feature_files = []
    for f in sorted(features_dir.glob("*.feature")):
        domain = f.stem.replace("_", " ").title()
        scenarios = count_scenarios(f)
        feature_files.append({
            "domain": domain,
            "name": f.name,
            "scenarios": scenarios,
            "link": f"../examples/features/{f.name}",
        })

    return feature_files


def get_example_loc(repo_root: Path, lang: str, domain: str) -> int:
    """Get total LOC for a specific example."""
    examples_dir = repo_root / "examples" / lang / domain
    if not examples_dir.exists():
        return 0

    total_loc = 0

    if lang == "go":
        logic_dir = examples_dir / "logic"
        if logic_dir.exists():
            for f in logic_dir.glob("*.go"):
                if "_test.go" not in f.name:
                    total_loc += count_loc(f)

    elif lang == "python":
        for pattern in ["*_logic.py", "state.py"]:
            for f in examples_dir.glob(pattern):
                if "test" not in f.name:
                    total_loc += count_loc(f)
        handlers_dir = examples_dir / "handlers"
        if handlers_dir.exists():
            for f in handlers_dir.glob("*.py"):
                if f.name != "__init__.py" and "test" not in f.name:
                    total_loc += count_loc(f)

    elif lang == "rust":
        src_dir = examples_dir / "src"
        if src_dir.exists():
            for f in src_dir.glob("*.rs"):
                total_loc += count_loc(f)

    return total_loc


def collect_data(repo_root: Path) -> dict:
    """Collect all introspection data from the repository."""
    data = {}

    # Customer example LOC for different languages
    customer_examples = []

    go_loc = get_example_loc(repo_root, "go", "customer")
    if go_loc > 0:
        customer_examples.append({
            "language": "Go",
            "loc": go_loc,
            "path": "examples/go/customer/logic/",
            "link": "../examples/go/customer/logic/",
        })

    python_loc = get_example_loc(repo_root, "python", "customer")
    if python_loc > 0:
        customer_examples.append({
            "language": "Python",
            "loc": python_loc,
            "path": "examples/python/customer/",
            "link": "../examples/python/customer/",
        })

    rust_loc = get_example_loc(repo_root, "rust", "customer")
    if rust_loc > 0:
        customer_examples.append({
            "language": "Rust",
            "loc": rust_loc,
            "path": "examples/rust/customer/src/",
            "link": "../examples/rust/customer/src/",
        })

    data["customer_examples"] = customer_examples

    # Feature files (Gherkin specs)
    data["feature_files"] = get_feature_files(repo_root)

    return data


def render_template(template_path: Path, data: dict) -> str:
    """Render a Mustache template with the given data."""
    with open(template_path, "r", encoding="utf-8") as f:
        template = f.read()

    return chevron.render(template, data)


def render_all_templates(repo_root: Path, dry_run: bool = False) -> list[tuple[Path, bool]]:
    """Render all templates in docs/templates/.

    Returns list of (output_path, was_modified) tuples.
    """
    templates_dir = repo_root / "docs" / "templates"
    if not templates_dir.exists():
        print(f"Templates directory not found: {templates_dir}", file=sys.stderr)
        return []

    data = collect_data(repo_root)
    results = []

    for template_path in templates_dir.glob("*.mustache"):
        # Output path: docs/templates/FOO.md.mustache -> docs/FOO.md
        output_name = template_path.stem  # e.g., "PITCH.md"
        output_path = repo_root / "docs" / output_name

        rendered = render_template(template_path, data)

        # Check if content changed
        existing_content = ""
        if output_path.exists():
            existing_content = output_path.read_text()

        was_modified = rendered != existing_content

        if dry_run:
            if was_modified:
                print(f"Would update: {output_path}")
            else:
                print(f"Up to date: {output_path}")
        else:
            if was_modified:
                output_path.write_text(rendered)
                print(f"Updated: {output_path}")
            else:
                print(f"Up to date: {output_path}")

        results.append((output_path, was_modified))

    return results


def main():
    parser = argparse.ArgumentParser(
        description="Render documentation templates with repository data"
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=Path(__file__).parent.parent,
        help="Repository root directory",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be updated without writing files",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit with error if any files would be modified (for CI)",
    )
    args = parser.parse_args()

    results = render_all_templates(args.repo_root, dry_run=args.dry_run or args.check)

    if args.check:
        modified = [path for path, was_modified in results if was_modified]
        if modified:
            print(f"\nError: {len(modified)} file(s) out of date. Run 'just docs' to update.")
            sys.exit(1)
        else:
            print("\nAll documentation up to date.")


if __name__ == "__main__":
    main()
