#!/usr/bin/env python3
"""Public small-repository benchmark tooling. Standard library only."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parent
CASES = ROOT / "cases"


def execute(argv, cwd, timeout=120, env=None):
    """Bound output on disk; cancellation/timeout kills the whole process group."""
    started = time.monotonic()
    with tempfile.TemporaryFile() as output:
        process = subprocess.Popen(argv, cwd=cwd, env=env, stdout=output,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        timed_out = False
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
        finally:
            # Also reap descendants left behind by a command which exited.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
        output.seek(0, os.SEEK_END)
        size = output.tell()
        output.seek(max(0, size - 24_000))
        tail = output.read().decode(errors="replace")
    return {"exit_code": process.returncode, "timeout": timed_out,
            "seconds": round(time.monotonic() - started, 3), "output_tail": tail,
            "output_bytes": size}


def digest_tree(directory):
    digest = hashlib.sha256()
    for path in sorted(directory.rglob("*")):
        if path.is_file() and not any(p in {".git", "target", "__pycache__"} for p in path.parts):
            digest.update(path.relative_to(directory).as_posix().encode() + b"\0")
            digest.update(path.read_bytes() + b"\0")
    return digest.hexdigest()


def apply_patch(work, patch):
    result = execute(["git", "apply", "--unsafe-paths", str(patch.resolve())], work)
    if result["exit_code"]:
        raise RuntimeError(f"reference patch failed: {patch}\n{result['output_tail']}")


def seed(case, destination):
    shutil.copytree(case / "seed", destination)
    return destination


def grade_local(case, work):
    """Trusted self-validation only. Live candidates are graded inside a fresh container."""
    metadata = json.loads((case / "case.json").read_text())
    language = metadata["language"]
    env = os.environ.copy()
    env.update(PYTHONDONTWRITEBYTECODE="1", PYTHONPATH=str(work), CARGO_NET_OFFLINE="true")
    if metadata.get("suite") == "polyglot-subset":
        result = execute([sys.executable, "-m", "unittest", "discover", "-p", "*_test.py"]
                         if language == "python" else
                         ["cargo", "test", "--offline", "--quiet", "--", "--include-ignored"], work, env=env)
        return {"passed": result["exit_code"] == 0 and not result["timeout"], "upstream_tests": result}
    smoke = execute([sys.executable, "-m", "unittest", "discover", "-s", "tests"]
                    if language == "python" else ["cargo", "test", "--offline", "--quiet"],
                    work, env=env)
    if language == "python":
        acceptance = execute([sys.executable, str(case / "judge.py")], work, env=env)
    else:
        target = work / "tests" / "benchmark_acceptance.rs"
        target.parent.mkdir(exist_ok=True)
        target.write_bytes((case / "judge.rs").read_bytes())
        acceptance = execute(["cargo", "test", "--offline", "--quiet",
                              "--test", "benchmark_acceptance"], work, env=env)
        target.unlink()
    return {"passed": all(x["exit_code"] == 0 and not x["timeout"]
                          for x in [smoke, acceptance]),
            "regression": smoke, "acceptance": acceptance}


def self_validate():
    results = []
    for case in sorted(CASES.iterdir()):
        if not (case / "case.json").exists():
            continue
        metadata = json.loads((case / "case.json").read_text())
        with tempfile.TemporaryDirectory(prefix="niucode-case-") as temporary:
            base = Path(temporary)
            original = seed(case, base / "original")
            seed_result = grade_local(case, original)
            reference = seed(case, base / "reference")
            apply_patch(reference, case / "reference.patch")
            reference_result = grade_local(case, reference)
            mutants = {}
            for name in metadata["mutants"]:
                work = seed(case, base / name)
                apply_patch(work, case / "reference.patch")
                apply_patch(work, case / "mutants" / f"{name}.patch")
                mutants[name] = grade_local(case, work)
        valid = (seed_result["regression"]["exit_code"] == 0
                 and not seed_result["passed"] and reference_result["passed"]
                 and all(not m["passed"] for m in mutants.values()))
        row = {"case": case.name, "sha256": digest_tree(case), "valid": valid,
               "seed": seed_result, "reference": reference_result, "mutants": mutants}
        results.append(row)
        print(f"{case.name}: {'PASS' if valid else 'FAIL'} "
              f"(seed={seed_result['passed']}, reference={reference_result['passed']}, "
              f"mutants rejected={sum(not m['passed'] for m in mutants.values())}/{len(mutants)})",
              flush=True)
        if not valid:
            for name, value in [("seed", seed_result), ("reference", reference_result), *mutants.items()]:
                print(name, json.dumps(value, ensure_ascii=False), flush=True)
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    validate = sub.add_parser("validate", help="offline seed/reference/mutation checks")
    validate.add_argument("--output", type=Path)
    anchor = sub.add_parser("validate-anchors", help="upstream stubs fail and upstream examples pass")
    anchor.add_argument("--output", type=Path)
    sub.add_parser("list", help="list public cases and content hashes")
    args = parser.parse_args()
    if args.command == "list":
        for case in sorted(CASES.iterdir()):
            print(case.name, digest_tree(case))
        return 0
    if args.command == "validate-anchors":
        results = []
        for case in sorted((ROOT / "anchors").iterdir()):
            if not (case / "case.json").is_file():
                continue
            metadata = json.loads((case / "case.json").read_text())
            with tempfile.TemporaryDirectory(prefix="niucode-anchor-") as directory:
                work = seed(case, Path(directory) / "work")
                original = grade_local(case, work)
                extension = "py" if metadata["language"] == "python" else "rs"
                (work / metadata["solution_files"][0]).write_bytes((case / f"reference.{extension}").read_bytes())
                reference = grade_local(case, work)
                row = {"case": case.name, "sha256": digest_tree(case), "seed": original,
                       "reference": reference, "valid": not original["passed"] and reference["passed"]}
                results.append(row)
                print(case.name, "PASS" if row["valid"] else "FAIL", flush=True)
    else:
        results = self_validate()
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps({"schema_version": 1, "kind": "case_validation",
                                           "results": results}, indent=2, ensure_ascii=False) + "\n")
    expected = 4 if args.command == "validate-anchors" else 8
    return 0 if len(results) == expected and all(r["valid"] for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())

