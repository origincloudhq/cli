#!/usr/bin/env bash
#
# Puts together what a release publishes, from the four binaries
# release/build.sh made:
#
#   release/assemble.sh <binary-dir> <out-dir>
#
#   origin-<platform>                 the binaries, one per platform
#   origincloud-cli_<version>_<arch>.deb
#                                     the packages, for amd64 and arm64
#   install.sh, LICENSE               the installer and the licence
#   commands.json                     the command tree (`origin __commands`)
#   SHA256SUMS                        over all of the above
#
# That is what packages.origincloud.eu's `cli` channel reads from the
# release, but commands.json, which is the GitHub release's alone: the
# tools naming `origin`'s commands and flags read it there. The version is
# crates/cli/Cargo.toml's, which release/version.sh has checked against the
# tag.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly PLATFORMS="linux-amd64 linux-arm64 darwin-arm64 darwin-amd64"

fail() { printf '::error::%s\n' "$*" >&2; exit 1; }

[[ $# -eq 2 ]] || fail "usage: $0 <binary-dir> <out-dir>"
readonly BINARY_DIR="$1" OUT_DIR="$2"

version="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\(.*\)"[[:space:]]*$/\1/p' "${ROOT}/crates/cli/Cargo.toml" | head -1)"
[[ -n "${version}" ]] || fail "crates/cli/Cargo.toml has no package version"

mkdir -p "${OUT_DIR}"
[[ -z "$(ls -A "${OUT_DIR}")" ]] || fail "${OUT_DIR} isn't empty: SHA256SUMS would cover whatever else is in it"

for platform in ${PLATFORMS}; do
  binary="${BINARY_DIR}/origin-${platform}"
  [[ -f "${binary}" ]] || fail "origin-${platform} is missing: a platform's build didn't produce it"
  # Again here, on the bytes that are published: an artifact passes through
  # other jobs on its way.
  "${ROOT}/release/check-arch.sh" "${platform}" "${binary}"
  install -m 0755 "${binary}" "${OUT_DIR}/origin-${platform}"
done

# Printed by the published binary of this machine, so that it is exactly
# this version's tree; dpkg-deb, below, needs Linux anyway.
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) host=linux-amd64 ;;
  Linux-aarch64) host=linux-arm64 ;;
  *) fail "the command tree is printed by a published binary, which needs Linux on amd64 or arm64, not $(uname -s)-$(uname -m)" ;;
esac
"${OUT_DIR}/origin-${host}" __commands > "${OUT_DIR}/commands.json"

"${ROOT}/release/deb.sh" "${version}" "${OUT_DIR}" "${OUT_DIR}"
install -m 0755 "${ROOT}/release/install.sh" "${OUT_DIR}/install.sh"
install -m 0644 "${ROOT}/LICENSE" "${OUT_DIR}/LICENSE"

# Generated where the files are put together, over exactly what is published:
# packages.origincloud.eu checks every asset against it before it publishes.
(
  cd "${OUT_DIR}"
  # The glob expands before the redirection creates the file, so the list
  # doesn't name itself.
  sha256sum -- * > SHA256SUMS
  sha256sum --quiet -c SHA256SUMS
  cat SHA256SUMS
)
