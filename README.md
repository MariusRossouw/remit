# Remit

**A language and runtime for software that is safe to run without reading it.**

Status: early. The [language specification](SPEC.md) is a first draft. A checker for it works today. Nothing enforces a contract at run time yet.

## The problem

Agents can now write software in minutes. Nobody can review it at that speed. The web's trust model assumes a human read the code and that each site is a silo, so every new integration is slow, and every unreviewed program is a risk to your money and data.

When code is cheap, trust is the bottleneck. Remit is an attempt to remove it.

A remit is the area of authority someone has been given. In Remit, every program has one, and cannot act outside it.

## The idea

You write a short **contract** that says what a service can do. Everything else follows from it.

```
type Review { rating: Int  body: Untrusted<Text> }

"A place that takes table bookings."
interface Restaurant {
  "Find free tables for a date and party size."
  action findTables(date: Date, party: Int) -> List<Slot>
    effects: read

  "Read what other guests wrote."
  action reviews() -> List<Review>
    effects: read

  "Reserve a table. May take a deposit."
  action book(slot: Slot, guest: Contact) -> Booking
    effects: write, spend
    undo: cancel

  "Cancel a booking and return its deposit."
  action cancel(booking: Booking)
    effects: write
}
```

From that one declaration the toolchain produces:

- the API,
- the tool definition an agent calls,
- the permission prompt a human sees, in plain language,
- the skeleton of the human interface,
- the bounds the runtime enforces on the implementation.

The implementation behind the contract can be written by an agent and never read by a person. It cannot do more than the contract allows.

## Try it

You need [Rust](https://rustup.rs).

```
cargo run -- check examples/dinner/*.remit
cargo run -- explain examples/dinner/friday-dinner.remit
cargo run -- check --json tests/bad/R11-irreversible-allowed.remit
```

`check` applies the rules in the specification and reports each problem with a suggested fix. `explain` prints a grant in plain language, using only the words in the contracts.

## Principles

1. **Contracts are durable; code is disposable.** A person reads twenty lines of what. Machines write and check the two thousand lines of how.
2. **No ambient authority.** Code can touch only what it is handed. A dependency that was not given the network cannot reach the network.
3. **Effects are in the type.** Whether an action reads, writes, sends, or spends, and whether it can be undone, is declared, checked, and visible before anything runs.
4. **Untrusted content stays untrusted.** Text from outside is typed as data from storage to screen. It can be shown. It can never become code or instructions.
5. **Authority is delegated, narrowed, and logged.** A user hands an agent a scoped, revocable capability, never a session. Each hop can only reduce it.
6. **One definition, many projections.** The same contract serves a person, an agent, and another program.
7. **Small enough to learn in one read.** The whole language specification must fit in an agent's context. Errors are machine-readable and say how to fix them.
8. **Compile to the standards that win.** Remit emits MCP, WebMCP, OAuth scopes, payment mandates, and Wasm components. It does not compete with them.
9. **Open and unowned.** Open specification, permissive licence, anyone can implement it.

## What it does not promise

- Bounds are not intent. Remit guarantees what code cannot do, not that it does what you wanted.
- It limits the damage of prompt injection. It does not prevent a model from being misled.
- It does not stop phishing or a user approving something they should not.

## Read next

- [Values](docs/values.md): the lines we will not cross, and how we decide everything else.
- [Language specification](SPEC.md): the whole contract language, version 0.
- [The dinner scenario](docs/scenario.md): one use case worked end to end.
- [Positioning](docs/positioning.md): what exists already and where Remit fits.
- [Roadmap](docs/roadmap.md): goals, milestones, and how we reach agents and new builders.
- [Evidence](bench/README.md): how each claim is measured against existing alternatives, and the results so far.
- [Design 0001: Shared vocabularies](docs/design/0001-shared-vocabularies.md): who defines `Restaurant`, and why a grant against it is safe.

## Contributing

Criticism is the most useful contribution at this stage. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

[Apache-2.0](LICENSE).
