#!/usr/bin/env bash
#
# Moves the CLI to the newest API definitions its MCP tools have: the MCP
# dependency to the head of origincloudhq/mcp's main, the SDK dependency to
# the origincloudhq/sdk-rust commit that one is built on, API_REVISION to the
# API revision that commit was generated from, then generates the commands
# again (scripts/generate.sh). The regeneration workflow
# (.github/workflows/regenerate.yml) runs it daily; run it by hand the same
# way.
#
# The MCP tools lead, and the SDK before them: `origin mcp serve` hands the
# tools the SDK's `Client`, so the CLI and the tools must use one SDK
# commit, and the generated commands use the SDK's types, so the CLI can
# only move to an API revision the SDK has been generated from.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly SDK_REPOSITORY="https://github.com/origincloudhq/sdk-rust.git"
readonly MCP_REPOSITORY="https://github.com/origincloudhq/mcp.git"

cd "${ROOT}"

mcp="$(git ls-remote "${MCP_REPOSITORY}" refs/heads/main | cut -f1)"
[[ "${mcp}" =~ ^[0-9a-f]{40}$ ]] || { echo "no main branch in ${MCP_REPOSITORY}" >&2; exit 1; }

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

# The SDK commit the MCP tools are built on.
git init --quiet "${tmp}/mcp"
git -C "${tmp}/mcp" fetch --quiet --depth 1 "${MCP_REPOSITORY}" "${mcp}"
sdk="$(git -C "${tmp}/mcp" show FETCH_HEAD:Cargo.toml | grep -m1 -oE 'sdk-rust", rev = "[0-9a-f]{40}"' | grep -oE '[0-9a-f]{40}')"
[[ "${sdk}" =~ ^[0-9a-f]{40}$ ]] || { echo "the MCP commit ${mcp:0:12} pins no commit of ${SDK_REPOSITORY}" >&2; exit 1; }
if ! grep -qF "rev = \"${mcp}\"" Cargo.toml; then
  sed -i -E "s|(git = \"https://github.com/origincloudhq/mcp\", rev = \")[0-9a-f]{40}(\")|\1${mcp}\2|" Cargo.toml
  cargo update --quiet -p origincloud-mcp
fi
git init --quiet "${tmp}/sdk"
git -C "${tmp}/sdk" fetch --quiet --depth 1 "${SDK_REPOSITORY}" "${sdk}"
revision="$(git -C "${tmp}/sdk" show FETCH_HEAD:API_REVISION | tr -d '[:space:]')"
[[ "${revision}" =~ ^[0-9a-f]{40}$ ]] || { echo "the SDK commit ${sdk:0:12} has no API_REVISION" >&2; exit 1; }

if ! grep -qF "rev = \"${sdk}\"" Cargo.toml; then
  sed -i -E "s|(git = \"https://github.com/origincloudhq/sdk-rust\", rev = \")[0-9a-f]{40}(\")|\1${sdk}\2|" Cargo.toml
  # Every crate Cargo.toml takes from the SDK: a service's crate is added
  # there when its command group is (CONTRIBUTING.md).
  packages=()
  while read -r crate; do
    packages+=(-p "${crate}")
  done < <(grep -oE '^[a-z0-9-]+ = \{ git = "https://github.com/origincloudhq/sdk-rust"' Cargo.toml | cut -d' ' -f1)
  [[ "${#packages[@]}" -gt 0 ]] || { echo "Cargo.toml takes no crate from ${SDK_REPOSITORY}" >&2; exit 1; }
  cargo update --quiet "${packages[@]}"
fi
echo "${revision}" > API_REVISION
# Adds what main's Cargo.toml needs and the lock doesn't hold yet, such as
# after a lock conflict was resolved with the branch's side, so that the
# result builds with --locked; it moves no other dependency.
cargo update --quiet --workspace

echo "origincloudhq/mcp@${mcp:0:12}, origincloudhq/sdk-rust@${sdk:0:12}, origincloudhq/api@${revision:0:12}"
scripts/generate.sh
