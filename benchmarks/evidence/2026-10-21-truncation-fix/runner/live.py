#!/usr/bin/env python3
"""Container runner; sources, oracle and billing journal never enter agent containers."""
import argparse
import difflib
import hashlib
import itertools
import json
import os
from pathlib import Path
import random
import signal
import subprocess
import tempfile
import time

from bench import ROOT, CASES, digest_tree, execute, seed

NETWORK = "niucode-bench-m7"
GATEWAY = "172.30.98.1"
IMAGE = "niucode-bench:m7-v1"


def checked(args, **kwargs):
    return subprocess.check_output(args, text=True, **kwargs).strip()


def prepare_network():
    result = subprocess.run(["docker", "network", "inspect", NETWORK], capture_output=True, text=True)
    if result.returncode:
        checked(["docker", "network", "create", "--internal", "--subnet", "172.30.98.0/24", NETWORK])
    value = json.loads(checked(["docker", "network", "inspect", NETWORK]))[0]
    if not value["Internal"] or value["IPAM"]["Config"][0]["Gateway"] != GATEWAY:
        raise RuntimeError("benchmark network configuration mismatch")


def snapshot_candidate(case, candidate, destination):
    """Rebuild from trusted seed; accept source files only, never agent tests/manifests."""
    seed(case, destination)
    metadata = json.loads((case / "case.json").read_text())
    allowed = metadata.get("solution_files")
    for path in candidate.rglob("*"):
        relative = path.relative_to(candidate)
        if allowed is not None and relative.as_posix() not in allowed:
            continue
        if any(part in {".git", "target", "__pycache__", "tests", ".agents", ".pi", ".niucode"} for part in relative.parts):
            continue
        if path.is_symlink():
            raise RuntimeError(f"candidate symlink refused: {relative}")
        if path.is_file() and (path.suffix == ".py" or (relative.parts[0] == "src" and path.suffix == ".rs")):
            if path.stat().st_size > 1_000_000:
                raise RuntimeError("candidate source exceeds budget")
            output = destination / relative
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(path.read_bytes())
    # Deletions of original source are meaningful.
    for path in (case / "seed").rglob("*"):
        relative = path.relative_to(case / "seed")
        if allowed is not None and relative.as_posix() not in allowed:
            continue
        if "tests" not in relative.parts and path.suffix in {".py", ".rs"} and not (candidate / relative).exists():
            (destination / relative).unlink(missing_ok=True)


