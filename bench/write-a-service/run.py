"""Benchmark: can a model write a correct service description on its first try?

Each task is a plain-language brief for a small service. The model writes it
three ways: as a Remit contract, as an MCP tool list, and as an OpenAPI 3.1
document. For Remit the model is given SPEC.md, because it has never seen the
language. For the other two it is given nothing extra, because it already
knows them.

An answer scores when it is:
  valid     the format's own validator accepts it
  complete  every action and parameter in the brief is present

Run from the repository root, after `cargo build --release`:

  python3 bench/write-a-service/run.py --models haiku sonnet --trials 2

The model is called through a command that reads a prompt on standard input
and writes the answer to standard output. Set REMIT_BENCH_MODEL_CMD to change
it; `{model}` is replaced by each name given to --models.
"""
import argparse, concurrent.futures, json, os, re, shlex, subprocess, sys, tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
REMIT = ROOT / "target" / "release" / "remit"
DEFAULT_CMD = ('claude -p --model {model} --tools "" --output-format json --no-session-persistence '
               '--strict-mcp-config --disable-slash-commands '
               '--system-prompt "You write files exactly as asked."')
FORMATS = ["remit", "mcp", "openapi"]

INSTRUCTIONS = {
    "remit": (
        "Write the service below as a Remit contract: one file containing its types and one "
        "interface named {name}. You have not seen this language before. Its complete "
        "specification follows the brief. Output only the file, in one code block.\n\n"
        "{brief}\n\n---- SPECIFICATION ----\n\n{spec}"
    ),
    "mcp": (
        "Write the service below as a Model Context Protocol tool list: a JSON object of the "
        "form {{\"tools\": [...]}}. Give each tool a name, a description, an inputSchema, an "
        "outputSchema when the action returns something, and annotations (readOnlyHint, "
        "destructiveHint, idempotentHint). Each schema must be valid JSON Schema and must "
        "stand alone. Express as much of the brief as the format allows. Output only the "
        "JSON, in one code block.\n\n{brief}"
    ),
    "openapi": (
        "Write the service below as an OpenAPI 3.1 document in YAML. Use each action name as "
        "its operationId. Express as much of the brief as the format allows. Output only the "
        "YAML, in one code block.\n\n{brief}"
    ),
}


def call_model(cmd_template, model, prompt):
    cmd = shlex.split(cmd_template.replace("{model}", model))
    with tempfile.TemporaryDirectory() as empty:
        proc = subprocess.run(cmd, input=prompt, capture_output=True, text=True, cwd=empty, timeout=600)
    if proc.returncode != 0:
        raise RuntimeError("model command failed: " + proc.stderr.strip()[:300])
    out = proc.stdout
    meta = {}
    try:
        data = json.loads(out)
        if isinstance(data, dict) and "result" in data:
            out = data["result"]
            usage = data.get("usage") or {}
            meta = {
                "output_tokens": usage.get("output_tokens"),
                "input_tokens": sum(usage.get(k) or 0 for k in
                                    ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens")),
                "models": sorted((data.get("modelUsage") or {}).keys()),
            }
    except ValueError:
        pass
    return out, meta


def code_block(text):
    blocks = re.findall(r"```[^\n]*\n(.*?)```", text, re.S)
    return (max(blocks, key=len) if blocks else text).strip() + "\n"


# ---- Scoring. Each returns (valid, why_not, found) where found maps an action
# ---- name to {"params": set, "effects": list or None, "undo": name or None}.

def score_remit(text):
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "answer.remit")
        Path(path).write_text(text)
        check = subprocess.run([str(REMIT), "check", "--json", path], capture_output=True, text=True)
        try:
            report = json.loads(check.stdout)
        except ValueError:
            return False, "checker gave no report", {}, {}
        if not report["ok"]:
            first = report["errors"][0]
            return False, "%s %s" % (first["code"], first["message"]), {}, {}
        listing = json.loads(subprocess.run([str(REMIT), "list", path], capture_output=True, text=True).stdout)[0]
    found = {}
    for group in listing["interfaces"] + listing["services"]:
        for a in group["actions"]:
            found[a["name"]] = {
                "params": {p["name"] for p in a["params"]},
                "effects": sorted(e["kind"] for e in a["effects"]),
                "undo": (a["undo"] or {}).get("action"),
                "spend_max": next((e["max"] for e in a["effects"] if e["kind"] == "spend"), None),
            }
    untrusted = {(t["name"], f["name"]) for t in listing["types"] for f in t["fields"] if "Untrusted<" in f["type"]}
    return True, "", found, {"untrusted": untrusted}


