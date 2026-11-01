#!/usr/bin/env bash
#
# Generates the command groups of every service of the API (`origin pvs`,
# `origin location`, ...) from the API definitions in origincloudhq/api at
# the commit API_REVISION names, with this repository's generator
# (generator/), built at the versions Cargo.lock pins, into
# crates/cli/src/sdk/generated/.
#
#   scripts/generate.sh           # generate them again
#   scripts/generate.sh --check   # check the checked-in code is current
#
# The commands use the Rust SDK's request and response types, so the SDK
# commit Cargo.lock pins must have been generated from the same API
# revision: both modes check that its API_REVISION is this one. The
# generator also reads the SDK's clients, which say which services answer
# at a region's host. The MCP tools `origin mcp serve` serves
# (origincloudhq/mcp) must be built on that same SDK commit, which both
# modes check too.
#
# The tests' scratch service (testing/scratch/origincloud/) is generated
# too: its SDK crate's code, by the SDK's own generator at that commit, into
# testing/scratch/sdk/src/generated/, and its command group beside the
# others, compiled only with the CLI's `test-services` feature. Its
# manifest, testing/scratch/sdk/Cargo.toml, takes its edition and its
# dependencies from the manifest the SDK's generator scaffolds for a new
# service, so that the crate builds as the SDK's own crates do.
#
# Needs cargo, git and network access: the API's dependencies are
# downloaded from the Buf Schema Registry at the commits its buf.lock pins,
# and checked against the digests it pins.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly API_REPOSITORY="https://github.com/origincloudhq/api.git"
readonly SDK_REPOSITORY="https://github.com/origincloudhq/sdk-rust.git"
readonly MCP_REPOSITORY="https://github.com/origincloudhq/mcp.git"
readonly GENERATED=("crates/cli/src/sdk/generated" "testing/scratch/sdk/src/generated" "testing/scratch/sdk/Cargo.toml")

case "${1:-}" in
  "") mode="write" ;;
  --check) mode="check" ;;
  *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

revision="$(tr -d '[:space:]' < "${ROOT}/API_REVISION")"
[[ "${revision}" =~ ^[0-9a-f]{40}$ ]] || { echo "API_REVISION must hold a full commit hash" >&2; exit 1; }

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

# The SDK commit Cargo.lock pins, and the API revision it was generated from.
sdk="$(grep -m1 -o 'git+https://github.com/origincloudhq/sdk-rust?rev=[0-9a-f]*#[0-9a-f]\{40\}' "${ROOT}/Cargo.lock" | sed 's/.*#//')"
[[ "${sdk}" =~ ^[0-9a-f]{40}$ ]] || { echo "Cargo.lock pins no commit of ${SDK_REPOSITORY}" >&2; exit 1; }
git init --quiet "${tmp}/sdk"
git -C "${tmp}/sdk" fetch --quiet --depth 1 "${SDK_REPOSITORY}" "${sdk}"
sdk_revision="$(git -C "${tmp}/sdk" show FETCH_HEAD:API_REVISION | tr -d '[:space:]')"
if [[ "${sdk_revision}" != "${revision}" ]]; then
  echo "FAILED: the SDK commit ${sdk:0:12} was generated from origincloudhq/api@${sdk_revision:0:12}, not API_REVISION's ${revision:0:12}; run scripts/update.sh, or pin the SDK commit generated from it"
  exit 1
fi
git -C "${tmp}/sdk" checkout --quiet FETCH_HEAD

# The MCP tools `origin mcp serve` serves must be built on the same SDK
# commit: the CLI hands them the SDK's `Client`.
mcp="$(grep -m1 -o 'git+https://github.com/origincloudhq/mcp?rev=[0-9a-f]*#[0-9a-f]\{40\}' "${ROOT}/Cargo.lock" | sed 's/.*#//')"
[[ "${mcp}" =~ ^[0-9a-f]{40}$ ]] || { echo "Cargo.lock pins no commit of ${MCP_REPOSITORY}" >&2; exit 1; }
git init --quiet "${tmp}/mcp"
git -C "${tmp}/mcp" fetch --quiet --depth 1 "${MCP_REPOSITORY}" "${mcp}"
mcp_sdk="$(git -C "${tmp}/mcp" show FETCH_HEAD:Cargo.toml | grep -m1 -oE 'sdk-rust", rev = "[0-9a-f]{40}"' | grep -oE '[0-9a-f]{40}')"
if [[ "${mcp_sdk}" != "${sdk}" ]]; then
  echo "FAILED: the MCP commit ${mcp:0:12} is built on origincloudhq/sdk-rust@${mcp_sdk:0:12}, not the CLI's ${sdk:0:12}; run scripts/update.sh"
  exit 1
