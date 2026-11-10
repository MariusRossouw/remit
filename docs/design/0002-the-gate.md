# Design 0002: The gate

Status: built, first version. Behaviour is specified in [section 14 of the language](../../SPEC.md). Code: [src/gate.rs](../../src/gate.rs).

## The question

The checker proves that a grant is well formed. Nothing so far stopped an agent from ignoring it. What is the smallest piece that makes a grant hold at call time, whatever the agent does?

## Decision

1. **One small program, the gate, stands between the agent and the services.** Every call goes through it. It answers allow, ask, or deny.
2. **The agent is not trusted.** The gate does not rely on the model behaving, on a system prompt, or on detecting an attack. It counts and compares.
3. **The gate holds every limit.** Call counts, money spent, and which results may be undone are kept by the gate, never reported by the agent or the service.
4. **Money moves only through the gate.** A service asks the gate before it takes payment. The gate permits at most what the action's ceiling and the grant's remaining total allow.
5. **The gate has no clock and no network.** The host passes the time in. A decision is a local computation.
6. **Contracts that fail the checker are never enforced.**

## What it looks like

A host starts the gate with a grant and the services it has connected:

```
remit gate friday-dinner.remit --bind bistro=bistro.remit#Bistro
```

Then it sends one request per line and reads one answer per line:

```
{"op":"call","service":"bistro","action":"book","args":{...}}
{"ok":true,"decision":"allow","call":2,"reserve":"500 ZAR","undo":false}

{"op":"charge","call":2,"amount":"600 ZAR"}
{"ok":false,"decision":"deny","code":"G06","reason":"This call may charge at most 500 ZAR in total."}
```

The other requests are `confirm`, `result`, `fail`, `revoke`, and `state`.

## Choices that could have gone the other way

| Choice | Why | Cost |
| --- | --- | --- |
| `times <=` counts calls that later fail | A service that fails after acting must not earn a retry | An honest failure uses up a call |
| A refund does not restore a spending limit | A limit that can rise again can be pumped | The person must grant again to spend more |
| Undo needs exactly the result the gate saw | The agent cannot undo something it did not do | The agent must pass the result back unchanged |
| An expired grant cannot undo | The agent holds nothing after expiry | The person undoes it themselves |
| Arguments with unknown names are refused | Silent dropping hides agent mistakes | Stricter than most APIs |
| Results are cut to declared fields | A caller sees only what the contract says | A service cannot pass extras through |

## What this does not do

- **It is tested, not proven.** There are 16 tests, including 400 random agents checked against an independent record. Six bugs planted in the gate were each caught by that test. A machine-checked proof is still the goal.
- **It does not confine implementations.** A service can still do anything its own process can do. The one exception is money, when payment goes through the gate.
- **It does not choose well inside the grant.** A hijacked agent can still do anything the grant permits. The [benchmark](../../bench/resist-injection) shows two attacks of this kind succeeding.
- **`Untrusted` is labelled, not tracked.** The gate reports which parts of a result are untrusted. It does not follow that content through the agent.
- **State is in memory.** A restart forgets the counts.
- **The host is trusted.** It supplies the time, shows the person the prompts, and must route every call and every payment through the gate.

## Open points

1. **Unbounded spending.** If neither the grant nor the action sets a ceiling, the gate permits any charge. Proposed rule: an `allow` on a spending action must carry `spend <=`, as an irreversible action must carry `irreversible`.
2. **Conditions on fields inside a parameter.** `slot.start == 2026-11-13T19:00+02:00` would have stopped the wrong-slot attack. The language allows conditions only on top-level parameters.
3. **Durable state and the grant's holder.** Where counts live, and how a grant is bound to one agent.
4. **Calls that fail honestly.** Whether a call that failed before charging should give its count back.
