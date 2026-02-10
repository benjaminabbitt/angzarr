Write or update a README.md for this project targeting developers as the primary audience.

## Structure (in this order)

1. **Project Name & One-liner** - What it is in one sentence
2. **The Problem** - What pain points does this solve? Why should developers care?
3. **The Solution** - How does this tool address those problems?
4. **Quick Start** - Minimal steps to get running (install, configure, first command)
5. **Usage Examples** - Common use cases with concrete commands
6. **Configuration** - Key settings and customization options
7. **Development** - How to build, test, and contribute (for contributors)

## Do Not
- Put in a structure with file system/directory lists.  The user can read that on their own computer.
- Be verbose/add low-utility verbiage.

## When Updating an Existing README

- Preserve all existing content unless it conflicts with current functionality
- Remove or update sections describing changed or removed features
- Maintain the author's voice and style where possible
- Add new sections for new functionality
- Reorganize to match the structure above if needed

## Use Version Control History

- Review git log and diffs to identify recent changes to the codebase
- Look for new features, renamed commands, changed behavior
- Pay special attention to small README diffs - these are often human edits with important context or corrections
- Large README rewrites may be generated; small tweaks are likely intentional refinements
- Check commit messages for context on why changes were made

## Guidelines

- Lead with value: problems solved and benefits before technical details
- Be concise: developers skim READMEs, use bullets and code blocks
- Show, don't tell: prefer examples over descriptions
- Keep development/build instructions at the end - most users don't need them
- Include a "Why this tool?" section if similar tools exist
- Avoid marketing language; be direct and technical