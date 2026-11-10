# Describe a service

**Question:** how much text does it take to tell an agent what a service does?

An agent pays for this text every time it meets a service, so smaller is cheaper and leaves more room for the task.

## What was compared

The restaurant and wallet interfaces from [the dinner example](../../examples/dinner): six actions and six data types, with identical descriptions in every form.

| Form | File |
| --- | --- |
| Remit contract | `examples/dinner/dining.remit` and `pay.remit`, comments removed |
| MCP tool list | `dinner.mcp.json`, with input schemas, output schemas, and behaviour hints |
| OpenAPI 3.1 | `dinner.openapi.yaml` |

## Result

| Form | Bytes | Ratio | Tokens (approximate) | Ratio |
| --- | --- | --- | --- | --- |
| Remit contract | 1,142 | 1.0 | 291 | 1.0 |
| OpenAPI 3.1, minified JSON | 3,314 | 2.9 | 792 | 2.7 |
| OpenAPI 3.1, YAML | 4,020 | 3.5 | 919 | 3.2 |
| MCP tool list, minified JSON | 5,109 | 4.5 | 1,285 | 4.4 |
| MCP tool list, inputs only | 2,763 | 2.4 | 683 | 2.3 |

The last row leaves out result shapes and behaviour hints, as many servers do today. It says less than the other forms and is shown for scale.

## Run it

```
python3 bench/describe-a-service/measure.py
```

Bytes need nothing installed. The token column needs the `tiktoken` package and `pyyaml`.

## What this does not show

- **It is one small service.** The ratio may differ for larger or differently shaped services.
- **We wrote all three forms.** The baselines are meant to be idiomatic, but nobody outside the project has reviewed them.
- **Tokens are counted with an OpenAI tokenizer** (`o200k_base`). Counts for other models will differ.
- **The forms do not say the same things.** The Remit contract also states which action undoes which, and that review text is untrusted, which the other two cannot express. OpenAPI also states addresses and methods for calling over the web, which Remit does not. MCP repeats shared types inside each tool because each tool's schema must stand alone.
- **Size is not understanding.** This does not measure whether an agent uses the shorter form correctly. That is the next benchmark.
