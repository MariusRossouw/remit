# Remit contract language, version 0

This is the whole language. It must stay under 8,000 tokens.

Status: draft. No checker exists yet. Version 0 will change without notice.

## 1. What a contract is

A contract says what a service can do, in terms a person can approve and a machine can enforce. It contains no implementation.

There are five kinds of declaration:

| Declaration | Purpose |
| --- | --- |
| `type`, `enum` | The shape of data |
| `interface` | A set of actions that many services can implement |
| `service` | One concrete service |
| `grant` | Authority a person hands to an agent |

## 2. Example

```
type Slot    { id: Text  start: DateTime  seats: Int  deposit: Money }
type Contact { name: Text  phone: Text? }
type Booking { id: Text  slot: Slot  guest: Contact }
type Review  { rating: Int  body: Untrusted<Text> }

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

## 3. Lexical rules

- Source is UTF-8. Whitespace separates tokens and has no other meaning.
- Comments run from `//` to the end of the line.
- Type, interface, service, and grant names start with an upper-case letter. All other names start with a lower-case letter.
- A string is written in double quotes. `\"` and `\\` are the only escapes.
- Literals:

| Kind | Form | Example |
| --- | --- | --- |
| Int | digits | `4` |
| Decimal | digits, point, digits | `0.5` |
| Money | number, then a three-letter currency code | `500 ZAR` |
| Duration | digits, then `m`, `h`, or `d` | `24h` |
| Date | ISO 8601 date | `2026-11-13` |
| DateTime | ISO 8601 with offset | `2026-11-13T23:59+02:00` |
| List | values in brackets | `["a", "b"]` |

## 4. Grammar

```
file      = { use } { decl } ;
use       = "use" name "from" string ;
decl      = type | enum | interface | service | grant ;

type      = [ doc ] "type" Name "{" { [ doc ] name ":" typeref } "}" ;
enum      = [ doc ] "enum" Name "{" name { "," name } "}" ;

typeref   = base [ "?" ] ;
base      = scalar | "List" "<" typeref ">" | "Untrusted" "<" typeref ">" | qname ;
scalar    = "Bool" | "Int" | "Decimal" | "Text" | "Date" | "DateTime"
          | "Duration" | "Money" ;
qname     = [ name "." ] Name ;

interface = doc "interface" Name "{" { action } "}" ;
service   = doc "service" Name [ "implements" qname { "," qname } ]
            "{" { uses | action } "}" ;
uses      = "uses" name ":" ( qname | "Store" | "Http" "(" string ")" ) ;

action    = doc "action" name "(" [ param { "," param } ] ")" [ "->" typeref ]
            "effects" ":" effects
            [ "undo" ":" name [ "within" duration ] ] ;
param     = name ":" typeref ;
effects   = "read" | acting { "," acting } ;
acting    = "write" | "send" | "spend" [ "(" "max" ":" money ")" ] ;

grant     = doc "grant" Name "{" { rule } "expires" datetime "}" ;
rule      = ( "allow" | "ask" ) qname "." name [ "where" cond { "," cond } ] ;
cond      = "spend" "<=" money
          | "times" "<=" int
          | "service" "in" list
          | "irreversible"
          | name ( "==" | "<=" | ">=" | "in" ) literal ;

doc       = string ;
```

`Name` is an upper-case name and `name` a lower-case one.

## 5. Types

- `T?` means the value may be absent.
- `List<T>` is an ordered list.
- `Untrusted<T>` is content the service did not write and does not vouch for.
- A `T` may be used where `Untrusted<T>` is expected. The reverse is never allowed, and the language has no conversion.
- Types may not refer to themselves, directly or through other types.

## 6. Descriptions

A description is a string placed before a declaration. It is required on every interface, service, action, and grant, and optional on types and fields.

Descriptions are not comments. They are the words a person sees when asked to approve a grant, so they are part of the contract and part of its identity.

## 7. Effects

Every action declares what it may do.

| Effect | Meaning |
| --- | --- |
| `read` | Observes and changes nothing. Cannot be combined with the others |
| `write` | Changes state the service holds |
| `send` | Communicates with a third party on the caller's behalf |
| `spend` | Takes money from the caller. `max` sets a ceiling per call |

Any action may read. An action may do only what it declares.

## 8. Undo

`undo: cancel` promises that the named action reverses this one. `within 24h` limits the promise to a period after the call.

- The undo action must be in the same interface or service, and must accept the result of the action it reverses as its only required parameter.
- A `read` action cannot have an undo.
- An acting action with no undo is **irreversible**.

Effects are an upper bound on what an action may do. Undo is a promise about what it will allow afterwards.

## 9. Files and `use`

