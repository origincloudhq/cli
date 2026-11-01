#!/usr/bin/env bash
#
# Covers release/deb.sh: the package's name, version and contents, the
# prerelease mapping, and that a rebuild from the same binaries is
# byte-identical, which the immutable pool at packages.origincloud.eu relies
# on for a rerun of a release.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly BUILD="${ROOT}/release/deb.sh"

failures=0
ok()  { printf '  ok   %s\n' "$1"; }
bad() { printf '  FAIL %s\n' "$1"; failures=$((failures + 1)); }

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

mkdir -p "${work}/bin"
for arch in amd64 arm64; do
  printf '#!/bin/sh\necho "origin %s"\n' "${arch}" > "${work}/bin/origin-linux-${arch}"
done

if SOURCE_DATE_EPOCH=1700000000 "${BUILD}" 1.9.16 "${work}/bin" "${work}/one" >/dev/null \
   && [[ "$(dpkg-deb -f "${work}/one/origincloud-cli_1.9.16_amd64.deb" Package Version Architecture | tr '\n' ' ')" \
         == "Package: origincloud-cli Version: 1.9.16 Architecture: amd64 " ]] \
   && [[ "$(dpkg-deb -f "${work}/one/origincloud-cli_1.9.16_arm64.deb" Architecture)" == arm64 ]]; then
  ok "builds origincloud-cli_<version>_<arch>.deb for amd64 and arm64, named after their control fields"
else
  bad "should build both packages with matching control fields"
fi

dpkg-deb -x "${work}/one/origincloud-cli_1.9.16_arm64.deb" "${work}/x"
if cmp -s "${work}/x/usr/bin/origin" "${work}/bin/origin-linux-arm64" \
   && [[ "$(stat -c %a "${work}/x/usr/bin/origin")" == 755 ]] \
   && dpkg-deb -c "${work}/one/origincloud-cli_1.9.16_arm64.deb" | grep -q '^-rwxr-xr-x root/root .* ./usr/bin/origin$'; then
  ok "installs the architecture's binary as /usr/bin/origin, root-owned and executable"
else
  bad "/usr/bin/origin should be the arm64 binary, 0755, root:root"
fi

dpkg-deb -e "${work}/one/origincloud-cli_1.9.16_arm64.deb" "${work}/control"
if [[ "$(awk '{ print $2 }' "${work}/control/md5sums" | tr '\n' ' ')" == "usr/bin/origin usr/share/doc/origincloud-cli/copyright " ]] \
   && (cd "${work}/x" && md5sum --quiet -c "${work}/control/md5sums"); then
  ok "DEBIAN/md5sums lists every file, matching what the package installs"
else
  bad "DEBIAN/md5sums should list /usr/bin/origin and the copyright file with their checksums"
fi

if cmp -s "${work}/x/usr/share/doc/origincloud-cli/copyright" "${ROOT}/LICENSE"; then
  ok "carries the licence as /usr/share/doc/origincloud-cli/copyright"
else
  bad "/usr/share/doc/origincloud-cli/copyright should be the repository's LICENSE"
fi

if SOURCE_DATE_EPOCH=1700000000 "${BUILD}" 1.9.16 "${work}/bin" "${work}/two" >/dev/null \
   && cmp -s "${work}/one/origincloud-cli_1.9.16_amd64.deb" "${work}/two/origincloud-cli_1.9.16_amd64.deb" \
   && cmp -s "${work}/one/origincloud-cli_1.9.16_arm64.deb" "${work}/two/origincloud-cli_1.9.16_arm64.deb"; then
  ok "a rebuild from the same binaries is byte-identical"
else
  bad "two builds from the same binaries must be byte-identical"
fi

if SOURCE_DATE_EPOCH=1700000000 "${BUILD}" 1.9.16-rc.1 "${work}/bin" "${work}/rc" >/dev/null \
   && [[ -f "${work}/rc/origincloud-cli_1.9.16~rc.1_amd64.deb" ]] \
   && dpkg --compare-versions 1.9.16~rc.1 lt 1.9.16; then
  ok "a pre-release 1.9.16-rc.1 becomes 1.9.16~rc.1, which sorts before 1.9.16"
else
  bad "1.9.16-rc.1 should be packaged as 1.9.16~rc.1"
fi

for version in v1.9.16 1.9 1.9.16-rc-1 1.9.16-; do
  if "${BUILD}" "${version}" "${work}/bin" "${work}/bad" >/dev/null 2>&1; then
    bad "the version ${version} must be refused"
  else
    ok "refuses ${version}, which isn't X.Y.Z or X.Y.Z-<suffix>"
  fi
done

rm "${work}/bin/origin-linux-arm64"
if "${BUILD}" 1.9.17 "${work}/bin" "${work}/missing" >/dev/null 2>&1; then
  bad "a missing binary must fail the build"
else
  ok "fails when a Linux binary is missing"
fi

if (( failures > 0 )); then
  printf "::error::%d case(s) failed - release/deb.sh doesn't behave as documented\n" "${failures}" >&2
  exit 1
fi
printf 'release/deb.sh: all cases passed\n'
