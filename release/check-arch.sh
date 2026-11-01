#!/usr/bin/env bash
#
# Refuses a binary whose architecture isn't its platform's, and a Linux
# binary that isn't statically linked. release/build.sh runs it on every
# build, and release/assemble.sh again on what is published: a cross build
# that silently produced a host binary would otherwise reach customers as the
# wrong platform, and a dynamically linked one would fail on every
# distribution whose C library differs from the build image's.
#
#   release/check-arch.sh <platform> <binary>
#
# <platform> is one of release/build.sh's. The header is read with od, which
# coreutils has on every host, rather than `file`, which minimal images lack.
#
#   linux:  ELF, 64-bit, little-endian; e_machine at offset 18 (u16); no
#           program header of type PT_INTERP (3), which names the dynamic
#           loader of a dynamically linked binary (a static-pie has none)
#   darwin: Mach-O 64-bit, little-endian; cputype at offset 4 (u32)
set -euo pipefail

fail() { printf '::error::%s\n' "$*" >&2; exit 1; }

[[ $# -eq 2 ]] || fail "usage: $0 <platform> <binary>"
readonly PLATFORM="$1" BINARY="$2"
[[ -f "${BINARY}" ]] || fail "${BINARY} doesn't exist"

# The <count> bytes at <offset>, as lowercase hex without separators.
bytes() { od -An -v -tx1 -j "$1" -N "$2" "${BINARY}" | tr -d ' \n'; }

# The little-endian unsigned number of <count> bytes at <offset>, read byte
# by byte, so the host's own byte order doesn't matter.
number() {
  local hex reversed="" i
  hex="$(bytes "$1" "$2")"
  [[ ${#hex} -eq $(($2 * 2)) ]] || fail "${BINARY} ends before offset $(($1 + $2))"
  for ((i = ${#hex} - 2; i >= 0; i -= 2)); do reversed+="${hex:i:2}"; done
  printf '%d' "$((16#${reversed}))"
}

# No PT_INTERP among the program headers: e_phoff (u64 at 32), e_phentsize
# (u16 at 54), e_phnum (u16 at 56), p_type the first u32 of each header.
static_elf() {
  local phoff phentsize phnum i
  phoff="$(number 32 8)" phentsize="$(number 54 2)" phnum="$(number 56 2)"
  (( phnum == 0 || phentsize >= 4 )) || fail "${BINARY} has program headers of ${phentsize} bytes"
  for ((i = 0; i < phnum; i++)); do
    if (( $(number $((phoff + i * phentsize)) 4) == 3 )); then
      fail "${BINARY} is dynamically linked: it has an interpreter (PT_INTERP); the Linux builds must be static"
    fi
  done
}

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
if [[ "${format}" == elf ]]; then
  static_elf
  printf '%s: a static %s binary\n' "${BINARY}" "${PLATFORM}"
else
  printf '%s: a %s binary\n' "${BINARY}" "${PLATFORM}"
fi
