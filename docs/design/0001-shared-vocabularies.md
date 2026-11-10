# Design 0001: Shared vocabularies

Status: proposed, not built. No hashing, lockfile, or fetching exists yet; a grant and a service match an interface only when both load the same local file. Syntax follows [version 0 of the language](../../SPEC.md).

## The question

In [the dinner scenario](../scenario.md), Thandi's grant names `Restaurant.book`, not `Bistro.book`. Her agent had never heard of Bistro when she approved it. So something called `Restaurant` has to exist apart from any one service.

Who defines it, how does a service claim to be one, and why is a grant written against it safe?

## Decision

1. **Anyone can publish an interface.** There is no registry to apply to and no body that approves vocabularies.
2. **An interface is identified by the hash of its content.** Names are for people; the hash is what grants, lockfiles, and the wire refer to. A published interface can never change underneath a grant.
3. **A service declares what it implements, and the checker proves it.** Matching by shape alone is not enough to count.
4. **An implementation may do less than the interface allows, never more.** This is the rule that makes a grant against an interface safe.
5. **Compatibility between revisions is computed, not claimed.**
6. **Anyone can write an adapter** that makes an existing service satisfy an interface, without that service's cooperation.

## What it looks like

A vocabulary, published by anyone at an address they control:

```
// published at example.org/dining

interface Restaurant {
  "Find free tables for a date and party size."
  action findTables(date: Date, party: Int) -> List<Slot>
    effects: read

  "Read what other guests wrote."
  action reviews() -> List<Review>
    effects: read

  "Reserve a table. May take a deposit."
  action book(slot: Returned<Slot>, guest: Contact) -> Booking
    effects: write, spend
    undo: cancel

  "Cancel a booking and return its deposit."
  action cancel(booking: Booking)
    effects: write
}

type Review { rating: Int  body: Untrusted<Text> }
```

A service that implements it:

```
use dining from "example.org/dining"

service Bistro implements dining.Restaurant {
  // The interface's actions are required. Bistro may add its own.
  action todaysSpecials() -> List<Dish>
    effects: read
}
```

A grant written against the interface:

```
"Book Friday dinner."
grant FridayDinner {
  allow dining.Restaurant.findTables
  allow dining.Restaurant.book   where spend <= 500 ZAR, times <= 1
  expires 2026-11-13T23:59+02:00
}
```

The grant works on Bistro, and on any other service that implements the same interface, including ones published after Thandi approved it.

## Identity and names

- The toolchain reduces an interface to a canonical form and hashes it. The hash covers everything: action names, types, effects, and the description strings, because those descriptions become the consent prompt a person approves.
- A name such as `example.org/dining` is an address the toolchain fetches from. Whoever controls the address controls the name, as with Go modules. Nobody can squat a global name, because there are none.
- The first fetch records the hash in a lockfile. After that the name is a convenience and the hash is the truth, so a hijacked address cannot change what an existing grant means.
- A small set of built-in types (`Date`, `Money`, `Text`, `Untrusted`, and similar) is part of the language. Domain vocabularies such as dining do not, and are left to whoever needs them.

## The conformance rules

A service implements an interface when, for every action in the interface:

| Rule | Meaning |
| --- | --- |
| Present | The service has an action with the same name |
| Inputs | It accepts every input the interface lists. Extra inputs must be optional |
| Outputs | It returns at least what the interface promises. Extra fields are allowed |
| Effects | Its effects are a subset of the interface's. A spend ceiling is the same or lower |
| Undo | If the interface promises an undo, the service does too, for the same period or longer |
| Provenance | Anything the interface marks `Untrusted` stays `Untrusted` for callers that come through the interface |

The effects rule carries the weight. If `Restaurant.book` allows `write, spend` and promises an undo, then no implementation can be irreversible, and none can reach beyond writing and spending. A person who approved the interface has approved an upper bound on every service behind it.

## What implementing an interface does not prove

The checker proves shape and bounds. It cannot prove that Bistro is a real restaurant, that its tables exist, or that it is the Bistro you think it is. That is a question of who you trust, and it is separate from this design. A grant can narrow the audience when it matters:

```
dining.Restaurant.book   where service in ["bistro.example"]
```

Reputation and attestation belong with discovery, later in the roadmap.

## Revisions

Changing an interface produces a new hash. The toolchain compares two interfaces and reports whether the new one is compatible: a service that implements the new one also satisfies the old one, by the same rules as above.

- Fixing a typo in a description is a compatible revision.
- Adding an action is compatible.
- Adding an effect to an existing action is not. Old grants do not cover it.

A compiled service lists the hash of every interface it satisfies, including earlier compatible revisions, so old grants keep working without anyone maintaining version numbers by hand.

## Adapters

Most services will not be written in Remit, and competing vocabularies will appear. Both problems have the same answer: a small service that holds a capability to one thing and implements an interface over it.

```
service BistroAdapter implements dining.Restaurant {
  uses legacy: HttpApi("https://bistro.example/api")
}
```

The adapter is checked like any other service. It can be written by Bistro, by a third party, or by an agent on the spot. This is how Remit reaches existing APIs and MCP servers before their owners adopt anything, and how two rival `Restaurant` interfaces get bridged.

## How a vocabulary comes to exist

1. An agent building a service asks the toolchain whether any known interface fits. The toolchain searches by shape and suggests candidates.
2. If one fits, the service declares it. Shape finds the candidate; the declaration is the commitment.
3. If none fits, the agent writes a new interface and publishes it alongside the service.
4. Interfaces that many services implement become the vocabulary. Nobody has to decide in advance.

## Why not the alternatives

| Alternative | Why not |
| --- | --- |
| A central registry or standards body | Slow, political, and a gate. It contradicts "anyone can contribute", and it fails the moment an agent needs a vocabulary that does not exist yet |
| No shared vocabulary; agents work out each service by reading it | A grant could not be written before the service was found, so every new service would need a new approval |
| Matching by shape alone | Two actions with the same shape can mean different things. A grant is a security decision and should not rest on a coincidence |
| Names with mutable versions | The meaning of an approved grant could change after approval |

## Consequences for the grammar

The grammar needs `interface`, `implements`, and `use`, as well as `service`, `action`, and `effects`. These are now in [the specification](../../SPEC.md). The conformance rules and the compatibility comparison are the core of the checker.

## Consequences for the emitters

- Interfaces map naturally onto WebAssembly interface definitions, which are also named and versioned.
- For MCP, effects map onto the existing tool hints for read-only and destructive behaviour, and the interface hash can travel as tool metadata, so an agent recognises the same interface on two unrelated servers.

## Open

1. **Ordering of effect limits.** Settled for version 0: effects are an upper bound and undo is a promise, each with one ordering. It will need revisiting when undo periods can depend on data.
2. **Where the spend amount comes from.** A service must not be able to state its own deposit and have it believed. The likely answer is that money moves only through a payment capability the runtime mediates, so the runtime sees the real amount.
3. **Canonical form.** The exact rules for hashing, including how one interface refers to another's types.
4. **Third-party adapters and consent.** Should a person be told when a call goes through an adapter the service's owner did not write?
5. **First fetch.** The lockfile protects every fetch after the first. The first still trusts the address.
