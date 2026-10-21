#!/usr/bin/env python3
"""Export reviewable campaign evidence without build outputs or credentials."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tarfile

from bench import ROOT
from report import report, markdown


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("campaign", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--toolchains", type=Path, required=True)
    args = parser.parse_args()
    if (args.campaign / "billing.lock").exists():
        raise SystemExit("stop the gateway before exporting an immutable campaign")
    args.output.mkdir(parents=True, exist_ok=False)
    summary = report(args.campaign)
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    (args.output / "results.md").write_text(markdown(summary))
    shutil.copy2(args.toolchains, args.output / "toolchains.json")
    for path in sorted(args.campaign.glob("*.json*")):
        shutil.copy2(path, args.output / path.name)
    # Include complete traces and candidate source, never target/.git/bytecode caches.
    # Symlinks are not followed or exported, even when present in candidate workspaces.
    excluded = {"target", ".git", "__pycache__", "node_modules"}
    manifest = {}
    with tarfile.open(args.output / "attempts.tar.gz", "w:gz") as archive:
        for path in sorted((args.campaign / "runs").rglob("*")):
            relative = path.relative_to(args.campaign)
            if any(part in excluded for part in relative.parts) or path.is_symlink() or not path.is_file():
                continue
            manifest[relative.as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
            archive.add(path, arcname=relative.as_posix(), recursive=False)
    (args.output / "attempts-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    # Preserve exact dirty niucode source used to build the evaluated binary.
    provenance = json.loads(args.toolchains.read_text())
    with tarfile.open(args.output / "niucode-source.tar.gz", "w:gz") as archive:
        for relative, expected in provenance["source_sha256"].items():
            path = ROOT.parent / relative
            if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
                raise SystemExit(f"source changed after binary build: {relative}")
            archive.add(path, arcname=relative, recursive=False)
        for name in [".cargo/config.toml"]:
            archive.add(ROOT.parent / name, arcname=name)
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
              for p in sorted(args.output.iterdir()) if p.is_file()}
    (args.output / "SHA256SUMS.json").write_text(json.dumps(hashes, indent=2) + "\n")
    print(f"Exported {len(summary['runs'])} attempts and {len(manifest)} evidence files")


if __name__ == "__main__":
    main()
