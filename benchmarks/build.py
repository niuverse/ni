#!/usr/bin/env python3
"""Prepare a Linux x86_64 image context from pinned dependencies and local toolchains."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent


def run(*args, **kwargs):
    subprocess.run(args, check=True, **kwargs)


def output(*args):
    return subprocess.check_output(args, text=True).strip()


def source_hashes():
    paths = [REPO / "Cargo.toml", REPO / "Cargo.lock"]
    for folder in ["crates", "benchmarks/gateway"]:
        paths.extend(p for p in (REPO / folder).rglob("*") if p.is_file())
    return {p.relative_to(REPO).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(paths) if "__pycache__" not in p.parts and "target" not in p.parts}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--context", type=Path, required=True)
    parser.add_argument("--tag", default="niucode-bench:m7-v1")
    args = parser.parse_args()
    work = args.context.resolve()
    if work.exists() and any(work.iterdir()):
        raise SystemExit("use a new empty context directory")
    work.mkdir(parents=True, exist_ok=True)
    config = json.loads((ROOT / "experiment.json").read_text())
    mini = config["harnesses"]["mini"]
    run("git", "clone", mini["repo"], str(work / "mini"))
    run("git", "-C", str(work / "mini"), "checkout", "--detach", mini["commit"])
    for name in ["Dockerfile", "package.json", "package-lock.json", "adapter.py"]:
        shutil.copy2(ROOT / name, work / name)
    run("npm", "ci", "--ignore-scripts", "--no-audit", "--no-fund", cwd=work)
    run("uv", "pip", "install", "--python", "python3.12", "--target", str(work / "python-deps"),
        "--require-hashes", "-r", str(ROOT / "requirements.lock"))
    run("uv", "pip", "install", "--python", "python3.12", "--target", str(work / "python-deps"),
        "--no-deps", str(work / "mini"))
    run("cargo", "build", "--release", "-p", "niucode", "-p", "niucode-bench-gateway", cwd=REPO)
    shutil.copy2(REPO / "target/release/niucode", work / "niucode")
    rust = Path(output("rustc", "--print", "sysroot"))
    for directory in ["bin", "lib"]:
        shutil.copytree(rust / directory, work / "rust" / directory)
    shutil.copy2(Path(shutil.which("node")).resolve(), work / "node")
    compat = work / "compat"
    compat.mkdir()
    # Only non-libc system dependencies. The pinned Debian base supplies libc/compiler.
    for line in output("ldd", str(work / "niucode")).splitlines():
        if "=>" not in line:
            continue
        name, rest = line.split("=>", 1)
        name = name.strip()
        if name in {"libc.so.6", "libm.so.6", "libgcc_s.so.1"}:
            continue
        source = Path(rest.strip().split()[0])
        if source.is_file():
            shutil.copy2(source, compat / name)
    provenance = {
        "rust": output("rustc", "-Vv"), "node": output("node", "--version"),
        "niucode_sha256": hashlib.sha256((work / "niucode").read_bytes()).hexdigest(),
        "niucode_git_head": output("git", "-C", str(REPO), "rev-parse", "HEAD"),
        "niucode_dirty": bool(output("git", "-C", str(REPO), "status", "--porcelain")),
        "mini_commit": mini["commit"],
        "source_sha256": source_hashes(),
        "compat_sha256": {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in compat.iterdir()},
    }
    (work / "toolchains.json").write_text(json.dumps(provenance, indent=2) + "\n")
    (work / ".dockerignore").write_text("mini/.git\nmini/tests\nrust/lib/rustlib/src\n")
    run("docker", "build", "-t", args.tag, str(work))
    print(output("docker", "image", "inspect", args.tag, "--format", "{{.Id}}"))


if __name__ == "__main__":
    main()

