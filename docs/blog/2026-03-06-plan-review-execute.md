---
slug: plan-review-execute
title: "Plan, Review, Execute: Getting Better Results from LLMs"
authors: [angzarr]
tags: [llm, workflow, patterns, collaboration]
keywords: [llm, ai, planning, code review, workflow, collaboration, claude, gpt, illuminated code walkthrough]
---

import BlogHeader from '@site/src/components/BlogHeader';

<BlogHeader />

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

## The Illuminated Code Walkthrough

For existing code, planning becomes reviewing. The **illuminated code walkthrough** applies the same checkpoint principle: AI narrates execution flow one step at a time while you read along, controlling the pace.

The interaction:

1. AI presents a function or handler with explanation
2. AI flags potential issues
3. AI asks: "Changes, or continue?"
4. Human responds
5. Repeat

This works especially well when tracing integration tests or application flows—you follow complete paths from entry point through all possible endings.

For a deeper treatment of illuminated walkthroughs and how they fit with test-driven development, see [Building Deterministic Systems with Non-Deterministic Tools](/blog/deterministic-systems-non-deterministic-tools).

## Status Tracking for Multi-Session Work

Long reviews span multiple sessions. Track progress with a simple status document:

- List of files/functions to review
- Checkmarks for completed items
- Notes on decisions made
- Questions to revisit

Keep this gitignored. It's session state, not documentation.

## When to Use This

**Codebase onboarding.** Illuminate key flows with the AI narrating as you read.

**Code review with approval gates.** When every change needs sign-off before the next.

**Refactoring sessions.** Make one change, verify it works, move to the next.

**Teaching and learning.** Slow pace with space for questions beats firehose explanations.

## The Core Insight

LLMs work best with feedback loops, not fire-and-forget prompts. Plan mode creates one checkpoint. Illuminated walkthroughs create many. Both share the same principle: you can't review what you haven't seen.

Build the pause into your workflow. The LLM will produce better work, and you'll catch problems before they become expensive.
