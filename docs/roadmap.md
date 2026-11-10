# Roadmap

Milestones have exit criteria, not dates. Each one must be useful without the next.

## Goals we can measure

| Goal | Target |
| --- | --- |
| Learnable in one read | The full language specification fits in 8,000 tokens. Currently about 4,000 |
| Agents succeed without training | An agent given only the specification writes a valid contract first try on 90% of a public task suite. First result: 24 of 24 on an easy suite ([write-a-service](../bench/write-a-service)) |
| Safe unread | No implementation in a public red-team suite exceeds its granted capabilities |
| Minutes, not weeks | From a one-sentence idea to a running, bounded service in under ten minutes |
| Plain-language consent | People shown a generated permission prompt can correctly say what they approved |
| Cheap to read | The same service costs fewer tokens to describe as a Remit contract than as MCP tool definitions or OpenAPI, measured and published |
| Known interfaces cost nothing | A service implementing an interface the agent has already seen is described by a hash and its additions |
| Few round trips | Benchmark tasks finish in a published number of model turns, and the number falls over time |
| Enforcement is cheap | Authorising a call needs no network request and adds under a millisecond |
| Proven, not asserted | The core guarantees are stated as theorems and machine-checked, with the trusted parts listed |

The targets are first guesses. The measures are what matter.

Every target is measured against the strongest existing alternative, and the results are published whether we win or lose. See [bench](../bench/README.md).

## Milestones

### M0: Vision (now)

- Principles, scenario, positioning, roadmap.
- **Exit:** a stranger can read the README and explain the project back.

### M1: The contract language

The smallest thing that is useful alone.

- Grammar for types, interfaces, services, actions, effects, grants, and `Untrusted`. First draft: [SPEC.md](../SPEC.md).
- Conformance checking: a service may do less than the interface it implements, never more ([Design 0001](design/0001-shared-vocabularies.md)).
- A checker with machine-readable errors and suggested fixes. Built, in Rust; the lockfile and fetching from a host are still to do.
- Emitters: an MCP server skeleton and a plain-language permission summary.
- The safety claims written as precise statements: conformance bounds every implementer, authority only narrows, `Untrusted` never reaches a trusted position.
- A token benchmark against MCP tool definitions and OpenAPI for the same services. First result: [describe-a-service](../bench/describe-a-service).
- Implementations are written in an existing language for now.
- **Exit:** the dinner scenario's contracts check, and an agent holding only the specification can write a new one.

This milestone answers the adoption risk: nobody has to switch languages to benefit.

### M2: Enforced bounds

- Implementations run as Wasm components holding only the capabilities the contract grants.
- A grant format that each hop can narrow but never widen, built on existing token work.
- An audit log with the chain of authority.
- A public red-team suite.
- Machine-checked proofs of the M1 statements, and a written list of what must be trusted for them to hold.
- **Exit:** the compromised dependency and the over-limit booking from the scenario both fail, demonstrably.

### M3: Projections

- A human interface generated from a contract, in today's browsers.
- A WebMCP emitter.
- Provenance carried through to what a model is shown.
- **Exit:** the dinner scenario runs end to end for a person and for an agent from the same source.

### M4: The implementation language

Only now, and only if M1 to M3 show that existing languages are the limit.

- Effects and capabilities checked inside implementations, not just at the boundary.
- Built-in data sync.
- **Exit:** a full service written by an agent, with no human-written implementation code, passes the red-team suite.

### M5: Money and discovery

- Mapping `spend` onto existing agent payment protocols.
- Shared vocabularies such as `Restaurant`, and a way to find services that implement them.

## Reaching agents

Agents pick tools that are in their context, work first try, and explain their failures. So:

- Publish the specification as a single file sized for a context window, plus `llms.txt` and `AGENTS.md`.
- Ship a skill or plugin for the main coding agents and an MCP server for the toolchain, so an agent can check and emit without leaving its session.
- Publish the first-try success benchmark and keep it current. It is our advertisement to agents and to the people choosing for them.
- Treat every confusing checker error as a bug.

## Reaching new builders

People building with agents and no engineering background are the first audience, not an afterthought.

- One command from idea to running service.
- Safe by default with no configuration. There is no unsafe mode to turn off.
- A plain-language summary of what the thing they just built is able to do.
- The pitch: you cannot leak your users' data by accident.

## Open source

- Public repository from M0. The idea gains nothing from secrecy and needs critics early.
- Apache-2.0, for its patent grant. The specification stays free for anyone to implement, whoever ends up maintaining the project.
- Contributions carry a Developer Certificate of Origin sign-off, so ownership stays clear if the project moves to an organisation.
- Credibility comes from a working checker, so M1 should follow the announcement closely.
- Decisions recorded in the open, small specification changes by proposal.

## Decisions still open

- Trademark and domain checks for the name "Remit". Package names will use `remit-lang` or a scope, since the bare name is taken on the main registries.
