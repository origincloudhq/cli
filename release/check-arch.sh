#!/usr/bin/env bash
#
# Refuses a binary whose architecture isn't its platform's. release/build.sh
# runs it on every build, before the binary can reach a release: a cross
# build that silently produced a host binary would otherwise reach customers
# as the wrong platform.
#
#   release/check-arch.sh <platform> <binary>
#
# <platform> is one of release/build.sh's. The header is read with od, which
# coreutils has on every host, rather than `file`, which minimal images lack.
#
#   linux:  ELF, 64-bit, little-endian; e_machine at offset 18 (u16)
#   darwin: Mach-O 64-bit, little-endian; cputype at offset 4 (u32)
set -euo pipefail

fail() { printf '::error::%s\n' "$*" >&2; exit 1; }

[[ $# -eq 2 ]] || fail "usage: $0 <platform> <binary>"
readonly PLATFORM="$1" BINARY="$2"
[[ -f "${BINARY}" ]] || fail "${BINARY} doesn't exist"

# The <count> bytes at <offset>, as lowercase hex without separators.
bytes() { od -An -v -tx1 -j "$1" -N "$2" "${BINARY}" | tr -d ' \n'; }

case "${PLATFORM}" in
  linux-amd64)  format=elf   want=3e00 ;;     # EM_X86_64
  linux-arm64)  format=elf   want=b700 ;;     # EM_AARCH64
  darwin-amd64) format=macho want=07000001 ;; # CPU_TYPE_X86_64
  darwin-arm64) format=macho want=0c000001 ;; # CPU_TYPE_ARM64
  *) fail "unknown platform '${PLATFORM}'" ;;
esac

case "${format}" in
  elf)
    [[ "$(bytes 0 6)" == 7f454c460201 ]] || fail "${BINARY} isn't a 64-bit little-endian ELF file"
    got="$(bytes 18 2)"
    ;;
  macho)
    [[ "$(bytes 0 4)" == cffaedfe ]] || fail "${BINARY} isn't a 64-bit little-endian Mach-O file"
    got="$(bytes 4 4)"
    ;;
esac

[[ "${got}" == "${want}" ]] \
  || fail "${BINARY} isn't ${PLATFORM}: its ${format} machine field is ${got}, ${PLATFORM}'s is ${want}"
printf '%s: a %s binary\n' "${BINARY}" "${PLATFORM}"
