---
slug: deterministic-systems-non-deterministic-tools
title: "Building Deterministic Systems with Non-Deterministic Tools"
authors: [angzarr]
tags: [llm, workflow, tdd, testing, collaboration]
keywords: [llm, ai, tdd, test-driven development, code review, workflow, collaboration, claude, gpt, illuminated code walkthrough]
---

Large Language Models are probabilistic text generators. Their raw outputs cannot be trusted for correctness. So how do you build reliable software with unreliable assistants?

You don't ask for answers. You ask for tools that produce answers.

<!-- truncate -->

## The Fundamental Problem

Large Language Models (LLMs)—the technology behind ChatGPT, Claude, and similar AI assistants—are probabilistic text generators. They predict the next most likely token based on patterns learned from training data. This makes them remarkably useful for many tasks, but it also means their raw outputs cannot be trusted for correctness.

Ask an LLM to calculate something, and it might be right. Or it might confidently produce nonsense. Ask it again, and you might get a different answer. This non-determinism is a feature of how these systems work, not a bug to be fixed.

So how do you build reliable software with unreliable assistants?

**You don't ask for answers. You ask for tools that produce answers.**

## The Tool-First Approach

Consider the difference:

**Wrong approach:**
> "What is the sum of all prime numbers under 1000?"

The LLM will likely produce an answer. It might even be correct. But you have no way to verify it without doing the work yourself.

**Right approach:**
> "Write a function that identifies prime numbers, then use it to sum all primes under 1000. Include tests."

Now you have:
1. Code you can read and understand
2. Tests that verify the logic
3. A tool you can re-run with different inputs
4. Something deterministic built from something non-deterministic

The LLM's non-determinism is contained to the code generation step. Once the code exists and tests pass, the system behaves predictably.

## Demand TDD

This is non-negotiable: **require test-driven development from your LLM**.

Not "write tests." Not "include tests." **Write the tests first, get my approval, then implement.**

Here's why this matters for non-deterministic systems:

**Tests are a contract.** When the LLM writes tests first, it's forced to articulate what it thinks you want. You review that articulation *before* any implementation exists. Misunderstandings surface when they're cheap to fix—before hundreds of lines of code encode the wrong assumptions.

**Tests constrain the solution space.** An LLM with a blank canvas will produce *something*. An LLM with failing tests to satisfy has a target. The non-determinism still exists, but it's bounded by concrete assertions.

**Tests are reviewable by humans.** Implementation code requires understanding algorithms, data structures, edge cases. Test code requires understanding intent: "when X happens, Y should result." You can review whether tests capture your requirements without being an expert in the implementation language.

The workflow:

1. Describe what you want
2. LLM writes tests (not implementation)
3. You review: "Do these tests capture my requirements?"
4. Iterate until tests are correct
5. LLM implements to make tests pass
6. You verify tests actually pass

If the LLM writes implementation before tests, reject it. "Stop. Tests first. Show me what you think success looks like before you show me how to achieve it."

This isn't pedantry. It's the difference between reviewing a blueprint and reviewing a finished building. One is cheap to change. The other isn't.

## Tests as Documentation

When demanding TDD, demand that tests document the **problem**, not just the solution:

```python
def test_reservation_prevents_double_spending():
    """
    Problem: Players could join multiple poker tables with the same bankroll,
    creating settlement disputes when they lose at both tables simultaneously.

    Solution: Fund reservation locks a portion of the bankroll, making it
    unavailable for other reservations until released.

    This test verifies that a second reservation fails when insufficient
    unreserved funds remain.
    """
    player = Player(bankroll=500)

    player.reserve(300)  # First table

    with pytest.raises(InsufficientFunds):
        player.reserve(300)  # Second table - should fail

    assert player.available_balance == 200
```

The docstring explains:
- **What problem exists** (double-spending across tables)
- **Why this solution** (fund locking)
- **What this specific test validates** (second reservation fails)

The test code shows **how** the solution works.

