#!/bin/sh
#
# Installs the Origin Cloud CLI (`origin`).
#
#   curl -fsSL https://packages.origincloud.eu/install.sh | sh
#
# Environment:
#   ORIGIN_VERSION      version to install, e.g. v1.10.0 (default: the
#                       latest stable release, read from cli/latest)
#   ORIGIN_INSTALL_DIR  where to put the binary (default: $HOME/.local/bin)
#   ORIGIN_BASE_URL     where to fetch from (default:
#                       https://packages.origincloud.eu). For a
#                       mirror of the same layout - the checksum is still
#                       verified, so a mirror cannot serve a different
#                       binary than the release it copied.
#
# POSIX sh, not bash: this runs on whatever the customer has, including a
# minimal container image where /bin/sh is dash or busybox ash.
#
# Everything is a function and `main` is called on the LAST line. A pipe into
# a shell executes what has arrived so far, so a download truncated mid-way
# through would otherwise run half a script - with this shape it defines some
# functions and does nothing.

set -eu

BASE_URL="https://packages.origincloud.eu"
DOCS_URL="https://github.com/origincloudhq/cli#install"
BINARY="origin"

log()  { printf '%s\n' "$*"; }
err()  { printf 'error: %s\n' "$*" >&2; exit 1; }

# Maps uname output onto the platform names the release assets use. These
# have to match release/build.sh's platforms exactly; a mismatch
# here is a 404 at download time.
detect_platform() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "${os}" in
        Linux)  os_name="linux" ;;
        Darwin) os_name="darwin" ;;
        *) err "unsupported operating system: ${os}. The CLI ships for Linux and macOS; see ${DOCS_URL} to build it for another platform." ;;
    esac

    case "${arch}" in
        x86_64|amd64)  arch_name="amd64" ;;
        aarch64|arm64) arch_name="arm64" ;;
        *) err "unsupported architecture: ${arch}. The CLI ships for x86_64 and arm64." ;;
    esac

    printf '%s-%s' "${os_name}" "${arch_name}"
}

# One of these must exist. We do NOT fall back to installing unverified:
# skipping the checksum on a binary fetched over the network is the whole
# risk this script exists to manage.
checksum_of() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d' ' -f1
    else
        err "neither sha256sum nor shasum found - cannot verify the download, and installing an unverified binary is not offered. Install coreutils (Linux) or follow the manual install at ${DOCS_URL}."
    fi
}

download() {
    url="$1"
    dest="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "${url}" -o "${dest}" || err "download failed: ${url}"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "${dest}" "${url}" || err "download failed: ${url}"
    else
        err "neither curl nor wget found"
    fi
}

main() {
    platform="$(detect_platform)"
    asset="${BINARY}-${platform}"

    root="${ORIGIN_BASE_URL:-${BASE_URL}}"
    root="${root%/}"
    install_dir="${ORIGIN_INSTALL_DIR:-${HOME}/.local/bin}"

    tmp="$(mktemp -d)"
    # Cleared on every exit path, including the error ones above: a failed
    # install must not leave a half-downloaded binary behind for someone to
    # find and run later.
    trap 'rm -rf "${tmp}"' EXIT INT TERM

    version="${ORIGIN_VERSION:-}"
    if [ -z "${version}" ]; then
        download "${root}/cli/latest" "${tmp}/latest"
        version="$(tr -d ' \t\r\n' < "${tmp}/latest")"
    fi
    # The version becomes part of a URL and a log line; anything but a
    # version string here means the pointer or the variable is wrong.
    case "${version}" in
        v[0-9]*) ;;
        *) err "'${version}' is not a CLI version (expected e.g. v1.10.0)" ;;
    esac
    base="${root}/cli/${version}"

    log "Installing ${BINARY} ${version} (${platform}) into ${install_dir}"

    download "${base}/${asset}" "${tmp}/${asset}"
    download "${base}/SHA256SUMS" "${tmp}/SHA256SUMS"

    expected="$(grep " \{1,2\}${asset}\$" "${tmp}/SHA256SUMS" | cut -d' ' -f1)"
    [ -n "${expected}" ] || err "${asset} is not listed in SHA256SUMS - refusing to install something the release does not vouch for."

    actual="$(checksum_of "${tmp}/${asset}")"
    if [ "${expected}" != "${actual}" ]; then
        err "checksum mismatch for ${asset}
  expected ${expected}
  actual   ${actual}
The download is corrupt or has been tampered with. Nothing was installed."
    fi

    mkdir -p "${install_dir}"
    chmod +x "${tmp}/${asset}"
    # Move into place only after verification, so an interrupted or failed
    # run never leaves a runnable binary at the destination.
    mv "${tmp}/${asset}" "${install_dir}/${BINARY}"

    log "Installed ${install_dir}/${BINARY}"

    if ! printf '%s' ":${PATH}:" | grep -q ":${install_dir}:"; then
        log ""
        log "${install_dir} is not on your PATH. Add it:"
        log "    export PATH=\"${install_dir}:\$PATH\""
    fi

    log ""
    log "Next: ${BINARY} id login"
}

main "$@"
