#!/usr/bin/env bash
#
# Builds the CLI's Debian packages, origincloud-cli_<version>_<arch>.deb for
# amd64 and arm64, from the release's Linux binaries. packages.origincloud.eu
# publishes them to its apt repository, and reads the download channel's
# version from them.
#
#   release/deb.sh <version> <binary-dir> <out-dir>
#
# <version> is crates/cli/Cargo.toml's, X.Y.Z or X.Y.Z-<suffix>. The suffix
# goes behind Debian's `~`, so that a prerelease sorts before its release:
# 1.2.0-rc.1 is packaged as 1.2.0~rc.1.
# <binary-dir> holds origin-linux-amd64 and origin-linux-arm64.
#
# A package holds /usr/bin/origin and the licence, as
# /usr/share/doc/origincloud-cli/copyright. It is reproducible: fixed
# modification times (SOURCE_DATE_EPOCH, by default the last commit's time),
# root-owned, xz. A version in packages.origincloud.eu's pool is immutable,
# so a rerun of a release has to rebuild the same bytes from the same
# binaries.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly PACKAGE=origincloud-cli
readonly ARCHS="amd64 arm64"

fail() { printf '::error::%s\n' "$*" >&2; exit 1; }

[[ $# -eq 3 ]] || fail "usage: $0 <version> <binary-dir> <out-dir>"
readonly CLI_VERSION="$1" BINARY_DIR="$2" OUT_DIR="$3"
[[ "${CLI_VERSION}" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z]+(\.[0-9A-Za-z]+)*)?$ ]] \
  || fail "'${CLI_VERSION}' isn't X.Y.Z or X.Y.Z-<suffix>"
readonly VERSION="${CLI_VERSION/-/\~}"
command -v dpkg-deb >/dev/null || fail "dpkg-deb is required"

epoch="${SOURCE_DATE_EPOCH:-$(git -C "${ROOT}" log -1 --format=%ct 2>/dev/null || echo 0)}"
work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
mkdir -p "${OUT_DIR}"

for arch in ${ARCHS}; do
  binary="${BINARY_DIR}/origin-linux-${arch}"
  [[ -f "${binary}" ]] || fail "${binary} doesn't exist"
  root="${work}/${arch}"
  mkdir -p "${root}/DEBIAN" "${root}/usr/bin" "${root}/usr/share/doc/${PACKAGE}"
  install -m 0755 "${binary}" "${root}/usr/bin/origin"
  install -m 0644 "${ROOT}/LICENSE" "${root}/usr/share/doc/${PACKAGE}/copyright"
  # Installed-Size, in KiB, from the files' sizes alone: a directory's size
  # depends on the filesystem it was built on.
  size="$(find "${root}/usr" -type f -printf '%s\n' | awk '{ kib += int(($1 + 1023) / 1024) } END { print kib }')"
  cat > "${root}/DEBIAN/control" <<CONTROL
Package: ${PACKAGE}
Version: ${VERSION}
Architecture: ${arch}
Maintainer: Origin Cloud <release@origincloud.eu>
Installed-Size: ${size}
Section: utils
Priority: optional
Homepage: https://github.com/origincloudhq/cli
Description: Origin Cloud command-line interface
 The origin command manages Origin Cloud organisations, projects, access,
 instances, networks and volumes from a terminal or a script.
CONTROL
  # Directories are 0755 whatever the caller's umask, so the bytes don't
  # depend on who builds.
  find "${root}" -type d -exec chmod 0755 {} +
  find "${root}" -exec touch -h -d "@${epoch}" {} +
  SOURCE_DATE_EPOCH="${epoch}" dpkg-deb --root-owner-group -Zxz --build "${root}" \
    "${OUT_DIR}/${PACKAGE}_${VERSION}_${arch}.deb" >/dev/null
  printf '%s\n' "${OUT_DIR}/${PACKAGE}_${VERSION}_${arch}.deb"
done