A file is a unit. `use dining from "example.org/dining"` makes its declarations available as `dining.Name`.

- An address is a relative path beginning with `./`, or a host and path.
- The checker records the content hash of each address the first time it is fetched, in `remit.lock`. Later fetches must match.
- An interface's identity is the hash of its canonical form, including its descriptions and every type it refers to. Canonical form is not yet specified.

## 10. Services

A service that implements an interface has all of that interface's actions without restating them. It restates an action only to tighten it.

```
use dining from "./dining.remit"

"A small restaurant."
service Bistro implements dining.Restaurant {
  uses tables: Store

  "Reserve a table. Deposit never exceeds R500."
  action book(slot: dining.Slot, guest: dining.Contact) -> dining.Booking
    effects: write, spend(max: 500 ZAR)
    undo: cancel
}
```

`uses` lists everything the implementation is handed:

| Resource | Meaning |
| --- | --- |
| `Store` | Private persistent state |
| `Http("https://host")` | Outbound requests to that origin only |
| an interface | The right to call a service that implements it |

An implementation holds nothing that is not listed.

## 11. Conformance

A service implements an interface when, for every action of the interface:

1. The service has an action of the same name.
2. It accepts every parameter of the interface. Extra parameters are optional types.
3. Its result is the interface's result, or has additional fields.
4. Its effects are a subset of the interface's. A `spend` ceiling is the same or lower.
5. If the interface promises an undo, the service does too, with the same or a longer period.

A caller that reaches a service through an interface sees the interface's types, so anything the interface marks `Untrusted` is `Untrusted` to that caller.

## 12. Grants

```
use dining from "./dining.remit"
use pay from "./pay.remit"

"Book Friday dinner for four and split the deposit."
grant FridayDinner {
  allow dining.Restaurant.findTables
  allow dining.Restaurant.reviews
  allow dining.Restaurant.book   where spend <= 500 ZAR, times <= 1
  allow pay.Wallet.request       where to in ["sipho", "lerato", "anna"], times <= 3
  expires 2026-11-13T23:59+02:00
}
```

- A grant permits the listed actions and nothing else.
- `allow` permits calls without interruption. `ask` permits them only after the person confirms each one.
- `spend <=` is a total across the life of the grant, not per call.
- `times <=` is the number of calls permitted.
- `service in` restricts the rule to services at the listed addresses.
- Any other condition compares a parameter of the action with a literal.
- A grant to call an action includes the right to call its undo on results that grant produced.
- Every grant expires.

**Irreversible actions.** An `allow` rule for an acting action with no undo is an error unless it carries the condition `irreversible`. The person must either be asked each time or have accepted, in the grant, that the action cannot be reversed.

## 13. What the checker rejects

| Code | Rule |
| --- | --- |
| R01 | A name does not resolve, or is declared twice |
| R02 | A name has the wrong case for its kind |
| R03 | A required description is missing or empty |
| R04 | A type refers to itself |
| R05 | `Untrusted<T>` is used where `T` is expected |
| R06 | `read` is combined with another effect, or a `read` action has an undo |
| R07 | An undo target is missing, or does not accept the action's result |
| R08 | A service does not conform to an interface it claims (section 11) |
| R09 | A grant rule names an action or parameter that does not exist |
| R10 | A grant condition has the wrong type, or `spend <=` is on an action that does not spend |
| R11 | An `allow` rule covers an irreversible action without `irreversible` |
| R12 | A fetched file does not match `remit.lock` |

Every error is reported as structured data: code, location, the rule in one sentence, and a suggested fix.

## 14. What is guaranteed, and by what

| Guarantee | Version 0 |
| --- | --- |
| A contract is well formed and its names resolve | Proven by the checker |
| A service's declared bounds fit inside its interface's | Proven by the checker |
| A grant refers only to real actions and acknowledges irreversibility | Proven by the checker |
| An implementation stays inside its declared effects and `uses` | **Not enforced.** Needs the runtime |
| A grant's limits hold at call time | **Not enforced.** Needs the runtime |
| A spend amount is what the service says it is | **Not enforced.** Needs runtime-mediated payment |
| A service marks third-party content `Untrusted` | **Not covered.** A service can omit the mark |
| A service is honest, or is who it says it is | **Not covered** |

Because a service can omit the mark, a caller must treat everything a service returns as data, never as instruction. `Untrusted` adds a narrower, checkable fact: this content can never be passed where trusted content is required.

## 15. Not in version 0

- Implementation code. Implementations are written in existing languages.
- Undo periods that depend on data, such as "until two hours before the booking".
- Generic types other than `List` and `Untrusted`.
- A marker for personal data.
- Who holds a grant, and the format it travels in.
