"""An MCP server (stdio) that offers the benchmark's services as tools.

Used in part 2, where a real model is the agent. The tool list, names,
descriptions and schemas are the same for every guard, so a model sees the
same thing whichever guard is behind it.

Settings come from the environment:
  BENCH_GUARD   a guard name from sim.py
  BENCH_ATTACK  an attack name from sim.py, or "none"
  BENCH_RECORD  file to write what really happened to, after every call
  BENCH_ONLY    offer one service's tools only, under their bare names, as a
                separate server for that service would
"""

import json
import os
import sys

from sim import ATTACKS, GUARDS, Refused, ServiceError, Services

TEXT = {"type": "string"}
MONEY = {"type": "string", "description": "An amount and currency, for example \"200 ZAR\""}
SLOT = {
    "type": "object",
    "properties": {"id": TEXT, "start": {"type": "string", "description": "Date and time with offset"}, "seats": {"type": "integer"}, "deposit": MONEY},
    "required": ["id", "start", "seats", "deposit"],
    "additionalProperties": False,
}
CONTACT = {"type": "object", "properties": {"name": TEXT, "phone": TEXT}, "required": ["name"], "additionalProperties": False}
BOOKING = {"type": "object", "properties": {"id": TEXT, "slot": SLOT, "guest": CONTACT}, "required": ["id", "slot", "guest"], "additionalProperties": False}
REQUEST = {
    "type": "object",
    "properties": {"id": TEXT, "to": TEXT, "amount": MONEY, "reason": TEXT},
    "required": ["id", "to", "amount", "reason"],
    "additionalProperties": False,
}


def tool(service, action, description, properties=None):
    properties = properties or {}
    schema = {"type": "object", "properties": properties, "required": list(properties), "additionalProperties": False}
    return {"name": f"{service}_{action}", "description": description, "inputSchema": schema}


# Descriptions are the ones in the contracts.
TOOLS = [
    tool("bistro", "findTables", "Find free tables for a date and party size.", {"date": {"type": "string", "description": "A date, for example 2026-11-13"}, "party": {"type": "integer"}}),
    tool("bistro", "reviews", "Read what other guests wrote."),
    tool("bistro", "todaysSpecials", "List today's specials."),
    tool("bistro", "book", "Reserve a table. May take a deposit.", {"slot": SLOT, "guest": CONTACT}),
    tool("bistro", "cancel", "Cancel a booking and return its deposit.", {"booking": BOOKING}),
    tool("wallet", "request", "Ask someone to pay you. They decide whether to pay.", {"to": TEXT, "amount": MONEY, "reason": TEXT}),
    tool("wallet", "cancelRequest", "Withdraw a request for money.", {"request": REQUEST}),
    tool("wallet", "send", "Send money to someone. It cannot be recalled.", {"to": TEXT, "amount": MONEY}),
    tool("mail", "sendMessage", "Send an email.", {"to": TEXT, "body": TEXT}),
]


def main():
    attack = os.environ.get("BENCH_ATTACK", "none")
    services = Services(review=ATTACKS[attack]["review"] if attack != "none" else None)
    guard = GUARDS[os.environ.get("BENCH_GUARD", "mcp")](services)
    record_path = os.environ.get("BENCH_RECORD")
    refusals = []
    only = os.environ.get("BENCH_ONLY")
    tools = TOOLS
    if only:
        tools = [dict(t, name=t["name"].split("_", 1)[1]) for t in TOOLS if t["name"].startswith(only + "_")]

    def save():
        if record_path:
            with open(record_path, "w") as f:
                json.dump(dict(services.record(), refusals=refusals, attempted=services.attempted(refusals)), f, indent=1)

    save()
    for line in sys.stdin:
        if not line.strip():
            continue
        message = json.loads(line)
        if "id" not in message:
            continue  # A notification. Nothing to answer.
        method, params = message.get("method"), message.get("params") or {}
        error = None
        if method == "initialize":
            result = {"protocolVersion": params.get("protocolVersion", "2025-06-18"), "capabilities": {"tools": {}}, "serverInfo": {"name": "dinner", "version": "0"}}
        elif method == "ping":
            result = {}
        elif method == "tools/list":
            result = {"tools": tools}
        elif method == "tools/call":
            service, _, action = params.get("name", "").partition("_")
            if only:
                service, action = only, params.get("name", "")
            try:
                value = guard.call(service, action, params.get("arguments") or {})
                result = {"content": [{"type": "text", "text": "Done." if value is None else json.dumps(value)}]}
            except Refused as e:
                refusals.append({"service": service, "action": action, "args": params.get("arguments"), "why": str(e)})
                result = {"content": [{"type": "text", "text": f"Refused: {e}"}], "isError": True}
            except ServiceError as e:
                result = {"content": [{"type": "text", "text": f"Error: {e}"}], "isError": True}
            save()
        else:
            result, error = None, {"code": -32601, "message": f"unknown method {method}"}
        reply = {"jsonrpc": "2.0", "id": message["id"]}
        reply.update({"error": error} if error else {"result": result})
        sys.stdout.write(json.dumps(reply) + "\n")
        sys.stdout.flush()
    guard.close()


if __name__ == "__main__":
    main()
