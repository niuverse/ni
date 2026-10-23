#!/usr/bin/env bash
# Publish the niucode crates in dependency order.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

mode="${1:---check}"
order=(
    niucode-core
    niucode-computer
    niucode-computer-x11
    niucode-computer-macos
    niucode-attachments
    niucode-auth
    niucode-provider
    niucode-tools
    niucode-extension
    niucode-mcp
    niucode-agent
    niucode
)

version="$(grep -A1 '^\[workspace.package\]' Cargo.toml | grep '^version' | cut -d'"' -f2)"
license="$(grep -E '^license' Cargo.toml || true)"
token="${CARGO_REGISTRY_TOKEN:-}"
credentials="${CARGO_HOME:-$HOME/.cargo}/credentials.toml"

echo "version: $version"
echo "license: ${license:-MISSING}"
if [ -n "$token" ]; then
    echo "token:   environment"
elif [ -f "$credentials" ]; then
    echo "token:   $credentials"
else
    echo "token:   MISSING (run cargo login or set CARGO_REGISTRY_TOKEN)"
fi
echo "order:   ${order[*]}"

if [ "$mode" != "--execute" ]; then
    echo "check only; pass --execute to publish"
    exit 0
fi

if [ -z "$license" ]; then
    echo "refusing to publish without a license in [workspace.package]" >&2
    exit 1
fi
if [ -z "$token" ] && [ ! -f "$credentials" ]; then
    echo "refusing to publish without a crates.io token" >&2
    exit 1
fi
if [ -n "$(git status --porcelain)" ]; then
    echo "refusing to publish from a dirty tree" >&2
    exit 1
fi

for crate in "${order[@]}"; do
    prefix="${crate:0:2}/${crate:2:2}"
    if curl -fsSL "https://index.crates.io/$prefix/$crate" 2>/dev/null \
        | grep -q "\"vers\":\"$version\""
    then
        echo "== $crate $version already published; skipping =="
        continue
    fi
    echo "== publishing $crate =="
    cargo publish -p "$crate"
    # Give the sparse index time to expose the new version to dependents.
    sleep 20
done