This transforms tests from "verification that code works" into "documentation of why code exists." The test docstring is the right place for explanations—it's coupled to the behavior it describes and breaks visibly when the behavior changes.

## The Illuminated Code Walkthrough

TDD handles code generation. But what about understanding existing code?

The **illuminated code walkthrough** is a collaborative reading pattern where AI narrates execution flow while you read the code. Like illuminated manuscripts with their explanatory marginalia, the AI provides context and commentary that helps you understand what you're seeing—without that commentary becoming permanent (and eventually stale) documentation.

**Start with flows, not files.** The most valuable walkthroughs trace execution paths: "Walk me through what happens when a user places an order" or "Step through the integration test for hand completion." You follow complete paths from entry point through all possible endings.

The AI narrates: "The OrderCompleted event triggers the fulfillment saga, which emits a CreateShipment command to the fulfillment aggregate, which..." You read each piece of code as it becomes relevant, understanding the full path rather than isolated functions.

**One step at a time.** The AI presents each function or handler in execution order, not file order. You see the code in the sequence it actually runs.

**AI explains as you go.** What data flows in? What transforms? What side effects occur? The AI provides narrative while you read, connecting each step to the last.

**AI questions unusual patterns.** Not just description—interrogation. "This saga assumes the inventory check already passed, but I don't see where that's enforced." The AI acts as a second set of eyes on the flow, not just the code.

**You control the pace.** The AI asks "Changes, or continue?" Don't proceed until you understand how this step connects to the whole.

The interaction:

1. You name the flow: "Walk me through the table-to-hand event flow"
2. AI presents the entry point with context
3. AI follows execution to the next handler, explaining the transition
4. AI flags potential issues in the flow
5. AI asks: "Changes, or continue?"
6. Repeat until the flow completes

**This works especially well with integration tests.** The test defines the scenario; the illuminated walkthrough reveals every step of execution that makes the test pass. You understand not just *that* it works, but *how* it works—and whether the "how" matches your mental model.

**Crucially, you're validating the AI's understanding in real-time.** When it misexplains a transition or loses the thread, you catch it immediately. This trains your calibration of when to trust its output and when to dig deeper.

**The illumination is ephemeral by design.** It helps you understand the code *now*. Don't paste it into comments—as the code changes, the explanations become stale lies. The test docstrings are your durable documentation; the illuminated walkthrough is scaffolding you discard when the session ends.

## Practical Guidelines

**1. Tests first, always**

Whether generating new code or reviewing existing code, start with tests. For generation: "Write tests first, then implement." For review: "Walk me through the tests, then the implementation."

**2. Require problem documentation in tests**

Every test function should document the specific problem it validates. This is the right place for durable explanations—coupled to behavior, visible when behavior changes.

**3. Let the illumination be ephemeral**

AI explanations during illuminated walkthroughs help you understand code in the moment. Don't preserve them as comments—they'll rot. Use them, then let them go.

**4. Verify incrementally**

Don't let the LLM write 500 lines before you review. Small batches, frequent verification. Errors compound.

**5. Run everything**

Actually execute the tests. Actually check the output. "It should work" is not the same as "it works."

## The Meta-Point

LLMs are tools for two things:
1. **Generating artifacts**—code, tests
2. **Providing narrative**—explanations, analysis, questions

The artifacts can be deterministic even when the generation process isn't. The narrative helps you understand but shouldn't be preserved—it's tied to a moment in time, not to the code itself.

Your job is to:
1. Demand tests first (constrain before implementing)
2. Review the tests (verify they capture intent)
3. Verify the artifacts (run, don't assume)
4. Use the narrative to understand (then let it go)
5. Put durable documentation in test docstrings (coupled to behavior)

The LLM accelerates the drafting and illuminates the reading. You ensure the correctness.

This isn't a limitation to work around. It's the appropriate division of labor between a probabilistic generator and a human who needs reliable systems.

---

*The irony of this post being written with AI assistance is not lost on me. The difference: I reviewed every claim, verified it matched my experience, and take responsibility for the result. That's the model.*
