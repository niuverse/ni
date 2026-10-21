# Public external anchor

Four exercises exported from [Aider Polyglot](https://github.com/Aider-AI/polyglot-benchmark)
at `7e0611e77b54e2dea774cdc0aa00cf9f7ed6144f`, originally authored by
[Exercism Python](https://github.com/exercism/python) and
[Exercism Rust](https://github.com/exercism/rust). Copyright © Exercism;
MIT license is retained in `LICENSE.exercism`. Per-exercise author/contributor
credits are preserved in `upstream-config.json`.

Selection was fixed before model attempts: two small, dependency-free exercises
per language, covering roster state, text transposition, word counting and byte
encoding. This is a convenience subset, not a representative random sample.

Original source stubs, test files and manifests are copied verbatim. Instructions
add only the common standard-library constraint and test command. Reference
examples stay outside agent workspaces. `import_polyglot.py` regenerates exports
from a clean checkout of the pinned revision into a new empty anchors directory.

The grader restores original tests/manifests and accepts only declared solution
files. Rust runs `cargo test --offline -- --include-ignored`; Python discovers
`*_test.py`. `bench.py validate-anchors` proves all four stubs fail and all four
upstream examples pass. This is evaluation using upstream tests, **not the official
Aider leaderboard protocol**: tests are visible, and each harness gets its full
bounded tool loop rather than Aider's fixed two-attempt procedure. Public task
familiarity and training contamination cannot be excluded. Keep these scores
separate from the newly authored synthetic tasks.
