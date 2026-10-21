#!/usr/bin/env python3
"""Derive transparent per-run tables from immutable results and billing events."""
import argparse
import collections
import json
from pathlib import Path
import statistics


def observed_tools(directory, harness):
    """Observed parent calls only; not the configured surface or child call count."""
    names = []
    if harness == "mini":
        path = directory / "artifacts/trajectory.json"
        if path.exists():
            for message in json.loads(path.read_text()).get("messages", []):
                if message.get("role") == "assistant":
                    names.extend(call.get("function", {}).get("name", "unknown")
                                 for call in message.get("tool_calls") or [])
    else:
        path = directory / "console.log"
        if path.exists():
            for line in path.read_text(errors="replace").splitlines():
                try:
                    event = json.loads(line)
                except ValueError:
                    continue
                if not isinstance(event, dict):
                    continue
                if event.get("type") == "tool_call":
                    names.append(event.get("name", event.get("tool", "unknown")))
                elif event.get("type") == "tool_execution_start":
                    names.append(event.get("toolName", "unknown"))
    return dict(collections.Counter(names))


def billing(path):
    requests = {}
    if path.exists():
        for line in path.read_text().splitlines():
            event = json.loads(line)
            identifier = event["request"]
            if event["kind"] == "reserved":
                requests[identifier] = {"run": event["run"], "micro": event["upper_micro_cny"],
                                        "input": None, "output": None, "settled": False}
            elif event["kind"] == "settled":
                requests[identifier].update(micro=event["upper_micro_cny"],
                    input=event["input_tokens"], output=event["output_tokens"], settled=True)
            elif event["kind"] != "unknown":
                raise ValueError("unknown billing event")
    by_run = collections.defaultdict(lambda: {"upper_micro_cny": 0, "input_tokens": 0,
        "output_tokens": 0, "requests": 0, "unknown_usage": 0})
    for request in requests.values():
        run = by_run[request["run"]]
        run["upper_micro_cny"] += request["micro"]
        run["requests"] += 1
        run["input_tokens"] += request["input"] or 0
        run["output_tokens"] += request["output"] or 0
        run["unknown_usage"] += not request["settled"]
    return dict(by_run)


def report(campaign):
    costs = billing(campaign / "billing.jsonl")
    results = []
    for path in sorted((campaign / "runs").glob("*/result.json")):
        result = json.loads(path.read_text())
        result["billing"] = costs.get(result["run"], {})
        result["observed_parent_tool_calls"] = observed_tools(path.parent, result["harness"])
        results.append(result)
    planned = []
    groups = []
    for path in sorted(campaign.glob("*plan*.json")):
        plan = json.loads(path.read_text())
        planned.append({"file": path.name, "smoke_only": plan["smoke_only"],
                        "expected_runs": len(plan["cases"]) * len(plan["tracks"]) *
                            len(plan["harnesses"]) * plan["repeats"]})
        if not plan["smoke_only"]:
            for track in plan["tracks"]:
                for harness in plan["harnesses"]:
                    rows = [r for r in results if r["run"].startswith("test-") and
                            r["case"] in plan["cases"] and r["track"] == track and r["harness"] == harness]
                    groups.append({"suite": plan.get("suite", "synthetic"), "harness": harness,
                        "track": track, "planned": len(plan["cases"]) * plan["repeats"],
                        "completed": len(rows), "resolved": sum(r["resolved"] for r in rows),
                        "statuses": dict(collections.Counter(r["status"] for r in rows)),
                        "median_seconds": statistics.median(r["seconds"] for r in rows) if rows else None,
                        "input_tokens": sum(r["billing"].get("input_tokens", 0) for r in rows),
                        "output_tokens": sum(r["billing"].get("output_tokens", 0) for r in rows),
                        "requests": sum(r["billing"].get("requests", 0) for r in rows),
                        "upper_cny": sum(r["billing"].get("upper_micro_cny", 0) for r in rows) / 1_000_000})
    summary = {"schema_version": 1, "cost_semantics": "peak-rate upper bound; not actual account debit",
               "upper_cny": sum(v["upper_micro_cny"] for v in costs.values()) / 1_000_000,
               "unknown_usage_requests": sum(v["unknown_usage"] for v in costs.values()),
               "plans": planned, "groups": groups, "runs": results,
               "billing_without_result": sorted(set(costs) - {r["run"] for r in results})}
    return summary


def markdown(summary):
    lines = ["# Benchmark results", "",
        "Synthetic small-repository tasks and a separate four-task Polyglot subset. Development smoke runs are not benchmark scores.",
        "API alias does not freeze the underlying model snapshot. No broad ranking is inferred.",
        f"Campaign cost upper bound: CNY {summary['upper_cny']:.6f}; "
        f"requests without final usage: {summary['unknown_usage_requests']}.", "",
        "## Frozen formal comparisons", "",
        "| Suite | Track | Harness | Resolved / completed / planned | Median seconds | Requests | Input tokens | Output tokens | CNY upper bound |",
        "|---|---|---|---:|---:|---:|---:|---:|---:|"]
    for group in summary["groups"]:
        seconds = f"{group['median_seconds']:.1f}" if group["median_seconds"] is not None else "—"
        lines.append(f"| {group['suite']} | {group['track']} | {group['harness']} | "
                     f"{group['resolved']} / {group['completed']} / {group['planned']} | {seconds} | "
                     f"{group['requests']} | {group['input_tokens']} | {group['output_tokens']} | {group['upper_cny']:.6f} |")
    lines += ["", "## All attempts, including development failures", "",
        "| Run | Harness | Track | Status | Independent tests | Seconds | CNY upper bound |",
        "|---|---|---|---|---|---:|---:|"]
    for result in summary["runs"]:
        lines.append(f"| {result['run']} | {result['harness']} | {result['track']} | "
            f"{result['status']} | {'pass' if result['candidate_passed'] else 'fail'} | "
            f"{result['seconds']:.1f} | {result['billing'].get('upper_micro_cny',0)/1_000_000:.6f} |")
    lines += ["", "Plans (denominators):"]
    for plan in summary["plans"]:
        lines.append(f"- {plan['file']}: {plan['expected_runs']} planned "
                     f"({'development' if plan['smoke_only'] else 'scored'}).")
    if summary["billing_without_result"]:
        lines += ["", "Billed runs without completed result: " + ", ".join(summary["billing_without_result"])]
    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("campaign", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    summary = report(args.campaign)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (args.output / "results.md").write_text(markdown(summary))
    print(f"{len(summary['runs'])} completed runs; cost upper bound CNY {summary['upper_cny']:.6f}")


if __name__ == "__main__":
    main()

