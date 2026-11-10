"""Measures how much text it takes to describe the same service three ways.

Run from the repository root:  python3 bench/describe-a-service/measure.py

Bytes are always reported. Tokens are reported only if the `tiktoken` package
is installed, and are counted with an OpenAI tokenizer, so they are an
approximation for any other model.
"""
import json, re, sys
from pathlib import Path

here = Path(__file__).parent
root = here.parent.parent

def remit_text():
    parts = []
    for name in ("dining.remit", "pay.remit"):
        text = (root / "examples" / "dinner" / name).read_text()
        text = "\n".join(l for l in text.splitlines() if not l.strip().startswith("//"))
        parts.append(re.sub(r"\n{3,}", "\n\n", text).strip())
    return "\n\n".join(parts) + "\n"

def mcp_text():
    # As it travels: JSON with no optional whitespace.
    return json.dumps(json.loads((here / "dinner.mcp.json").read_text()), separators=(",", ":"))

def openapi_text():
    try:
        import yaml
        yaml.safe_load((here / "dinner.openapi.yaml").read_text())  # must at least be valid
    except ImportError:
        pass
    return (here / "dinner.openapi.yaml").read_text()

def openapi_json_text():
    import yaml
    return json.dumps(yaml.safe_load((here / "dinner.openapi.yaml").read_text()), separators=(",", ":"))

def mcp_inputs_only_text():
    # What many servers publish today: no result shapes and no behaviour hints.
    # It says less than the other forms, so it is shown for scale, not as an equal.
    tools = json.loads((here / "dinner.mcp.json").read_text())["tools"]
    lean = [{k: t[k] for k in ("name", "description", "inputSchema")} for t in tools]
    return json.dumps({"tools": lean}, separators=(",", ":"))

forms = [("Remit contract", remit_text()), ("MCP tool list (minified JSON)", mcp_text()),
         ("MCP, inputs only (says less)", mcp_inputs_only_text()),
         ("OpenAPI 3.1 (YAML)", openapi_text())]
try:
    forms.append(("OpenAPI 3.1 (minified JSON)", openapi_json_text()))
except ImportError:
    pass

try:
    import tiktoken
    enc = tiktoken.get_encoding("o200k_base")
    count = lambda s: len(enc.encode(s))
except ImportError:
    count = None

base = forms[0][1]
print(f"{'form':32} {'bytes':>7} {'x':>5}" + (f" {'tokens':>7} {'x':>5}" if count else ""))
for name, text in forms:
    row = f"{name:32} {len(text.encode()):7d} {len(text.encode())/len(base.encode()):5.2f}"
    if count:
        row += f" {count(text):7d} {count(text)/count(base):5.2f}"
    print(row)
if not count:
    print("\ntokens: not measured (install tiktoken to add an approximate count)", file=sys.stderr)
