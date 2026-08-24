---
name: behavioral-testing
description: Write, revise, or review refactor-resistant tests derived from public behavior rather than implementation details. Use for unit, integration, property, regression, or mutation-testing work where test scope, assertions, mocks, or case selection must reflect the real contract.
---

# Behavioral testing

Create tests that fail when promised behavior changes and remain stable when the
implementation is reorganized without changing that behavior.

Treat production code as one possible implementation of the contract, not as
the specification. Do not turn everything currently observable into a permanent
obligation.

## Success criteria

- Tests exercise the public API or another explicitly documented boundary.
- Each case is justified by a user scenario, equivalence class, boundary,
  invariant, property, meaningful sequence, concurrency risk, or contractual
  external failure.
- Assertions cover the smallest set of observations that proves the behavior.
- Test doubles isolate true external boundaries or nondeterminism without
  encoding internal collaboration.
- A behavior-preserving refactor can change private structure, algorithms, call
  order, helper count, and intermediate representations without breaking the
  suite.
- Validation demonstrates that the tests run and, for new behavior when
  practical, fail for the intended reason before the implementation satisfies
  them.

## Establish the contract

Prefer contract evidence in this order:

1. Explicit requirements, acceptance criteria, and user-provided examples.
2. Public documentation, protocols, schemas, types, and API guarantees.
3. Observable behavior required by real callers or user workflows.
4. Existing tests that clearly express an intentional requirement.
5. Production code, used as supporting evidence and to locate test seams, not
   as automatic proof that every branch or detail is required.

Before adding test code, form a compact contract and case matrix that states:

- the behavior promised to a caller;
- representative inputs or preconditions;
- the public observation that proves the behavior;
- why each case is distinct and valuable.

Expose this matrix in the working explanation when useful. If no checkpoint was
requested, continue once the contract is sufficiently clear. When missing
product knowledge would materially change the expected behavior, state the gap
instead of silently blessing the current implementation. Write characterization
tests for existing behavior only when that is the stated goal; label assumptions
and suspected bugs rather than converting them into requirements.

## Choose cases by behavior

Aim for broad behavioral confidence, not one test per method, branch, or `if`.
Select the techniques that fit the contract:

- equivalence classes for inputs with the same expected behavior;
- values at, immediately below, and immediately above meaningful boundaries;
- invariants and property-based checks across a wider input space;
- user-visible state transitions and sequences of public operations;
- concurrency, retry, idempotency, or ordering scenarios when promised;
- failures of external systems when the response to those failures is part of
  the contract;
- regression cases that reproduce a confirmed contract violation.

It is valid to test many inputs and failure modes. Keep each case focused on the
few observations needed to prove its behavioral claim.

## Observe only the contract

Prefer public return values, errors with stable semantic identity, state read
through public APIs, and externally visible effects.

Avoid assertions about:

- private methods, helpers, or internal state;
- internal call order or invocation counts;
- complete objects when only selected fields carry contractual meaning;
- full error text when a stable type, code, category, or status proves the case;
- logs, incidental serialization, timestamps, generated identifiers, or
  snapshots broader than the promised output;
- every conditional path merely because it exists in the implementation.

An otherwise internal detail may be asserted only when evidence makes it part
of the public contract. Examples include a standardized wire representation, a
user-visible error message, an audit event, or an exactly-once external charge.
In that case, assert the semantic obligation at the narrowest observable
boundary instead of the mechanism used to produce it.

Do not reproduce the production algorithm inside the test to calculate the
expected result. Prefer independently stated examples, properties, or
invariants.

## Use doubles at real boundaries

Do not mock components merely to mirror the production call graph. Exercise
owned internal collaborators together through the public boundary when that is
fast and deterministic.

Use a stub, fake, spy, or mock for a boundary such as network access, filesystem
I/O, a database process, the system clock, randomness, a third-party service, or
another nondeterministic or prohibitively expensive resource. Prefer the least
behavior-coupled double that gives deterministic control.

Assert an interaction with a double only when that interaction is itself the
contract, such as sending a command to an external service. Do not assert call
count, argument shape, or ordering beyond the semantic requirement. Where
possible, verify the resulting public state or output instead.

## Review existing tests

For every fragile assertion, ask what regression it would detect. Replace or
remove assertions that only detect implementation reorganization. Consolidate
overlapping cases by behavioral partition, while preserving distinct boundary,
invariant, failure, and sequence coverage.

Use this refactor-resistance check:

> Would this test fail if the implementation were completely reorganized while
> preserving the same public API, accepted inputs, results, errors, and required
> external effects?

If yes, identify the exact contractual reason. Without one, the test is coupled
to accidental architecture.

## Validation

Run the narrowest configured test target that exercises the changed suite, then
broaden validation in proportion to the affected boundary. For new or corrected
behavior, confirm a meaningful red state when practical rather than accepting a
test that was already green for an unrelated reason.

Use mutation testing when it is configured or when high-value logic justifies
the cost. Interpret surviving mutants by behavioral relevance: strengthen tests
for meaningful incorrect behavior, not solely to maximize a mutation score.

Report the contract covered, the command run, and any ambiguity or untested risk.

## Retrieval and stop rules

Read the minimum requirements, public API, nearby tests, and test configuration
needed to establish the contract and follow repository conventions. Inspect
implementation details only far enough to understand setup, available seams,
and why a test fails.

Stop gathering once the behavioral partitions, public observations, boundary
doubles, and validation command are clear. Read more only when a missing fact
could change the expected behavior, the public boundary, or the reliability of
the test.
