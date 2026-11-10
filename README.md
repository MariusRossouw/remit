# Remit

**Task-sized permissions for AI agents, enforced even when the agent is compromised.**

A remit is the area of authority someone has been given. You approve one for a task. A small program, the gate, stands between the agent and the services it calls, and nothing outside the remit gets through.

Status: early and unproven. The table below says exactly what is built.

## The problem

An agent that can book, pay, and send messages for you needs authority. Today it usually gets all of it: every tool a service offers, for as long as the session lasts. If the agent is misled by something it reads, or is simply wrong, nothing but the model's own judgement stands in the way.

The existing answers each cover a part:

- Payment networks can cap money, at one seller.
- Policy engines can say which tools and arguments are allowed, usually one call at a time and inside one vendor's platform.
- Asking the person to approve every action works, until they stop reading.

Nobody offers one approval, sized to one task, that holds across unrelated services.

## What Remit does

You approve a grant like this once:

```
"Book Friday dinner for four and split the deposit."
grant FridayDinner {
  allow dining.Restaurant.findTables
  allow dining.Restaurant.reviews
  allow dining.Restaurant.book   where spend <= 500 ZAR, times <= 1
  allow pay.Wallet.request       where to in ["sipho", "lerato", "anna"], times <= 3
  expires 2026-11-13T23:59+02:00
}
```

The gate then decides every call the agent asks for. It does not rely on the model behaving or on spotting an attack. It counts and compares.

- **Limits across calls and services.** One booking, R500 in total, three requests to three named people, ending Friday.
- **Arguments tied to where they came from.** The agent can book only a table the restaurant actually offered, passed back unchanged.
- **Undo limited to what the agent did.** It can cancel the booking it made, and no other.
- **Nothing irreversible or costly without a decision.** A grant must cap spending and must acknowledge actions that cannot be undone, or the person is asked each time.
- **Consent in plain language**, generated from what the person approved.

Services are described by short contracts, such as the [restaurant vocabulary](examples/dinner/dining.remit) the grant above refers to.

## What is built, and what is not

| | State |
| --- | --- |
| Contract and grant language, with a checker that explains each error | Built. [Specification](SPEC.md) |
| The gate: enforces a grant at call time | Built. Tested, including by an outside audit that found 24 flaws, since fixed. Not proven, and more flaws are likely |
| Plain-language summary of a grant | Built |
| Running in front of real MCP servers | **Not built.** The gate speaks its own small protocol today |
| Counts that survive a restart | **Not built.** State is in memory, so a restart forgets what was used |
| Spending caps enforced by a payment network | **Not built.** Today a service must ask the gate before it charges |
| Confining a service's own code | **Not built, and not claimed.** A service can do whatever its process can do |
| Following untrusted content through the agent | **Not built.** Content is labelled untrusted. Nothing tracks where it goes |
| Shared vocabularies identified by content hash | **Not built.** [Designed](docs/design/0001-shared-vocabularies.md) only |

## What it does not promise

- **A grant limits harm. It does not make the agent choose well.** Inside the grant, a misled agent can still pick the wrong table.
- **A service's description of itself is a claim.** Whether an action really only reads, or can really be undone, is the service's word. The gate enforces what it can observe: calls, counts, money that passes through it, and where arguments came from.
- **It does not stop a person approving something they should not.**

## Evidence so far

Every claim is measured against the strongest alternative we could build, and losses are published. See [bench](bench/README.md). In short:

- With a fully hijacked agent and nineteen attacks, a guard written by hand for the task let 4 through, Remit 6, a stateless policy 7, and a careful person approving every call 3. Remit's protection came from 8 declared lines; the hand-written guard took 46 lines of code for this one task.
- Nothing automatic stopped attacks that stay inside the grant, such as a phishing link in a message the grant permits.
- With two current models as the agent, no planted instruction was followed under any guard. The gate had nothing to stop, and cost nothing.
- Two outside reviews found 27 flaws, now fixed, and judged that nothing enforced today needs a new language. We agree. The author's own tests had missed all of them. See [Review 0001](docs/reviews/0001-outside-review.md).

## Try it

You need [Rust](https://rustup.rs).

```
cargo run -- check examples/dinner/*.remit
cargo run -- explain examples/dinner/friday-dinner.remit
cargo run -- check --json tests/bad/R11-irreversible-allowed.remit
echo '{"op":"call","service":"bistro","action":"todaysSpecials"}' | cargo run -- gate examples/dinner/friday-dinner.remit --bind bistro=examples/dinner/bistro.remit#Bistro
```

`check` applies the rules in the specification and reports each problem with a suggested fix. `explain` prints a grant in plain language, using only the words in the contracts. `gate` enforces a grant: it answers each call an agent asks for with allow, ask, or deny, and keeps every count and spending limit itself ([Design 0002](docs/design/0002-the-gate.md)).

## Read next

- [Language specification](SPEC.md): contracts and grants, and exactly what the gate enforces.
- [Design 0002: The gate](docs/design/0002-the-gate.md): how a grant is made to hold, and what it does not do.
- [Evidence](bench/README.md): each benchmark, its result, and what it does not show.
- [Review 0001](docs/reviews/0001-outside-review.md): the criticism that reshaped the project, and our response.
- [Roadmap](docs/roadmap.md): what ships next.
- [Positioning](docs/positioning.md): what exists already and where this fits.
- [Values](docs/values.md): the lines we will not cross.
- [The dinner scenario](docs/scenario.md): the long-term picture. Most of it is not built.

## Contributing

Criticism is the most useful contribution at this stage, and attacks most of all. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

[Apache-2.0](LICENSE).