def local_refs_resolve(schema):
    def walk(node):
        if isinstance(node, dict):
            ref = node.get("$ref")
            if isinstance(ref, str):
                if not ref.startswith("#/"):
                    return "reference %s points outside the schema" % ref
                target = schema
                for part in ref[2:].split("/"):
                    if not isinstance(target, dict) or part not in target:
                        return "reference %s does not resolve" % ref
                    target = target[part]
            for v in node.values():
                problem = walk(v)
                if problem:
                    return problem
        elif isinstance(node, list):
            for v in node:
                problem = walk(v)
                if problem:
                    return problem
        return None
    return walk(schema)


def score_mcp(text):
    from jsonschema import Draft202012Validator
    try:
        data = json.loads(text)
    except ValueError as e:
        return False, "not JSON: %s" % e, {}, {}
    tools = data.get("tools") if isinstance(data, dict) else None
    if not isinstance(tools, list) or not tools:
        return False, "no tools list", {}, {}
    found = {}
    for t in tools:
        if not isinstance(t, dict) or not isinstance(t.get("name"), str) or not isinstance(t.get("inputSchema"), dict):
            return False, "a tool lacks a name or inputSchema", {}, {}
        if not isinstance(t.get("description"), str) or not t["description"].strip():
            return False, "tool %s has no description" % t["name"], {}, {}
        if t["inputSchema"].get("type") != "object":
            return False, "inputSchema of %s is not an object" % t["name"], {}, {}
        for key in ("inputSchema", "outputSchema"):
            if key in t:
                try:
                    Draft202012Validator.check_schema(t[key])
                except Exception as e:
                    return False, "%s of %s is not valid JSON Schema: %s" % (key, t["name"], str(e)[:120]), {}, {}
                problem = local_refs_resolve(t[key])
                if problem:
                    return False, "%s of %s: %s" % (key, t["name"], problem), {}, {}
        hints = t.get("annotations") or {}
        found[t["name"]] = {
            "params": set((t["inputSchema"].get("properties") or {}).keys()),
            "read_only": hints.get("readOnlyHint"),
        }
    return True, "", found, {}


def score_openapi(text):
    import yaml
    from openapi_spec_validator import validate
    try:
        spec = yaml.safe_load(text)
    except Exception as e:
        return False, "not YAML: %s" % str(e)[:120], {}, {}
    try:
        validate(spec)
    except Exception as e:
        return False, "not valid OpenAPI: %s" % str(e).strip().splitlines()[0][:160], {}, {}

    def deref(node, depth=0):
        while isinstance(node, dict) and "$ref" in node and depth < 20:
            target = spec
            for part in node["$ref"][2:].split("/"):
                target = target.get(part, {}) if isinstance(target, dict) else {}
            node, depth = target, depth + 1
        return node if isinstance(node, dict) else {}

    def properties(schema, depth=0):
        schema = deref(schema)
        names = set((schema.get("properties") or {}).keys())
        if depth < 5:
            for part in schema.get("allOf") or []:
                names |= properties(part, depth + 1)
        return names

    found = {}
    for path, item in (spec.get("paths") or {}).items():
        item = deref(item)
        shared = item.get("parameters") or []
        for method, op in item.items():
            if method not in ("get", "put", "post", "delete", "patch") or not isinstance(op, dict):
                continue
            params = {deref(p).get("name") for p in shared + (op.get("parameters") or [])}
            body = deref(op.get("requestBody") or {})
            for media in (body.get("content") or {}).values():
                params |= properties(media.get("schema") or {})
            found[op.get("operationId")] = {"params": params, "read_only": method == "get"}
    return True, "", found, {}


SCORERS = {"remit": score_remit, "mcp": score_mcp, "openapi": score_openapi}


