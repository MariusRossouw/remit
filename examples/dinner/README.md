# Dinner example

The contracts behind [the dinner scenario](../../docs/scenario.md), written in [version 0 of the language](../../SPEC.md). They are the first test cases for the checker.

| File | Contains |
| --- | --- |
| `dining.remit` | The `Restaurant` interface and its types |
| `pay.remit` | The `Wallet` interface |
| `bistro.remit` | A service that implements `Restaurant` and tightens its spend limit |
| `friday-dinner.remit` | The grant Thandi gives her agent |

No checker exists yet, so nothing has verified these files.