fi

git init --quiet "${tmp}/api"
git -C "${tmp}/api" fetch --quiet --depth 1 "${API_REPOSITORY}" "${revision}"
git -C "${tmp}/api" checkout --quiet FETCH_HEAD

# The SDK's code for the API and the scratch service, by the SDK's
# generator, built at the versions the SDK's Cargo.lock pins, in a target
# directory of its own beside this repository's.
cp -R "${tmp}/api" "${tmp}/api-scratch"
cp -R "${ROOT}/testing/scratch/origincloud/." "${tmp}/api-scratch/origincloud/"
(cd "${tmp}/sdk" && CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT}/target}/sdk-generator" \
  cargo run --quiet --locked -p origincloud-sdk-generator -- "${tmp}/api-scratch" "${tmp}/sdk-gen") \
  2> >(grep -v '^notice: origincloud.scratch is a new service' >&2)
mkdir -p "${tmp}/gen/testing/scratch/sdk/src"
cp -R "${tmp}/sdk-gen/crates/scratch/src/generated" "${tmp}/gen/testing/scratch/sdk/src/"

# The lines of a section of a manifest.
section() { awk -v name="[$2]" '$0 == name { on = 1; next } /^\[/ { on = 0 } on && NF' "$1"; }
edition="$(section "${tmp}/sdk/Cargo.toml" workspace.package | grep '^edition = ' || true)"
dependencies="$(section "${tmp}/sdk-gen/crates/scratch/Cargo.toml" dependencies \
  | sed -E 's/^origincloud-core = .*/origincloud-core.workspace = true/')"
[[ -n "${edition}" && -n "${dependencies}" ]] || { echo "the SDK's scaffolded manifest has no edition or dependencies" >&2; exit 1; }
cat > "${tmp}/gen/testing/scratch/sdk/Cargo.toml" <<MANIFEST
# The SDK crate of the tests' scratch service (testing/scratch/origincloud/):
# its code is generated by the Rust SDK's own generator, at the commit
# Cargo.lock pins, and this manifest from the one it scaffolds for a new
# service, by scripts/generate.sh; don't edit either. Only the CLI's
# test-services feature uses it.
[package]
name = "origincloud-scratch"
version = "0.0.0"
${edition}
license.workspace = true
publish = false
description = "The SDK crate of the CLI tests' scratch service."

[dependencies]
${dependencies}
MANIFEST

(cd "${ROOT}" && cargo run --quiet --locked -p origincloud-cli-generator -- \
  "${tmp}/api" "${tmp}/sdk-gen" "${tmp}/gen" "${ROOT}/testing/scratch")

# The generated files, relative to the repository.
generated() {
  local dir
  for dir in "${GENERATED[@]}"; do
    [[ -e "$1/${dir}" ]] || continue
    (cd "$1" && find "${dir}" -type f)
  done | LC_ALL=C sort
}

if [[ "${mode}" == write ]]; then
  generated "${ROOT}" | (cd "${ROOT}" && xargs -r rm -f)
  generated "${tmp}/gen" | (cd "${tmp}/gen" && xargs -r cp --parents -t "${ROOT}")
  echo "wrote $(generated "${ROOT}" | wc -l) files from origincloudhq/api@${revision:0:12}"
  exit 0
fi

failed=0
if ! diff <(generated "${ROOT}") <(generated "${tmp}/gen") >/dev/null; then
  failed=1
else
  while read -r file; do
    cmp -s "${ROOT}/${file}" "${tmp}/gen/${file}" || failed=1
  done < <(generated "${ROOT}")
fi
if [[ "${failed}" -eq 1 ]]; then
  echo "FAILED: the generated code isn't what origincloudhq/api@${revision:0:12} generates; run $0"
  exit 1
fi
echo "ok - the generated code is current (origincloudhq/api@${revision:0:12})"
