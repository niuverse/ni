# Small-repository harness comparison

Same-model comparison of niucode, mini-SWE-agent, DeepSeek Harness and Pi.
Eight newly authored synthetic tasks plus a separate four-exercise public
Polyglot subset. These measure bounded code changes, not general agent quality.
The initial campaign uses **one attempt per task × harness × track: 96 scored
runs**, with no best-of selection. Repeated trials are a follow-up needed before
claims about stability. Development smoke attempts and their costs remain visible.

## Cases and independent validation

| Case | Language | Change | Main checks |
|---|---|---|---|
| py-config | Python | Bug | Precedence, falsey values, invalid JSON, no mutation |
| py-pagination | Python | Bug | Empty pages, duplicate IDs, cycles, local HTTP |
| py-tasks | Python | Feature | Status filter, JSON, text compatibility, CLI errors |
| py-csv | Python | Feature | Quoting, decimal totals, missing values, invalid values |
| rs-unicode | Rust | Bug | Unicode scalar limits, boundaries, CLI compatibility |
| rs-scan | Rust | Bug | Path components, prefix collisions, ordering, symlinks |
| rs-dry-run | Rust | Feature | Preview/real write agreement, content and mtime unchanged |
| rs-cache | Rust | Feature | Expiry boundary, zero TTL, saturation, overwrite |

Each synthetic case includes a seed repository, public requirements, a reference
patch, a grader and two plausible incorrect patches. The old smoke tests pass
on the seed, new requirements fail, the reference passes both, and both mutants
fail. Agents receive only `seed/` and `task.md`; reference patches and acceptance
tests remain outside their containers. Public availability means these tests are
not a long-term held-out set. See [anchors/README.md](anchors/README.md) for the
external dataset, licensing, selection and scoring differences.

```sh
python3 -m unittest discover -s benchmarks -p 'test_*.py'
python3 benchmarks/bench.py validate --output /tmp/case-validation.json
python3 benchmarks/bench.py validate-anchors --output /tmp/anchor-validation.json
```

## Frozen comparison

`experiment.json`, `adapter.py`, npm's integrity lock and Python's hashed lock
define the experiment. Use DeepSeek `deepseek-flash`, thinking enabled, high
effort, 8192 output tokens and a configured context budget of 64000 where the
harness supports it. mini uses its native loop without context compaction;
the gateway's reservation still covers DeepSeek's full 1M context capacity.
The service alias is not an immutable model snapshot. DSH uses the official
Messages endpoint; the other three use Chat Completions. Native system prompts
and tool schemas are retained. This compares complete harness configurations;
it does not isolate a single algorithm or prompt component.

- **Basic:** niucode's six coding tools; Pi's read/bash/edit/write; mini's bash
  loop; DSH's coding profile with delegation and filesystem skills disabled.
- **Enhanced:** the same explicit verification workflow appended to each task.
  niucode/DSH retain native delegation; Pi loads its pinned official subagent
  example with worker/reviewer profiles; mini remains a single agent. Delegation
  is permitted, not forced. This is not each framework's optimized ceiling.
- All runs: fresh workspace/home, 4 GiB, two CPU cores, 256 processes, 900 seconds,
  40 paid requests including retries and child agents. Native tool timeouts may
  differ. No human feedback during scored runs. Fixed shuffled execution order.
- Grading: separate read-only-root container with no network, original tests and
  manifests restored, only candidate source admitted. Timeout/agent error remains
  a failed complete run even if its partial patch happens to pass.

The internal Docker network has no Internet route; the host metering gateway is
reachable. This setup is for the supplied benign tasks, not a sandbox service for
hostile submissions. The Docker socket and real credentials are never mounted.

## Build and run (Linux x86_64)

Requirements: Docker, Rust 1.96.0, Node 26.0.0/npm, Python 3.12, uv, Git and a
compatible Linux C toolchain. The base image is pinned by digest. `build.py`
installs locked dependencies, compiles niucode, copies local toolchains and
records binary/source/library hashes in `/opt/toolchains.json`. Local toolchain
copies make rebuilds auditable, not automatically byte-identical across hosts.
Use the recorded image ID to replay the exact local environment; image publication
is a separate release step.

```sh
python3 benchmarks/build.py --context /tmp/niucode-bench-image
python3 benchmarks/live.py --campaign /tmp/niucode-campaign --prepare-network

# Keep this running in another terminal. Credentials resolve through niucode-auth.
cargo run --release -p niucode-bench-gateway -- \
  --listen 172.30.98.1:18765 --journal /tmp/niucode-campaign/billing.jsonl
# Optional explicit import: --credential-shell-file "$HOME/.zshrc"
# Only a literal DEEPSEEK_API_KEY assignment is accepted; shell code is never run.

python3 benchmarks/live.py --campaign /tmp/niucode-campaign --smoke --timeout 120
# Inspect smoke results first: all four harnesses and both tracks must pass.
python3 benchmarks/live.py --campaign /tmp/niucode-campaign --repeats 1
python3 benchmarks/live.py --campaign /tmp/niucode-campaign --suite polyglot --repeats 1
python3 benchmarks/report.py /tmp/niucode-campaign --output /tmp/niucode-report
python3 benchmarks/audit.py /tmp/niucode-campaign --output /tmp/niucode-campaign/audit.json
# After stopping the gateway, export immutable evidence to a new directory:
python3 benchmarks/export.py /tmp/niucode-campaign \
  --output /tmp/niucode-release --toolchains /tmp/niucode-bench-image/toolchains.json
```

Commands need access to Docker and the gateway needs outbound HTTPS. Never give
the agent container a real API key. The generated `adapter.json`, console/session
or trajectory logs, candidate diff, `grade.json`, `result.json`, frozen plans and
append-only billing journal are the reviewable evidence. Completed run IDs are
not overwritten; configuration changes refuse resume. An interrupted directory
without a result requires explicit inspection rather than silent rerunning.

## Budget and interpretation

One campaign has a **CNY 300** cap, including smoke/debug/retry/child requests.
Each call reserves CNY 2.162688 before network I/O (1,048,576 input and 8192 output
tokens at the verified peak rates). Final provider usage lowers that reservation;
missing usage retains it. Journal writes are synced, replay is fail-closed, and
only one gateway can own a journal. Keep the same journal across restarts.
A stale lock requires confirming the old gateway exited before removing it.
Starting a new journal starts a new budget; do not split one campaign to evade
the cap. Recheck [official pricing](https://api-docs.deepseek.com/zh-cn/quick_start/pricing/)
before a later campaign. Reported CNY is a conservative upper bound, not the
account's actual charge; cache/off-peak discounts are ignored.
Harness-local cost displays (including zero-valued custom-provider prices) are
not used for comparison; the independent gateway's provider usage is authoritative.

The gateway serializes upstream calls, including child-agent calls. Timing thus
describes this controlled environment, not unconstrained parallel throughput.
Small convenience samples, one initial repeat, model-alias drift, protocol and
native-prompt differences limit attribution. Do not merge synthetic and Polyglot
scores into a headline rank. M5/M6 real-terminal, cancellation/resume, long-context
and endurance acceptance remain independent gates.
