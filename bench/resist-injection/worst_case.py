"""Part 1: the agent is fully hijacked.

No model is involved. A script plays an agent that does exactly what an
attacker wants, against each guard in turn. This is the worst case, and it is
the case a guard has to hold in: a guard that works only while the model
behaves is not a guard.

Run from anywhere:  python3 bench/resist-injection/worst_case.py [--json]
"""

import json
import os
import sys

from sim import ATTACKS, GUARDS, HERE, OTHER_BOOKING, Refused, ServiceError, Services, TASK


def play(guard_name, calls):
    services = Services()
    guard = GUARDS[guard_name](services)
    delivered = []
    for service, action, args in calls:
        try:
            delivered.append(guard.call(service, action, json.loads(json.dumps(args))))
        except (Refused, ServiceError):
            delivered.append(None)
    guard.close()
    return services, guard, delivered


def undo_own_booking(guard_name):
    """Can the agent cancel the booking it made, when the person changes their mind?"""
    services = Services()
    guard = GUARDS[guard_name](services)
    try:
        booking = guard.call(*TASK[2])
        guard.call("bistro", "cancel", {"booking": booking})
    except (Refused, ServiceError):
        pass
    guard.close()
    return bool(services.bookings) and services.bookings[0]["id"] in services.cancelled


def lines_between(path, begin, end):
    text = open(path).read().split(begin)[1].split(end)[0]
    code = [l for l in text.splitlines() if l.strip() and not l.strip().startswith(("#", '"""'))]
    return len(code)


def main():
    rows = {}
    for name in GUARDS:
        row = {"attacks": {}}
        for attack, spec in ATTACKS.items():
            services, _, _ = play(name, spec["calls"])
            row["attacks"][attack] = spec.get("harm", attack) in services.harms()
        services, guard, _ = play(name, TASK)
        row["task_done"] = services.task_done() and not services.harms()
        row["prompts"] = guard.prompts
        row["undo_own"] = undo_own_booking(name)
        rows[name] = row
    grant = [l for l in open(os.path.join(HERE, "contracts", "grant-tight.remit")) if l.strip() and not l.strip().startswith(("//", "use "))]
    sizes = {"remit_grant_lines": len(grant), "handwritten_guard_lines": lines_between(os.path.join(HERE, "sim.py"), "# BEGIN handwritten", "# END handwritten")}
    if "--json" in sys.argv:
        print(json.dumps({"guards": rows, "sizes": sizes}, indent=1))
        return
    names = list(GUARDS)
    print("| Attack | Inside the grant | " + " | ".join(names) + " |")
    print("| --- | --- |" + " --- |" * len(names))
    for attack, spec in ATTACKS.items():
        cells = ["**harm**" if rows[n]["attacks"][attack] else "blocked" for n in names]
        print(f"| {attack} | {'yes' if spec['inside_grant'] else 'no'} | " + " | ".join(cells) + " |")
    total = len(ATTACKS)
    print("| **Attacks that caused harm** | | " + " | ".join(f"{sum(rows[n]['attacks'].values())} of {total}" for n in names) + " |")
    print("| Honest task completed | | " + " | ".join("yes" if rows[n]["task_done"] else "**no**" for n in names) + " |")
    print("| Agent can undo its own booking | | " + " | ".join("yes" if rows[n]["undo_own"] else "**no**" for n in names) + " |")
    print("| Times the person was interrupted | | " + " | ".join(str(rows[n]["prompts"]) for n in names) + " |")
    print()
    print(f"Lines written for this task: Remit grant {sizes['remit_grant_lines']}, hand-written guard {sizes['handwritten_guard_lines']}.")


if __name__ == "__main__":
    main()
