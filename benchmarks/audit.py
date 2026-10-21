#!/usr/bin/env python3
"""Check complete paired denominators, frozen task hashes and independent evidence."""
import argparse
import itertools
import json
from pathlib import Path

from bench import ROOT, digest_tree
from report import report


def audit(campaign):
    summary = report(campaign)
    observed = {row["run"]: row for row in summary["runs"]}
    expected = set()
    errors = []
    cases = {}
    for path in sorted(campaign.glob("frozen-plan-*.json")):
        plan = json.loads(path.read_text())
        for name, expected_hash in plan["cases"].items():
            case = ROOT / ("anchors" if plan["suite"] == "polyglot" else "cases") / name
            if digest_tree(case) != expected_hash:
                errors.append(f"case changed: {name}")
            cases[name] = expected_hash
        for repeat, name, track, harness in itertools.product(
                range(plan["repeats"]), plan["cases"], plan["tracks"], plan["harnesses"]):
            identifier = f"test-{name}-{harness}-{track}-{repeat}"
            expected.add(identifier)
            row = observed.get(identifier)
            if row is None:
                errors.append(f"missing result: {identifier}")
                continue
            if row["case_sha256"] != cases[name]:
                errors.append(f"run hash mismatch: {identifier}")
            if row["harness"] != harness or row["track"] != track or row["case"] != name:
                errors.append(f"run labels mismatch: {identifier}")
            directory = campaign / "runs" / identifier
            for filename in ["candidate.diff", "console.log", "grade.json", "artifacts/adapter.json"]:
                if not (directory / filename).is_file():
                    errors.append(f"missing evidence: {identifier}/{filename}")
            if row["resolved"]:
                grade = json.loads((directory / "grade.json").read_text())
                if not grade["passed"] or row["exit_code"] != 0 or row["status"] != "passed":
                    errors.append(f"invalid success: {identifier}")
    actual = {name for name in observed if name.startswith("test-")}
    for identifier in sorted(actual - expected):
        errors.append(f"unplanned scored result: {identifier}")
    if summary["upper_cny"] > 300:
        errors.append("campaign cost exceeded CNY 300")
    if summary["billing_without_result"]:
        errors.append("billed attempts without result: " + ", ".join(summary["billing_without_result"]))
    if not expected:
        errors.append("no scored plan")
    return {"schema_version": 1, "passed": not errors, "expected_formal": len(expected),
            "completed_formal": len(actual), "cases": cases, "upper_cny": summary["upper_cny"],
            "unknown_usage_requests": summary["unknown_usage_requests"], "errors": errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("campaign", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = audit(args.campaign)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    raise SystemExit(0 if result["passed"] else 1)


if __name__ == "__main__":
    main()
