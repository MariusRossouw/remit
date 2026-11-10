# Review 0001: outside review, November 2026

A reviewer with no part in the design was asked whether the project should exist in its current form. The reviewer was another AI model from the same family as the one that helped build Remit, so this is a fresh reading, not an independent mind. Its findings were reproduced before being accepted.

## Verdict

The thesis is right and one mechanism is good: a gate that holds a task's budget and ties arguments to where they came from, against a grant a person approved. But nothing enforced today needs a new language, several documents claimed protections that did not exist, and three flaws were found in the gate.

We accept the verdict.

## Flaws found, and what was done

| Finding | Response |
| --- | --- |
| Content marked `Untrusted` was accepted as a `Returned` argument, defeating that rule | Fixed. Nothing inside an untrusted part is remembered. Tested |
| Two rules for the same action each got their own spending limit, and the summary did not say they add up | Fixed. Two `allow` rules, or two `ask` rules, that can cover the same call are rejected (R17) |
| A service could reword an action, and its wording led the prompt shown to the person | Fixed. The prompt uses the description from what the grant names, and marks the arguments as sent by the agent |
| The README and other documents claimed confinement, an audit log, and emitters that are not built | Fixed. The README now lists what is built and what is not |
| A restart forgets every count, so the gate fails open | Accepted. First on the roadmap |
| Interface identity by content hash is designed but does not exist | Accepted. The README says so |
| Undo is not recovery: after booking and cancelling, the agent cannot rebook | Accepted, open. A limit should come back only when the payment network confirms a refund |
| An action that echoes its input can turn any value into a "returned" one | Open. `Returned` should name the actions a value may come from |
| A stale record still passes as `Returned` | Open. Needs freshness, which signed results would carry |
| Free text the grant permits, such as the reason on a payment request, can carry anything | Open. A grant bounds who and how much, not what is said |

## Design changes accepted

- **The language is shorthand, not the product.** Grants and tool descriptions will get a JSON form that fits existing protocols.
- **A service's description of itself is a claim.** Declared effects and undo are the service's word. The documents now say so.
- **The gate should not mediate payment.** It should issue a capped credential that the payment network enforces.
- **The first users are teams running agents,** not people new to building.

## Where we did not fully agree

- The language did get small models writing correct contracts first try, and its checker's errors are useful. It stays as a way to write and check grants.
- The reviewer measured a TypeScript-style rendering of the dinner interfaces at about the same size as Remit. We have not repeated that measurement. Until we do, the size comparison in [describe-a-service](../../bench/describe-a-service) should be read as "smaller than JSON formats", not "smaller than everything".

## On the evidence

The reviewer's reading of the benchmarks, which we also accept:

- **describe-a-service** compares against verbose formats and leaves out a compact baseline.
- **write-a-service** hit a ceiling, used briefs close to the specification's wording, and never tested the task that matters: turning a person's sentence into a grant that is neither too wide nor too narrow.
- **resist-injection part 1** is deterministic and its attacks, harm thresholds, and baselines were chosen by the system's author. It works as a regression test. Its stateless policy baseline is weaker than a policy engine with an enforcement point that keeps counts.
- **resist-injection part 2** is a null result.

The experiments that would close these gaps are in the [roadmap](../roadmap.md) under S2.

## The audit

A second reviewer, of the same kind, was asked to break the gate and the checker by writing tests, and to write new attacks. It found 24 places where the code did not do what the specification says, each with a failing test, and wrote nine attacks. Its tests are now in `tests/audit_*.rs`.

Eight findings let an agent or a service get more than the grant allows. All are fixed:

| Finding | Fix |
| --- | --- |
| A service could charge after the grant was revoked | Charging checks that the grant is live |
| Two spending limits on one rule: the higher was enforced, the lower shown | A repeated limit is rejected |
| An approved call could charge more than its prompt said | The call may never charge more than the prompt said |
| An undo call could carry extra arguments, and cancelled a stranger's booking | An undo call carries only the result it undoes |
| A rule on an interface accepted extra parameters only the service declares | Such a rule covers the action as the interface declares it |
| `Returned` accepted any record of the same shape | The type must be the same one |
| `Returned` accepted untrusted content | Nothing inside an untrusted part is remembered |
| A service could remove an interface's `Untrusted` mark by restating an action | Results through an interface carry the interface's marks |

The other sixteen were crashes on extreme input, text that could mislead a person (hidden characters in descriptions and prompts, rule order), and loose parsing. All are fixed except that pending prompts and remembered results still grow without limit, and that anyone who can reach the gate can answer for any call. Both belong to the host and are listed in [Design 0002](../design/0002-the-gate.md).

Of the nine new attacks, six got past the tightened grant. The [benchmark](../../bench/resist-injection) reports them. Two of them, repeatedly asking and withdrawing, got past Remit but not a hand-written guard.
