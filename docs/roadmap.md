# Roadmap

Milestones have exit criteria, not dates. Each one must be useful without the next.

## Goals we can measure

| Goal | Target |
| --- | --- |
| Learnable in one read | The full language specification fits in 8,000 tokens. Currently about 4,800 by the highest count we have measured |
| Agents succeed without training | An agent given only the specification writes a valid contract first try on 90% of a public task suite. First result: 24 of 24 on an easy suite ([write-a-service](../bench/write-a-service)) |
| Safe unread | No implementation in a public red-team suite exceeds its granted capabilities. First result, for agents not implementations: [resist-injection](../bench/resist-injection) |
| Plain-language consent | People shown a generated permission prompt can correctly say what they approved |
| Cheap to read | The same service costs fewer tokens to describe as a Remit contract than as MCP tool definitions or OpenAPI, measured and published |
| Few round trips | Benchmark tasks finish in a published number of model turns, and the number falls over time |
| Enforcement is cheap | Authorising a call needs no network request and adds under a millisecond. Measured: about 2 microseconds inside the gate, not yet compared with an alternative |
| Proven, not asserted | The core guarantees are stated as theorems and machine-checked, with the trusted parts listed |

The targets are first guesses. The measures are what matter.

Every target is measured against the strongest existing alternative, and the results are published whether we win or lose. See [bench](../bench/README.md).

## Milestones

The plan changed in November 2026 after an [outside review](reviews/0001-outside-review.md). The earlier plan built towards a full language and runtime. This one ships the part that is real and useful now, and treats the rest as research.

### Done

- A contract and grant language with a checker that explains each error ([SPEC.md](../SPEC.md)).
- The gate, which enforces a grant at call time ([Design 0002](design/0002-the-gate.md)).
- A plain-language summary of a grant.
- Three benchmarks against existing alternatives ([bench](../bench/README.md)).

### S1: Usable in front of real tools

The smallest thing a team running agents could adopt.

- A proxy that sits in front of unmodified MCP servers and enforces a grant on every tool call.
- A way to describe an existing MCP server's tools as a contract, starting from its tool list.
- Counts and totals that survive a restart. A gate that cannot read its state refuses everything.
- A record of every decision, with the rule that allowed or refused it.
- **Exit:** the dinner scenario runs against real MCP servers that know nothing about Remit, and a restart in the middle loses nothing.

### S2: Evidence we did not write

- Attacks written by other people after the language is frozen, run against the gate and published whatever the result.
- A public agent-attack suite such as AgentDojo, with grants drafted by a model from each task's request, compared with published defences.
- Models from at least three vendors, including an open one.
- A study of whether people shown a grant's summary can say what they approved.
- **Exit:** a result on a benchmark we did not design, with our losses listed.

### S3: Fit the standards

- Grants and tool descriptions in a JSON form, with the current language as shorthand for it.
- A proposal to carry task grants as an MCP extension, with this gate as the reference.
- Spending enforced by the payment network: the gate issues a capped credential for each paying call, so a service never needs to ask the gate.
- Results the gate signs, so "this value was returned by that service" survives restarts and travels between machines.
- Grants that can be handed on and only narrowed.
- **Exit:** a grant written once is enforced by two independent implementations.

### Research: untrusted content that cannot steer

The larger claim, "safe to run without reading it", needs more than a gate. It needs the agent's plan written in a small language whose interpreter tracks where every value came from, so content from outside provably cannot choose an action. This is where a new language is justified. It is hard, others are working on it, and it may not pay off. Nothing above depends on it.

## Who this is for first

Teams that already run agents against tools and need a budget per task: internal platforms, agent hosts, and wallets. They have a place to put a gate. People building with agents and no engineering background remain a goal, and they are reached through those hosts.

## Open source

- Public repository from M0. The idea gains nothing from secrecy and needs critics early.
- Apache-2.0, for its patent grant. The specification stays free for anyone to implement, whoever ends up maintaining the project.
- Contributions carry a Developer Certificate of Origin sign-off, so ownership stays clear if the project moves to an organisation.
- Credibility comes from a working checker, so M1 should follow the announcement closely.
- Decisions recorded in the open, small specification changes by proposal.

## Decisions still open

- Trademark and domain checks for the name "Remit". Package names will use `remit-lang` or a scope, since the bare name is taken on the main registries.
