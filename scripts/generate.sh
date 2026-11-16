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
# The changes that can't be undone, which need `--yes`, are origincloudhq/mcp's
# `[[one_way]]` list, the one list of them: its generator/tools.toml at the
# commit Cargo.lock pins is handed to the generator, and copied to
# crates/cli/src/sdk/generated/mcp-tools.toml, from which the tests check
# that each of them needs `--yes`. That copy is never edited: a one-way
# change is added in origincloudhq/mcp, and reaches the CLI with
# scripts/update.sh.
#
# The tests' scratch service (testing/scratch/origincloud/) is generated
# too: its SDK crate's code, by the SDK's own generator at that commit, into
# testing/scratch/sdk/src/generated/, and its command group beside the
# others, compiled only with the CLI's `test-services` feature. Its
# manifest, testing/scratch/sdk/Cargo.toml, takes its edition and its
# dependencies from the manifest the SDK's generator scaffolds for a new
# service, so that the crate builds as the SDK's own crates do.
#
# The agent skills `origin skill install` installs are copied from
# origincloudhq/skills at the commit SKILLS_REVISION names: each skill's
# directory, skills/{skill}/, into crates/cli/skills/{skill}/, without its
# evals/, which are for the skills' authors, not for agents. The CLI embeds
# that copy, so that installing a skill needs no network access.
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
readonly SKILLS_REPOSITORY="https://github.com/origincloudhq/skills.git"
readonly GENERATED=("crates/cli/src/sdk/generated" "testing/scratch/sdk/src/generated" "testing/scratch/sdk/Cargo.toml" "crates/cli/skills")
readonly MCP_TABLE="crates/cli/src/sdk/generated/mcp-tools.toml"

case "${1:-}" in
  "") mode="write" ;;
  --check) mode="check" ;;
  *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

revision="$(tr -d '[:space:]' < "${ROOT}/API_REVISION")"
[[ "${revision}" =~ ^[0-9a-f]{40}$ ]] || { echo "API_REVISION must hold a full commit hash" >&2; exit 1; }

skills_revision="$(tr -d '[:space:]' < "${ROOT}/SKILLS_REVISION")"
[[ "${skills_revision}" =~ ^[0-9a-f]{40}$ ]] || { echo "SKILLS_REVISION must hold a full commit hash" >&2; exit 1; }

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
# Its table of what the API definitions don't express, whose [[one_way]]
# list the commands follow.
mkdir -p "$(dirname "${tmp}/gen/${MCP_TABLE}")"
git -C "${tmp}/mcp" show FETCH_HEAD:generator/tools.toml > "${tmp}/gen/${MCP_TABLE}" \
  || { echo "origincloudhq/mcp@${mcp:0:12} has no generator/tools.toml" >&2; exit 1; }

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

# The agent skills, without their evals. The CLI writes each skill's files
# as they are, so a skill holds regular files only: no links, nothing
# executable.
git init --quiet "${tmp}/skills"
git -C "${tmp}/skills" fetch --quiet --depth 1 "${SKILLS_REPOSITORY}" "${skills_revision}"
git -C "${tmp}/skills" checkout --quiet FETCH_HEAD
[[ -d "${tmp}/skills/skills" ]] || { echo "origincloudhq/skills@${skills_revision:0:12} has no skills/" >&2; exit 1; }
mkdir -p "${tmp}/gen/crates/cli/skills"
for skill in "${tmp}/skills/skills"/*/; do
  name="$(basename "${skill}")"
  [[ -f "${skill}/SKILL.md" ]] || continue
  [[ "${name}" =~ ^[a-z0-9]+(-[a-z0-9]+)*$ ]] || { echo "the skill ${name} isn't named as the Agent Skills specification requires" >&2; exit 1; }
  cp -R "${skill}" "${tmp}/gen/crates/cli/skills/${name}"
  rm -rf "${tmp}/gen/crates/cli/skills/${name}/evals"
done
if [[ -n "$(find "${tmp}/gen/crates/cli/skills" ! -type f ! -type d)" || -n "$(find "${tmp}/gen/crates/cli/skills" -type f -perm /111)" ]]; then
  echo "origincloudhq/skills@${skills_revision:0:12} has a link, or an executable file, in a skill" >&2
  exit 1
fi
[[ -f "${tmp}/gen/crates/cli/skills/origin/SKILL.md" ]] || { echo "origincloudhq/skills@${skills_revision:0:12} has no origin skill" >&2; exit 1; }

(cd "${ROOT}" && cargo run --quiet --locked -p origincloud-cli-generator -- \
  "${tmp}/api" "${tmp}/sdk-gen" "${tmp}/gen/${MCP_TABLE}" "${tmp}/gen" "${ROOT}/testing/scratch")

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
  # A skill gone, or a directory of one, goes with its files.
  for dir in "${GENERATED[@]}"; do
    if [[ -d "${ROOT}/${dir}" ]]; then find "${ROOT}/${dir}" -mindepth 1 -type d -empty -delete; fi
  done
  generated "${tmp}/gen" | (cd "${tmp}/gen" && xargs -r cp --parents -t "${ROOT}")
  echo "wrote $(generated "${ROOT}" | wc -l) files from origincloudhq/api@${revision:0:12} and origincloudhq/skills@${skills_revision:0:12}"
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
  echo "FAILED: the generated code isn't what origincloudhq/api@${revision:0:12} and origincloudhq/mcp@${mcp:0:12}'s table generate, or the skills aren't origincloudhq/skills@${skills_revision:0:12}'s; run $0"
  exit 1
fi
echo "ok - the generated code is current (origincloudhq/api@${revision:0:12}, origincloudhq/skills@${skills_revision:0:12})"
