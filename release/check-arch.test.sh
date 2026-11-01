#!/usr/bin/env bash
#
# Covers release/check-arch.sh with synthetic headers: each
# platform's binary passes, another platform's is refused, and so is a file
# that is not an executable of the platform's format at all.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly CHECK="${ROOT}/release/check-arch.sh"

failures=0
ok()  { printf '  ok   %s\n' "$1"; }
bad() { printf '  FAIL %s\n' "$1"; failures=$((failures + 1)); }

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

# A 64-byte file <name> that starts with the bytes <escapes>, zero-padded.
header() { { printf '%b' "$2"; head -c 64 /dev/zero; } | head -c 64 > "${work}/$1"; }

elf='\x7fELF\x02\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00'
header linux-amd64  "${elf}\x3e\x00"
header linux-arm64  "${elf}\xb7\x00"
header darwin-amd64 '\xcf\xfa\xed\xfe\x07\x00\x00\x01'
header darwin-arm64 '\xcf\xfa\xed\xfe\x0c\x00\x00\x01'
header elf32-arm    '\x7fELF\x01\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x02\x00\x28\x00'
printf '#!/bin/sh\n' > "${work}/script"

for platform in linux-amd64 linux-arm64 darwin-amd64 darwin-arm64; do
  if "${CHECK}" "${platform}" "${work}/${platform}" >/dev/null 2>&1; then
    ok "accepts a ${platform} binary as ${platform}"
  else
    bad "should accept a ${platform} binary as ${platform}"
  fi
done

refuses() {
  local platform="$1" file="$2" why="$3"
  if "${CHECK}" "${platform}" "${work}/${file}" >/dev/null 2>&1; then
    bad "should refuse ${why}"
  else
    ok "refuses ${why}"
  fi
}
refuses linux-arm64  linux-amd64  "an amd64 ELF as linux-arm64 (a cross-build that produced a host binary)"
refuses linux-amd64  linux-arm64  "an arm64 ELF as linux-amd64"
refuses darwin-arm64 darwin-amd64 "an x86_64 Mach-O as darwin-arm64"
refuses darwin-arm64 linux-arm64  "an ELF as a darwin binary"
refuses linux-arm64  darwin-arm64 "a Mach-O as a linux binary"
refuses linux-arm64  elf32-arm    "a 32-bit ELF"
refuses linux-amd64  script       "a file that is not an executable"
refuses linux-amd64  missing      "a missing file"
refuses windows-amd64 linux-amd64 "an unknown platform"

if (( failures > 0 )); then
  printf "::error::%d case(s) failed - release/check-arch.sh doesn't behave as documented\n" "${failures}" >&2
  exit 1
fi
printf 'release/check-arch.sh: all cases passed\n'
