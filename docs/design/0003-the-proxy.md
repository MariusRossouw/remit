# Design 0003: The proxy

Status: built, first version. Code: [src/proxy.rs](../../src/proxy.rs) and [src/journal.rs](../../src/journal.rs).

## The question

The gate enforces a grant, but it spoke its own small protocol, so nothing real could use it. How does it get in front of the tools agents already use, without those tools changing?

## Decision

1. **The proxy is an MCP server that stands in front of other MCP servers.** The agent's host connects to the proxy. The proxy starts the real servers and is the only thing that talks to them.
2. **The servers behind it need not know Remit exists.** A short contract describes each one's tools. A tool is matched to an action by name.
3. **The agent sees only what the grant could permit.** Tools the grant has no rule for are not listed and cannot be called. This includes tools the contract does not mention at all.
4. **Nothing a server says about itself reaches the agent.** Tool descriptions and input schemas are written from the contract, in the words the person approved. A server cannot steer an agent through its tool descriptions.
5. **Every call goes through the gate first,** and every result comes back through it, cut to the fields the contract declares.
6. **Decisions are written down before they are released.** The journal is the gate's memory.

## Using it

A host is given this one server in place of the three behind it:

```
remit proxy friday-dinner.remit --journal dinner.journal \
  --server bistro=bistro.remit#Bistro="python3 bistro_server.py" \
  --server wallet=wallet.remit#Tab="npx some-wallet-mcp"
```

The agent then sees tools such as `bistro_book` and `wallet_request`.

## The journal

Each request and its outcome is one line in a file. On start, the lines are replayed through a fresh gate. Because the gate decides from its inputs alone, replay rebuilds every count, total, and undo right exactly.

The gate refuses to start, and so allows nothing, when:

- the journal cannot be read,
- it was written for a different grant, different contracts, or different connections,
- a line replays to a different outcome than was recorded, which is what a removed or edited line looks like.

If a line cannot be written, the call it describes is refused.

## Asking the person

For an `ask` rule the proxy uses MCP's own way of asking the person a question through the host (elicitation). If the host does not support it, the call is refused and the agent is told why. Nothing is ever approved by default.

## What this does not do

- **It cannot see or cap what a server charges.** Money moves inside the server. A call that may spend is counted as spending its whole ceiling, which keeps the grant's total honest but does not stop a server taking more. That needs a credential the payment network enforces.
- **It only helps if it is the only way in.** If the agent's host is also connected to the same servers directly, the proxy protects nothing.
- **Someone must write the contract.** There is no tool yet that drafts one from a server's tool list.
- **A tool is matched by name only.** If a server changes what a tool does and keeps its name, the proxy cannot tell.
- **Results must be data.** A server that answers in prose where the contract declares a record has its result withheld.
- **The journal is not tamper-proof.** Removing a line is caught. Someone who can rewrite the whole file consistently is not. Its folder must be trusted.
- **A crash while writing can leave a broken last line.** The gate then refuses to start until a person looks at it. That is the safe direction, and it is not convenient.
- **One client at a time, one call at a time.**

## Tested

Four end-to-end tests run the proxy in front of a stand-in server that offers extra tools, words its tools to push an agent around, and returns extra fields. They cover what the agent is shown, what reaches the server, a restart in the middle of a task, and a journal with a line removed.

In the [benchmark](../../bench/resist-injection), two real models completed the dinner task through the proxy, in front of three separate servers.
