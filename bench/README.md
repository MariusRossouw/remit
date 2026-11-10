# Evidence

Remit claims to be better than what exists. Each claim here is measured against the strongest existing alternative, with everything needed to repeat the measurement.

## Rules

1. **Every proof of concept has a counterpart.** Whatever we build to show an ability, we build the same thing in the best existing alternative. If different alternatives are best for size, speed, and safety, we build against each.
2. **The baseline is the strongest one, written well.** A comparison against a weak or badly written baseline is worth nothing. Anyone may replace a baseline with a better one, and we want them to.
3. **Results are published when we lose.** A benchmark we lose stays in the repository with its numbers.
4. **Each result says what it does not show.** Sample size, who wrote the baselines, and what each form can express that the others cannot.
5. **Anyone can rerun it.** One command, no account, no service of ours.

## Benchmarks

| Benchmark | Question | State |
| --- | --- | --- |
| [describe-a-service](describe-a-service) | How much text does it take to tell an agent what a service does? | First result, one service |

## Planned

| Question | Compared against |
| --- | --- |
| Can an agent write a correct service description first try, given only the specification? | MCP tool definitions, OpenAPI |
| How many model turns does a task take? | Direct tool calling over MCP |
| What does it cost to authorise one call? | An OAuth token check against an authorisation server |
| Which attacks get through? | A conventional service with the same features, against a public attack suite |