def grade(case, candidate, output, image):
    with tempfile.TemporaryDirectory(prefix="niucode-grade-") as directory:
        work = Path(directory) / "work"
        snapshot_candidate(case, candidate, work)
        script = Path(directory) / "grade.py"
        script.write_text(
            "import sys,json\nfrom pathlib import Path\nsys.path.insert(0,'/judge')\n"
            "from bench import grade_local\n"
            "print(json.dumps(grade_local(Path('/case'),Path('/workspace'))))\n")
        name = "niucode-grade-" + hashlib.sha256(str(directory).encode()).hexdigest()[:10]
        args = ["docker", "run", "--rm", "--user", f"{os.getuid()}:{os.getgid()}", "--name", name, "--network", "none", "--read-only",
                "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--memory", "2g",
                "--cpus", "2", "--pids-limit", "128", "--tmpfs", "/tmp:rw,exec,size=256m",
                "--tmpfs", "/home/bench:rw,size=32m,mode=1777",
                "-v", f"{work}:/workspace:rw", "-v", f"{case}:/case:ro",
                "-v", f"{ROOT / 'bench.py'}:/judge/bench.py:ro",
                "-v", f"{script}:/judge/grade.py:ro",
                "--entrypoint", "python3", image, "/judge/grade.py"]
        try:
            result = execute(args, ROOT, timeout=240)
        finally:
            subprocess.run(["docker", "rm", "-f", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if result["exit_code"] or result["timeout"]:
            return {"passed": False, "status": "grader_error", "detail": result}
        try:
            value = json.loads(result["output_tail"].strip().splitlines()[-1])
        except (ValueError, IndexError):
            return {"passed": False, "status": "grader_error", "detail": result}
        value["status"] = "passed" if value["passed"] else "failed"
        output.write_text(json.dumps(value, indent=2) + "\n")
        return value


def run_one(case, harness, track, run_id, campaign, image, timeout):
    directory = campaign / "runs" / run_id
    directory.mkdir(parents=True)  # Refuse accidental overwrite/retry selection.
    work = seed(case, directory / "workspace")
    artifacts = directory / "artifacts"
    artifacts.mkdir()
    checked(["git", "init", "-q", str(work)])
    checked(["git", "-C", str(work), "add", "."])
    checked(["git", "-C", str(work), "-c", "user.name=Benchmark", "-c", "user.email=benchmark@localhost",
             "commit", "-qm", "Fixed task seed"])
    name = "niucode-" + run_id
    endpoint = f"http://{GATEWAY}:18765/{run_id}"
    args = ["docker", "run", "--rm", "--user", f"{os.getuid()}:{os.getgid()}", "--name", name, "--network", NETWORK, "--read-only",
            "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--memory", "4g",
            "--cpus", "2", "--pids-limit", "256", "--tmpfs", "/tmp:rw,exec,size=512m",
            "--tmpfs", "/home/bench:rw,exec,size=256m,mode=1777",
            "-v", f"{work}:/workspace:rw", "-v", f"{artifacts}:/artifacts:rw",
            "-v", f"{case / 'task.md'}:/task.md:ro",
            image, harness, track, endpoint, "/task.md"]
    started = time.monotonic()
    timed_out = False
    with (directory / "console.log").open("wb") as log:
        child = subprocess.Popen(args, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            child.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
        finally:
            subprocess.run(["docker", "rm", "-f", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait()
    elapsed = time.monotonic() - started
    # Never run candidate Git configuration/hooks on the host after the agent exits.
    diffs = []
    for path in sorted(work.rglob("*")):
        relative = path.relative_to(work)
        if any(p in {".git", "target", "__pycache__"} for p in relative.parts):
            continue
        if path.is_file() and not path.is_symlink() and path.stat().st_size < 1_000_000:
            original = case / "seed" / relative
            before = original.read_text(errors="replace") if original.is_file() else ""
            after = path.read_text(errors="replace")
            diffs.extend(difflib.unified_diff(before.splitlines(True), after.splitlines(True),
                fromfile="a/" + relative.as_posix(), tofile="b/" + relative.as_posix()))
    (directory / "candidate.diff").write_text("".join(diffs))
    try:
        assessment = grade(case, work, directory / "grade.json", image)
    except (OSError, RuntimeError) as error:
        assessment = {"passed": False, "status": "grader_error", "error": str(error)}
        (directory / "grade.json").write_text(json.dumps(assessment, indent=2) + "\n")
    status = ("timeout" if timed_out else "harness_error" if child.returncode else assessment["status"])
    result = {"schema_version": 1, "run": run_id, "case": case.name, "harness": harness,
              "track": track, "status": status, "candidate_passed": assessment["passed"],
              "resolved": status == "passed", "exit_code": child.returncode,
              "seconds": round(elapsed, 3), "case_sha256": digest_tree(case)}
    (directory / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result), flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--campaign", type=Path, required=True)
    parser.add_argument("--image", default=IMAGE)
    parser.add_argument("--cases", nargs="+", default=[p.name for p in sorted(CASES.iterdir())])
    parser.add_argument("--harnesses", nargs="+", choices=["niucode", "mini", "dsh", "pi"], default=["niucode", "mini", "dsh", "pi"])
    parser.add_argument("--tracks", nargs="+", choices=["basic", "enhanced"], default=["basic", "enhanced"])
    parser.add_argument("--repeats", type=int, default=1)
    parser.add_argument("--timeout", type=int, default=900)
    parser.add_argument("--smoke", action="store_true", help="use only independent development task")
    parser.add_argument("--suite", choices=["synthetic", "polyglot"], default="synthetic")
    parser.add_argument("--prepare-network", action="store_true")
    args = parser.parse_args()
    args.campaign = args.campaign.resolve()
    args.campaign.mkdir(parents=True, exist_ok=True)
    prepare_network()
    if args.prepare_network:
        print(f"Start gateway with --listen {GATEWAY}:18765 --journal {args.campaign / 'billing.jsonl'}")
        return
    if not (args.campaign / "billing.lock").exists():
        raise RuntimeError("metered gateway is not running for this campaign")
    image = checked(["docker", "image", "inspect", args.image, "--format", "{{.Id}}"])
    cases = ([ROOT / "dev" / "smoke"] if args.smoke else
             sorted(p for p in (ROOT / "anchors").iterdir() if (p / "case.json").exists())
             if args.suite == "polyglot" else [CASES / name for name in args.cases])
    if any(not (case / "case.json").is_file() for case in cases):
        raise ValueError("unknown case")
    frozen = {"experiment": json.loads((ROOT / "experiment.json").read_text()),
              "adapter_sha256": hashlib.sha256((ROOT / "adapter.py").read_bytes()).hexdigest(),
              "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "grader_sha256": hashlib.sha256((ROOT / "bench.py").read_bytes()).hexdigest(),
              "image": image, "cases": {case.name: digest_tree(case) for case in cases},
              "harnesses": args.harnesses, "tracks": args.tracks, "repeats": args.repeats,
              "timeout": args.timeout, "smoke_only": args.smoke, "suite": args.suite}
    smoke_tag = hashlib.sha256(json.dumps(frozen, sort_keys=True).encode()).hexdigest()[:8]
    path = args.campaign / (f"smoke-plan-{smoke_tag}.json" if args.smoke else f"frozen-plan-{args.suite}.json")
    if path.exists():
        if json.loads(path.read_text()) != frozen:
            raise RuntimeError("campaign config changed; do not mix incompatible runs")
    else:
        path.write_text(json.dumps(frozen, indent=2) + "\n")
    runs = list(itertools.product(range(args.repeats), cases, args.tracks, args.harnesses))
    random.Random(20261021).shuffle(runs)
    for repeat, case, track, harness in runs:
        run_id = f"{'smoke-' + smoke_tag if args.smoke else 'test'}-{case.name}-{harness}-{track}-{repeat}"
        result = args.campaign / "runs" / run_id / "result.json"
        if result.exists():
            continue
        run_one(case, harness, track, run_id, args.campaign, image, args.timeout)


if __name__ == "__main__":
    main()

