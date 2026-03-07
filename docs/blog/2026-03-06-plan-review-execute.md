---
slug: plan-review-execute
title: "Plan, Review, Execute: Getting Better Results from LLMs"
authors: [angzarr]
tags: [llm, workflow, patterns, collaboration]
keywords: [llm, ai, planning, code review, workflow, collaboration, claude, gpt]
---

The most effective LLM workflows share one trait: they force a pause between planning and execution. You wouldn't let a contractor start demolition before approving blueprints. The same applies to AI assistants.

<!-- truncate -->

## The Problem with Eager Execution

LLMs are biased toward action. Given a task, they want to produce output immediately. This leads to:

- Implementations that don't match your mental model
- Refactoring that introduces patterns you don't want
- Solutions to problems you didn't actually have

The fix isn't more detailed prompts. It's workflow structure.

## Plan Mode: The First Checkpoint

Before any implementation, require a plan. Not pseudocode, not a summary of what the LLM intends to do. A concrete list of files to touch, functions to modify, and decisions that need your input.

The plan itself isn't the value. The **review** is.

## Why Review Matters

Plans expose assumptions. An LLM might assume you want bcrypt when you're using Argon2, or assume PostgreSQL when you're on SQLite. Catching this before code exists saves hours.

More importantly, plans surface questions the LLM should ask but often doesn't. "Should this be configurable?" and "What happens on failure?" are questions better asked before implementation than discovered during code review.

## The Walkthrough Pattern

For existing code, planning becomes reviewing. The walkthrough pattern structures this:

**1. One chunk at a time.** Present a single function, not an entire file. Small enough to reason about. Large enough to be meaningful.

**2. Explain non-obvious aspects.** What dependencies exist? What side effects occur? What assumptions are baked in?

**3. Question unusual patterns.** If something looks odd, say so. Don't just describe; interrogate. "This defaults to MergeCommutative but merge strategy seems like it should be configurable."

**4. Wait for approval.** Don't proceed until the human says to. Make changes before moving to the next chunk.

The interaction is simple:

1. AI presents a function with explanation
2. AI flags potential issues
3. AI asks: "Changes, or continue?"
4. Human responds
5. Repeat

## Status Tracking for Multi-Session Work

Long reviews span multiple sessions. Track progress with a simple status document:

- List of files/functions to review
- Checkmarks for completed items
- Notes on decisions made
- Questions to revisit

Keep this gitignored. It's session state, not documentation.

## When to Use This

**Codebase onboarding.** Walk through key files with an expert (human or AI) asking questions at each stop.

**Code review with approval gates.** When every change needs sign-off before the next.

**Refactoring sessions.** Make one change, verify it works, move to the next.

**Teaching and learning.** Slow pace with space for questions beats firehose explanations.

## The Core Insight

LLMs work best with feedback loops, not fire-and-forget prompts. Plan mode creates one checkpoint. Walkthrough creates many. Both share the same principle: you can't review what you haven't seen.

Build the pause into your workflow. The LLM will produce better work, and you'll catch problems before they become expensive.
