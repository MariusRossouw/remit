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
| [describe-a-service](describe-a-service) | How much text does it take to tell an agent what a service does? | First result, one service. Remit is 2.7 to 4.4 times smaller |
| [write-a-service](write-a-service) | Can a model write a correct description first try, given only the specification? | First result, six services, two models. A tie on correctness; Remit answers are much smaller but the specification costs input tokens |
| [resist-injection](resist-injection) | Which guard stops the harm when an agent is told to do harm? | First result, one scenario, nine attacks. With a hijacked agent, Remit blocks 7 of 9, equal to a hand-written guard and ahead of stateless MCP guards (2 or 3 of 9). With two real models, no attack was attempted under any guard: a tie |

## Planned

| Question | Compared against |
| --- | --- |
| How many model turns does a task take? | Direct tool calling over MCP |
| What does it cost to authorise one call? | An OAuth token check against an authorisation server |
| Which attacks get through an implementation's bounds? | A conventional service with the same features, against a public attack suite |
| Do attacks that adapt to the model get through? | The guards in resist-injection, with a published attack suite and models from other vendors |
