# Positioning

Checked November 2026. Most of the sources below are secondary and the field moves monthly, so recheck before quoting any of this publicly.

## The short version

Every piece of Remit exists somewhere. No one has put them behind a single thing a builder writes. Each standard today solves one boundary and leaves the builder to wire the rest by hand:

| Need | What exists | State | Gap |
| --- | --- | --- | --- |
| Agents calling services | MCP | Widely adopted | Tools have no declared effects or enforced bounds; many servers still use static API keys |
| Agents acting on web pages | WebMCP | Draft; Chrome origin trial only; API still changing | Bolted onto pages; inherits the origin's full authority |
| Delegated authority | OAuth 2.1, token exchange, AAuth, agent identity drafts | Drafts and early specifications | No standard way for each hop to narrow permissions; no task-scoped vocabulary |
| Agent payments | ACP, AP2, x402 | ACP in production; others early, low volume | Separate from what the code is allowed to do |
| Sandboxed code | Wasm component model, WASI 0.3 | Stable on servers; browsers need a transpiler | A sandbox, not an authoring model |
| Capability and effect types | Scala 3 capture checking, AILANG, research on language-based agent control | Research and experiments | Aimed at agent code execution, not at web services and delegation |

## Where Remit sits

Remit is the **authoring layer above these standards**. You write one contract. The toolchain emits the MCP server, the WebMCP tools, the OAuth scopes, the payment mandate, and the Wasm component, and keeps them consistent.

This is the move TypeScript made with JavaScript: it did not replace the platform, it compiled to it and added the guarantees the platform lacked.

It follows that:

- we do not need any standard to lose,
- when a new protocol wins, we add an emitter,
- a builder gets safety across all of them from a single source.

## Why an agent would choose it

- The whole language fits in context, so no prior training is needed.
- A contract is a complete description of a service. No documentation to hunt for.
- The checker gives structured errors with fixes, so the agent converges in a few tries.
- Code it writes can be run by its user immediately, because the bounds are enforced.

## Why a person would choose it

- New builders cannot ship a data leak or an open payment endpoint by accident. The safe path is the only path.
- Permission prompts are generated in plain language from the contract.
- One definition gives them an agent-facing and a human-facing product.

## How we could be blindsided

| Risk | Likelihood | Response |
| --- | --- | --- |
| A large vendor ships effect declarations and enforced bounds inside its own agent framework | High | Stay vendor-neutral and emit to their format. Lock-in is their weakness. |
| An established language gains this (Scala's capability work is the most serious) | Medium | They carry a large language and ecosystem. Our edge is being small and web-shaped. Learn from their papers. |
| MCP itself adds effects, attenuation, and provenance | Medium | Good outcome. Remit becomes the easiest way to author for it. |
| Another small language gets there first | Medium | Watch AILANG and similar. Compete on the web service and delegation story, which they do not address. |
| The idea is right but nobody adopts a new language | High | Keep the contract language useful alone, with implementations in existing languages, so adoption does not require switching. |

The last row is the one most likely to sink the project. It shapes the roadmap.

## Sources

- [WebMCP reality check](https://dev.to/studiomeyer_io/webmcp-reality-check-where-the-spec-actually-stands-4gh1)
- [Which browsers support WebMCP today](https://www.mcpbundles.com/guides/webmcp-browser-support)
- [WebMCP: the API moved](https://mcpplaygroundonline.com/blog/what-is-webmcp)
- [Authorization Architectures for Tool-Using AI Agents](https://arxiv.org/pdf/2609.15906)
- [MCP authorization standards](https://cerbos.dev/blog/mcp-authorization-standards)
- [Agent identity and delegated access in MCP systems](https://aaif.io/blog/agent-identity-and-delegated-access-in-mcp-systems)
- [Agent payment protocols compared](https://atxp.ai/blog/agent-payment-protocols-compared)
- [x402 Foundation](https://www.paymentsjournal.com/industry-backed-x402-foundation-is-aimed-at-agentic-commerce-gaps/)
- [Tracking Capabilities for Safer Agents](https://arxiv.org/abs/2603.00991v1)
- [Securing agents with tracked capabilities](https://caisconf.org/program/2026/papers/securing-agents-with-tracked-capabilities/)
- [AILANG](https://sunholo-data.github.io/ailang/docs)
- [WASI releases](https://wasi.dev/releases)
- [State of WebAssembly 2026](https://devnewsletter.com/p/state-of-webassembly-2026/)
