#!/usr/bin/env bash
#
# Covers release/version.sh: which tags are releases, which are
# prereleases, which are refused, and that the tag must be
# crates/cli/Cargo.toml's version. A release becomes packages.origincloud.eu's
# cli/latest, which install.sh downloads, so a pattern that quietly stops
# matching would put a test build on customers' machines.
#
# Runs against a copy of the script beside a manifest of its own, so that
# the cases don't depend on the version the repository is at.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT

failures=0
ok()  { printf '  ok   %s\n' "$1"; }
bad() { printf '  FAIL %s\n' "$1"; failures=$((failures + 1)); }

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
mkdir -p "${work}/release" "${work}/crates/cli"
cp "${ROOT}/release/version.sh" "${work}/release/"

# manifest <version>: crates/cli/Cargo.toml at that version, with a
# dependency below it that has a version of its own.
manifest() {
  printf '[package]\nname = "origincloud-cli"\nversion = "%s"\n\n[dependencies]\nfoo = { version = "9.9.9" }\nversion = "0.0.0"\n' "$1" \
    > "${work}/crates/cli/Cargo.toml"
}

# expect <false|true|refused> <tag>, with the manifest at the tag's version.
expect() {
  local want="$1" tag="$2" out got
  manifest "${tag#v}"
  if out="$("${work}/release/version.sh" "${tag}" 2>/dev/null)"; then
    got="$(sed -n 's/^prerelease=//p' <<< "${out}")"
    [[ "$(sed -n 's/^version=//p' <<< "${out}")" == "${tag#v}" ]] || got="wrong version"
  else
    got=refused
  fi
  if [[ "${got}" == "${want}" ]]; then
    ok "${tag:-<empty>} -> ${got}"
  else
    bad "${tag:-<empty>} -> ${got}, expected ${want}"
  fi
}

expect false   v1.10.0
expect false   v0.0.1
expect false   v10.20.30
expect true    v1.11.0-rc.1
expect true    v2.0.0-beta
expect true    v2.0.0-rc.1.2
expect refused ""
expect refused 1.10.0          # no leading v
expect refused v1.10           # two components
expect refused v1.10.0.1       # four components
expect refused v01.2.3         # a leading zero
expect refused v1.2.3+build    # build metadata
expect refused v1.2.3-         # an empty suffix
expect refused v1.2.3-rc-1     # a `-` in the suffix, a Debian revision
expect refused v1.2.3-rc..1    # an empty identifier
expect refused latest
expect refused main

manifest 1.10.0
if "${work}/release/version.sh" v1.11.0 >/dev/null 2>&1; then
  bad "a tag that isn't crates/cli/Cargo.toml's version must be refused"
else
  ok "refuses a tag that isn't crates/cli/Cargo.toml's version"
fi

if "${work}/release/version.sh" >/dev/null 2>&1; then
  bad "no tag must be refused"
else
  ok "refuses to run without a tag"
fi

if (( failures > 0 )); then
  printf "::error::%d case(s) failed - release/version.sh doesn't behave as documented\n" "${failures}" >&2
  exit 1
fi
printf 'release/version.sh: all cases passed\n'
