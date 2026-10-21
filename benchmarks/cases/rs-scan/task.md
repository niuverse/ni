# File inventory
Fix exclusions: a relative rule excludes that exact path and descendants by path
components, not string prefix. Ignore empty rules. Inputs are normalized relative
paths without ... collect(root,rules) returns sorted slash-separated file paths.
Do not follow symlinks. Preserve APIs/CLI. Existing tests: cargo test --offline

Use only the standard library. Do not add dependencies or change build manifests.
