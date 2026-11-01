#!/usr/bin/env bash
#
# Checks a release tag, before anything is built, and says whether it is a
# release or a prerelease:
#
#   release/version.sh <tag>
#
#   vX.Y.Z           a release:    prints prerelease=false
#   vX.Y.Z-<suffix>  a prerelease: prints prerelease=true; never the latest
#   anything else    refused
#
# and version=<the tag without its v>, both in the key=value form
# $GITHUB_OUTPUT takes. The tag's version must be crates/cli/Cargo.toml's:
# `origin --version` reports that one, so a tag that differs would publish a
# binary that names another version.
#
# The tag's shape decides, not a checkbox someone has to remember. The
# packages carry the version too, with the suffix behind Debian's `~` so that
# 1.2.0~rc.1 sorts before 1.2.0, and packages.origincloud.eu writes that `~`
# back as `-` for the channel's directory. So the suffix is dot-separated
# letters and digits, without `-` (rc.1, beta.2): a `-` in it would be a
# Debian revision. Numbers have no leading zeros, as in semver; `+build`
# metadata is refused.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly MANIFEST="${ROOT}/crates/cli/Cargo.toml"

fail() { printf '::error::%s\n' "$*" >&2; exit 1; }

[[ $# -eq 1 ]] || fail "usage: $0 <tag>"
readonly TAG="$1"

readonly CORE='(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)'
readonly SUFFIX='[0-9A-Za-z]+(\.[0-9A-Za-z]+)*'

if [[ "${TAG}" =~ ^v${CORE}$ ]]; then
  prerelease=false
elif [[ "${TAG}" =~ ^v${CORE}-${SUFFIX}$ ]]; then
  prerelease=true
else
  fail "'${TAG}' isn't a release tag: vX.Y.Z for a release, vX.Y.Z-<suffix> for a prerelease (v1.2.0-rc.1), with a leading v, no leading zeros, and a suffix of dot-separated letters and digits"
fi

# The [package] version: the first `version =` line of the manifest, before
# any dependency table.
manifest_version="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\(.*\)"[[:space:]]*$/\1/p' "${MANIFEST}" | head -1)"
[[ -n "${manifest_version}" ]] || fail "crates/cli/Cargo.toml has no package version"
[[ "${manifest_version}" == "${TAG#v}" ]] \
  || fail "the tag ${TAG} isn't crates/cli/Cargo.toml's version, ${manifest_version}: bump the version first (RELEASING.md)"

printf 'prerelease=%s\nversion=%s\n' "${prerelease}" "${TAG#v}"
