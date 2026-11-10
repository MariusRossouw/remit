# Positioning

Checked November 2026. Most of the sources below are secondary and the field moves monthly, so recheck before quoting any of this publicly.

## The short version

Remit is one approval, sized to one task, enforced across unrelated services. Each existing piece covers part of that:

| What exists | What it covers | What it leaves open |
| --- | --- | --- |
| Payment network caps (capped payment tokens, AP2 mandates) | Money, at one seller, enforced where it counts | Everything that is not money |
| Policy engines at an agent gateway (Cedar, OPA) | Which tools, with which arguments. Analysis tools that can prove what a policy allows | Running totals need an enforcement point that keeps them. Tied to one platform |
| MCP tool annotations | A vocabulary for describing risk | They are hints. Nothing enforces them |
| OAuth Rich Authorization Requests, GNAP | Typed, detailed authorisation requests | No counting across calls, no consent vocabulary |
| Macaroons, Biscuit | Authority that can be handed on and only narrowed, checked offline | No counters. No link to a person-readable task |
| Research on capability and information-flow control for agents (CaMeL and others) | Tracking untrusted data through an agent's plan | Not products. A cost in what the agent can do |
| On-chain permissions (session keys, delegation frameworks) | Value, call, and time limits enforced by a ledger | Assets only |

Some of this table is from an outside review and secondary sources, and has not been checked against primary documents.

## What Remit adds

- Counts and totals kept across calls and across services, under one grant a person can read.
- Arguments tied to where they came from: the agent can act only on something the service really returned.
- Undo limited to what the agent itself did.
- Consent text generated from what the person approved.
- Vendor-neutral and open.

None of this needs a new language. The contract syntax is a convenient way to write it and check it. The plan is to give it a JSON form and offer it as an extension to MCP ([roadmap](roadmap.md)).

## How this could be overtaken

| Risk | Likelihood | Response |
| --- | --- | --- |
| A large vendor adds counters and task grants to its own agent gateway | High | Be the open, portable form. Offer the parts they lack, such as argument provenance and undo rights, upstream |
| MCP adopts task grants itself | Medium | The best outcome. Contribute the design and the reference gate |
| Payment networks make spending caps universal | High, and good | Stop mediating payment. Issue their credentials |
| Models become reliably immune to being misled | Low | Then the gate is insurance nobody claims on. Limits on an honest agent's mistakes would still matter |
| Nobody adopts another format | High | Work in front of unmodified MCP servers, so adopting costs one proxy |

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
