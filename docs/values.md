# Values

Remit asks people to run software nobody has read. That only works if the project itself can be trusted to hold a line. This document says where the lines are, and how we decide everything else.

It was written before there was code, money, or users, because that is the only time it can be written honestly.

## Non-negotiables

Seven lines we will not cross. Each has a test, because a value that cannot be checked is a slogan.

### 1. No ambient authority, and no way to switch that off

Code can touch only what it was handed. There is no unsafe mode, no trusted flag, no development setting that relaxes the bounds.

*Crossed if:* any program can reach something its contract does not name, by any means we provide.

### 2. Authority only narrows

Whoever passes authority on can reduce it. Nobody can widen it, including the runtime, the service, and us.

*Crossed if:* any holder of a grant can do something the person who issued it could not have read in it.

### 3. Outside content is never obeyed

What a service returns is data to whoever called it. Content marked `Untrusted` cannot become code, a query, an instruction, or a trusted value.

*Crossed if:* the language offers any way to turn `Untrusted` text into something that is executed or trusted.

### 4. People see what they grant, and can take it back

Consent text is generated from the contract, never written by the party asking for access. Every grant expires, can be revoked, and leaves a record the person can read.

*Crossed if:* a person can be shown a prompt that says less than the grant does, or a grant exists that they cannot end.

### 5. We claim only what we enforce

Every guarantee says whether it is proven by the checker, enforced by the runtime, or not covered. We do not describe a thing as safe because it is likely to be.

*Crossed if:* any document, error message, or prompt implies a protection that is not mechanically enforced.

### 6. Small enough to hold in one read

The whole language specification fits in 8,000 tokens. A feature that would break the budget must remove something first.

*Crossed if:* the specification exceeds the budget, or an agent needs material outside it to write a correct contract.

### 7. Open and unowned

The specification is free for anyone to implement. Nothing in the language depends on a service that one organisation controls. Safety is never a paid feature. This binds any company built on Remit, including ours.

*Crossed if:* a correct, complete implementation cannot be built from the public specification alone.

## Whose interest wins

When interests conflict, the earlier one wins.

1. The person whose authority, money, or data is at stake.
2. The people and agents who use a service.
3. The people and agents who build services.
4. The people who build Remit's tooling.
5. The elegance of the design.
6. The growth of the project, and of any company around it.

## Which quality wins

When qualities conflict, the earlier one wins.

1. Safety of the person.
2. Honesty about limits.
3. Clarity to whoever reads a contract or a prompt.
4. Smallness.
5. Fit with existing standards.
6. Convenience for the author.
7. Raw execution speed.

Raw speed is last, and Remit must still be fast. The list only says what gives way in a conflict, and the design aims to have no conflict: bounds are checked before code runs, a grant is verified without a network call, and nothing is looked up per request. Enforcement that is slow gets bypassed, so slowness is a safety problem too.

Speed of building, and speed for agents in tokens and round trips, are not in this list. They are goals, with targets in [the roadmap](roadmap.md).

## The decision test

Every design proposal answers these, in writing. The rules for comparisons are in [bench](../bench/README.md).

| Question | A proposal fails if |
| --- | --- |
| Does it cross a non-negotiable? | Yes. Stop here |
| Who bears the cost when it goes wrong? | Someone higher in the first list pays for the convenience of someone lower |
| Is each guarantee proven, enforced, or neither? | It does not say |
| What does it add to the specification, and what does it remove? | It grows the language with no accounting |
| Can an agent holding only the specification use it correctly first time? | It needs explanation the specification does not contain |
| Does it work without us? | It relies on a service or registry one party runs |
| What is it measured against? | It claims an improvement with no comparison against the strongest existing alternative, or the comparison cannot be rerun |
| Can it be undone later? | It is permanent and has had no more scrutiny than a reversible change |

## Changing this document

The two orderings and the decision test can be revised by open proposal, with the reason recorded.

The non-negotiables are different. Removing or weakening one means Remit has become a different project, and anyone relying on it is owed that statement in plain words, in public, before the change.
