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

- **It is tested, not proven.** There are 72 tests, 44 of them written by an outside audit. The author's own tests, including 400 random agents, missed all 24 flaws that audit found. A machine-checked proof is still the goal.
- **It does not confine implementations.** A service can still do anything its own process can do. The one exception is money, when payment goes through the gate.
- **It does not choose well inside the grant.** A hijacked agent can still do anything the grant permits. The [benchmark](../../bench/resist-injection) shows attacks of this kind succeeding.
- **`Untrusted` is labelled, not tracked.** The gate reports which parts of a result are untrusted. It does not follow that content through the agent.
- **State is in memory unless a journal is given.** With `--journal`, decisions are written down first and replayed on start ([Design 0003](0003-the-proxy.md)).
- **The host is trusted.** It supplies the time, shows the person the prompts, and must route every call and every payment through the gate.

## Open points

1. **Undo that contacts people.** The right to undo comes with the grant, and an undo can itself send messages. An agent can ask and withdraw repeatedly. Requiring an explicit rule for any undo that sends would stop it, at the cost of longer grants.
1. **Who may answer for a call.** Anyone who can reach the gate can charge or return a result for any call number. The host must keep each service to its own calls.
1. **Growth.** Pending prompts and remembered results are never dropped.
1. **The grant's holder.** How a grant is bound to one agent.
2. **Calls that fail honestly.** Whether a call that failed before charging should give its count back.
3. **`Returned` across grants.** A value counts only for the service that returned it, under the grant that saw it. A slot found under one grant cannot be booked under another.

## Closed since the first version

- **Found by [outside review](../reviews/0001-outside-review.md):** untrusted content counting as returned, two rules giving one action two budgets (now rule R17), and a service rewording the prompt a person sees.
- **Unbounded spending.** An `allow` rule on a spending action must now carry `spend <=` (rule R15). An `ask` rule may omit it, and the prompt then says there is no limit.
- **Conditions inside a parameter.** `slot.start == 2026-11-13T19:00+02:00` is now allowed, through record types only.
- **Arguments are claims.** A condition is checked against what the agent sent, and a service that goes by an id could be sent the id of one thing and the description of another. A parameter can now be declared `Returned<T>`: the gate accepts only a `T` that the same service returned under this grant, unchanged. The cost is that the agent must fetch before it acts, and must pass the value back exactly. Parameters not marked `Returned` are still claims.