def judge(task, fmt, text):
    """Returns the row recorded for one answer."""
    try:
        valid, why, found, extra = SCORERS[fmt](text)
    except Exception as e:  # a scorer crash is a harness bug, not a model failure
        return {"valid": False, "complete": False, "why": "SCORER ERROR: %r" % e, "harness_error": True}
    row = {"valid": valid, "complete": False, "why": why, "bytes": len(text.encode())}
    if not valid:
        return row
    missing = []
    for a in task["actions"]:
        got = found.get(a["name"])
        if got is None:
            missing.append("action %s" % a["name"])
            continue
        for p in a["params"]:
            if p not in got["params"]:
                missing.append("%s.%s" % (a["name"], p))
    row["complete"] = not missing
    if missing:
        row["why"] = "missing: " + ", ".join(missing)
    # Beyond the headline score: did the answer capture behaviour, where the format can?
    if fmt == "remit" and row["complete"]:
        wrong = []
        for a in task["actions"]:
            got = found[a["name"]]
            if got["effects"] != sorted(a["effects"]):
                wrong.append("%s effects %s, expected %s" % (a["name"], got["effects"], sorted(a["effects"])))
            if got["undo"] != a["undo"]:
                wrong.append("%s undo %s, expected %s" % (a["name"], got["undo"], a["undo"]))
            if a.get("spend_max") and got["spend_max"] != a["spend_max"]:
                wrong.append("%s spend ceiling %s, expected %s" % (a["name"], got["spend_max"], a["spend_max"]))
        for pair in task["untrusted"]:
            if tuple(pair) not in extra["untrusted"]:
                wrong.append("%s.%s not marked Untrusted" % tuple(pair))
        row["behaviour_exact"] = not wrong
        if wrong:
            row["behaviour_notes"] = wrong
    if fmt in ("mcp", "openapi") and row["complete"]:
        wrong = [a["name"] for a in task["actions"]
                 if found[a["name"]]["read_only"] is not None
                 and bool(found[a["name"]]["read_only"]) != (a["effects"] == ["read"])]
        row["read_only_exact"] = not wrong
    return row


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--models", nargs="+", required=True)
    ap.add_argument("--trials", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--rescore", action="store_true", help="score saved answers again without calling a model")
    args = ap.parse_args()
    if not REMIT.exists():
        sys.exit("Build the checker first: cargo build --release")
    cmd = os.environ.get("REMIT_BENCH_MODEL_CMD", DEFAULT_CMD)
    spec = (ROOT / "SPEC.md").read_text()
    tasks = [json.loads(p.read_text()) for p in sorted((HERE / "tasks").glob("*.json"))]
    out_dir = HERE / "results"

    def one(job):
        model, task, fmt, trial = job
        raw_path = out_dir / model / ("%s.%s.%d.txt" % (task["id"], fmt, trial))
        meta_path = raw_path.with_suffix(".meta.json")
        if args.rescore:
            raw, meta = raw_path.read_text(), json.loads(meta_path.read_text())
        else:
            prompt = INSTRUCTIONS[fmt].format(name=task["name"], brief=task["brief"], spec=spec)
            try:
                raw, meta = call_model(cmd, model, prompt)
            except Exception as e:
                return {"model": model, "task": task["id"], "format": fmt, "trial": trial,
                        "valid": False, "complete": False, "why": "CALL FAILED: %s" % e, "harness_error": True}
            raw_path.parent.mkdir(parents=True, exist_ok=True)
            raw_path.write_text(raw)
            meta_path.write_text(json.dumps(meta))
        row = judge(task, fmt, code_block(raw))
        row.update({"model": model, "task": task["id"], "format": fmt, "trial": trial}, **meta)
        return row

    jobs = [(m, t, f, n) for m in args.models for t in tasks for f in FORMATS for n in range(1, args.trials + 1)]
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        rows = list(pool.map(one, jobs))
    rows.sort(key=lambda r: (r["model"], r["format"], r["task"], r["trial"]))
    out_dir.mkdir(exist_ok=True)
    (out_dir / "rows.json").write_text(json.dumps(rows, indent=1, default=sorted))

    def mean(values):
        values = [v for v in values if v is not None]
        return sum(values) / len(values) if values else float("nan")

    print("%-10s %-8s %5s %6s %9s %8s %8s  %s" % ("model", "format", "runs", "valid", "complete", "out tok", "in tok", "extra"))
    for model in args.models:
        for fmt in FORMATS:
            rs = [r for r in rows if r["model"] == model and r["format"] == fmt and not r.get("harness_error")]
            if not rs:
                continue
            ok = [r for r in rs if r["complete"]]
            key = "behaviour_exact" if fmt == "remit" else "read_only_exact"
            label = "behaviour exact" if fmt == "remit" else "read-only flag right"
            extra = "%s %d/%d" % (label, sum(1 for r in ok if r.get(key)), len(ok))
            print("%-10s %-8s %5d %6d %9d %8.0f %8.0f  %s" % (
                model, fmt, len(rs), sum(r["valid"] for r in rs), len(ok),
                mean([r.get("output_tokens") for r in rs]), mean([r.get("input_tokens") for r in rs]), extra))
    problems = [r for r in rows if not r["complete"] or r.get("behaviour_notes")]
    if problems:
        print("\nWhat went wrong:")
        for r in problems:
            note = r["why"] or "; ".join(r.get("behaviour_notes", []))
            print("  %s %s %s #%d: %s" % (r["model"], r["format"], r["task"], r["trial"], note))
    errors = [r for r in rows if r.get("harness_error")]
    if errors:
        print("\n%d run(s) failed in the harness and are excluded from the table." % len(errors))


if __name__ == "__main__":
    main()
