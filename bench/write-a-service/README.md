# Write a service

**Question:** can a model write a correct service description on its first try, in a language it has never seen, given only the specification?

This tests the roadmap goal "agents succeed without training". The counterparts are the two formats every model already knows well: MCP tool lists and OpenAPI.

## What was compared

Six briefs in plain language, each describing a small service: a clinic, a courier, a forum, a wallet, a smart home, and a calendar. They are in `tasks/`.

Each model wrote each brief three ways, twice:

| Form | What the model was given |
| --- | --- |
| Remit contract | The brief and `SPEC.md`. Neither model has seen Remit before |
| MCP tool list | The brief only |
| OpenAPI 3.1 | The brief only |

An answer scores when it is **valid** (the format's own validator accepts it) and **complete** (every action and parameter in the brief is present). There is one attempt, with no feedback and no retry.

## Result

Run on 9 November 2026 with `claude-haiku-5-5` and `claude-sonnet-5-5`, 12 answers per cell.

| Model | Form | Valid and complete | Mean answer size | Mean output tokens | Mean input tokens |
| --- | --- | --- | --- | --- | --- |
| Haiku | Remit | 12 of 12 | 830 bytes | 1,825 | 4,950 |
| Haiku | MCP | 12 of 12 | 6,202 bytes | 3,680 | 968 |
| Haiku | OpenAPI | 12 of 12 | 4,375 bytes | 2,626 | 887 |
| Sonnet | Remit | 12 of 12 | 786 bytes | 593 | 4,929 |
| Sonnet | MCP | 12 of 12 | 6,192 bytes | 2,542 | 947 |
| Sonnet | OpenAPI | 11 of 12 | 3,436 bytes | 1,472 | 866 |

Every one of the 24 Remit answers also got the behaviour exactly right: the effects of every action, which action undoes which, the spending ceiling, and which text is untrusted. The other two formats cannot state most of that, so there is nothing to compare it with.

## What this shows

- **On correctness it is a tie.** These tasks were not hard enough to separate the formats. Remit did not beat MCP, and beat OpenAPI by one answer out of 24, which is not a meaningful difference.
- **The goal was met on this suite.** Two models wrote a language they had never seen, correctly, 24 times out of 24, from the specification alone. The smaller model did as well as the larger one.
- **Remit answers are four to eight times smaller**, and took fewer output tokens to write.
- **Remit costs more to read in.** The specification adds about 4,000 input tokens to every request. A model pays that once per session if the specification is cached, but it is a real cost and the other formats do not have it.

The one failure is worth reading. The brief said an action "can be undone with cancel". OpenAPI has no way to say that, and the model tried to say it anyway by putting a `links` block where the format does not allow one. See `results/sonnet/clinic.openapi.2.txt`.

## Run it

```
cargo build --release
pip install -r bench/write-a-service/requirements.txt
python3 bench/write-a-service/run.py --models haiku sonnet --trials 2
```

By default this calls models through the `claude` command. Set `REMIT_BENCH_MODEL_CMD` to any command that reads a prompt on standard input and writes the answer to standard output. Every answer is saved in `results/`, and `--rescore` scores the saved answers again without calling a model.

## What this does not show

- **The tasks are easy and few.** Six small services, 72 answers. A ceiling effect hides any real difference in correctness. Harder tasks are needed before claiming an advantage.
- **The briefs help.** They give exact names, and describe behaviour in words close to the specification's own, such as "changes data" and "contacts others on the caller's behalf". A vaguer brief would be a harder and more honest test.
- **Both models come from one company**, and the specification was drafted with a model from the same company. Models from elsewhere may read it differently.
- **The validators are not equally strict.** The Remit checker applies meaning-level rules. The other two are checked for structure only, which favours them.
- **Output tokens include any reasoning the model did** before answering, so they are not a pure measure of answer length. Answer size in bytes is.
- **Remit was scored by Remit's own checker.** A bug that made the checker too lenient would inflate the result. The checker's tests are in `tests/`.
- **It does not test using a service**, only describing one.
