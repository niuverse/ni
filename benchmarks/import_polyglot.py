#!/usr/bin/env python3
"""Export four preselected upstream exercises, retaining original tests and attribution."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

from bench import ROOT, digest_tree

COMMIT = "7e0611e77b54e2dea774cdc0aa00cf9f7ed6144f"
SELECTION = [("python", "grade-school"), ("python", "transpose"),
             ("rust", "word-count"), ("rust", "variable-length-quantity")]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checkout", type=Path)
    args = parser.parse_args()
    if subprocess.check_output(["git", "-C", str(args.checkout), "rev-parse", "HEAD"], text=True).strip() != COMMIT:
        raise SystemExit("upstream revision mismatch")
    if subprocess.check_output(["git", "-C", str(args.checkout), "status", "--porcelain"], text=True).strip():
        raise SystemExit("upstream checkout must be clean")
    for language, name in SELECTION:
        source = args.checkout / language / "exercises/practice" / name
        config = json.loads((source / ".meta/config.json").read_text())
        case = ROOT / "anchors" / f"polyglot-{'py' if language == 'python' else 'rs'}-{name}"
        case.mkdir(parents=True, exist_ok=False)
        seed = case / "seed"
        seed.mkdir()
        solutions = [p for p in config["files"]["solution"] if p.endswith((".py", ".rs"))]
        paths = config["files"]["solution"] + config["files"]["test"]
        for relative in paths:
            target = seed / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source / relative, target)
        instructions = (source / ".docs/instructions.md").read_text()
        extra = source / ".docs/instructions.append.md"
        if extra.exists():
            instructions += "\n" + extra.read_text()
        command = ("python3 -m unittest discover -p '*_test.py'" if language == "python"
                   else "cargo test --offline -- --include-ignored")
        task = (instructions + "\n\nImplement the existing API. Use only the standard library. "
                "Do not change tests or build manifests. Original tests are available in the workspace.\n"
                f"Run all tests with `{command}`.\n")
        (case / "task.md").write_text(task)
        (seed / "README.md").write_text(task)
        (case / "upstream-config.json").write_text(json.dumps(config, indent=2) + "\n")
        example = config["files"]["example"][0]
        shutil.copyfile(source / example, case / ("reference.py" if language == "python" else "reference.rs"))
        metadata = {"id": case.name, "language": language, "suite": "polyglot-subset",
                    "upstream": "https://github.com/Aider-AI/polyglot-benchmark", "commit": COMMIT,
                    "upstream_path": str(source.relative_to(args.checkout)), "upstream_sha256": digest_tree(source),
                    "solution_files": solutions, "test_files": config["files"]["test"],
                    "grading": "upstream tests; Rust ignored tests explicitly included",
                    "license": "MIT; see ../LICENSE.exercism"}
        (case / "case.json").write_text(json.dumps(metadata, indent=2) + "\n")
        print(case.name, digest_tree(case))


if __name__ == "__main__":
    main()
