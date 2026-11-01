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
#   ORIGIN_BASE_URL     an https mirror of packages.origincloud.eu's layout to
#                       download the binary from. Only the binary: cli/latest,
#                       SHA256SUMS and its signature always come from
#                       packages.origincloud.eu, and with a mirror the
#                       signature must verify, so gpg is required. A mirror
#                       can't get another binary than the release's installed.
#
# What is checked: the binary against SHA256SUMS, always; SHA256SUMS against
# the release key's signature (SHA256SUMS.asc) whenever gpg is installed. The
# release key's fingerprint is pinned below, so a key served alongside the
# signature can't stand in for it. Without gpg, SHA256SUMS is trusted on
# packages.origincloud.eu's HTTPS certificate alone, and the script says so.
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
RELEASE_KEY_FINGERPRINT="FEE6FD74A88C5DB8959F5A1B0BE6AF842E0B034A"
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

# HTTPS only, for every download, a mirror's too: curl is told so, and the
# check here covers wget as well.
download() {
    url="$1"
    dest="$2"
    case "${url}" in
        https://*) ;;
        *) err "refusing ${url}: downloads are over https only" ;;
    esac
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL --proto '=https' --tlsv1.2 "${url}" -o "${dest}" || err "download failed: ${url}"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "${dest}" "${url}" || err "download failed: ${url}"
    else
        err "neither curl nor wget found"
    fi
}

# SHA256SUMS ($1) carries a good signature ($2) by the release key, whose
# public key ($3) is imported into a keyring of its own: the user's is left
# alone. The signer's fingerprint must be the pinned one, so another key
# served in its place verifies nothing.
verify_signature() {
    gnupg_home="${tmp}/gnupg"
    mkdir -m 700 "${gnupg_home}"
    gpg --homedir "${gnupg_home}" --batch --quiet --import "$3" >/dev/null 2>&1 \
        || err "the release key at ${BASE_URL}/origincloud-release.asc can't be imported"
    status="$(gpg --homedir "${gnupg_home}" --batch --status-fd 1 --verify "$2" "$1" 2>/dev/null || true)"
    signer="$(printf '%s\n' "${status}" | awk '$2 == "VALIDSIG" { print $NF; exit }')"
    if ! printf '%s\n' "${status}" | grep -q '^\[GNUPG:\] GOODSIG ' \
       || [ "${signer}" != "${RELEASE_KEY_FINGERPRINT}" ]; then
        err "SHA256SUMS doesn't carry a good signature by the release key ${RELEASE_KEY_FINGERPRINT}. Nothing was installed."
    fi
}

cleanup() {
    if [ -n "${gnupg_home}" ] && command -v gpgconf >/dev/null 2>&1; then
        gpgconf --homedir "${gnupg_home}" --kill all >/dev/null 2>&1 || true
    fi
    if [ -n "${staged}" ]; then rm -f "${staged}"; fi
    if [ -n "${tmp}" ]; then rm -rf "${tmp}"; fi
}

main() {
    platform="$(detect_platform)"
    asset="${BINARY}-${platform}"

    mirror="${ORIGIN_BASE_URL:-}"
    mirror="${mirror%/}"
    install_dir="${ORIGIN_INSTALL_DIR:-${HOME}/.local/bin}"

    have_gpg=false
    if command -v gpg >/dev/null 2>&1; then have_gpg=true; fi
    if [ -n "${mirror}" ] && [ "${have_gpg}" = false ]; then
        err "ORIGIN_BASE_URL is set, and installing from a mirror needs gpg, to check the release key's signature. Install gnupg, or unset ORIGIN_BASE_URL."
    fi

    tmp="" staged="" gnupg_home=""
    # Cleared on every exit path, including the error ones above: a failed
    # install must not leave a half-downloaded binary behind for someone to
    # find and run later. An interrupt exits after cleaning up.
    trap cleanup EXIT
    trap 'cleanup; exit 130' INT TERM
    tmp="$(mktemp -d)"

    version="${ORIGIN_VERSION:-}"
    if [ -z "${version}" ]; then
        download "${BASE_URL}/cli/latest" "${tmp}/latest"
        version="$(tr -d ' \t\r\n' < "${tmp}/latest")"
    fi
    # The version becomes part of a URL and a log line; anything but a
    # version string here means the pointer or the variable is wrong.
    case "${version}" in
        v[0-9]*) ;;
        *) err "'${version}' is not a CLI version (expected e.g. v1.10.0)" ;;
    esac

    log "Installing ${BINARY} ${version} (${platform}) into ${install_dir}"

    download "${BASE_URL}/cli/${version}/SHA256SUMS" "${tmp}/SHA256SUMS"
    if [ "${have_gpg}" = true ]; then
        download "${BASE_URL}/cli/${version}/SHA256SUMS.asc" "${tmp}/SHA256SUMS.asc"
        download "${BASE_URL}/origincloud-release.asc" "${tmp}/release-key.asc"
        verify_signature "${tmp}/SHA256SUMS" "${tmp}/SHA256SUMS.asc" "${tmp}/release-key.asc"
        log "SHA256SUMS is signed by the release key ${RELEASE_KEY_FINGERPRINT}"
    else
        log "gpg not found: SHA256SUMS is trusted on ${BASE_URL}'s HTTPS certificate alone"
    fi

    download "${mirror:-${BASE_URL}}/cli/${version}/${asset}" "${tmp}/${asset}"

    expected="$(grep " \{1,2\}${asset}\$" "${tmp}/SHA256SUMS" | cut -d' ' -f1)"
    [ -n "${expected}" ] || err "${asset} is not listed in SHA256SUMS - refusing to install something the release does not vouch for."

    actual="$(checksum_of "${tmp}/${asset}")"
    if [ "${expected}" != "${actual}" ]; then
        err "checksum mismatch for ${asset}
  expected ${expected}
  actual   ${actual}
The download is corrupt or has been tampered with. Nothing was installed."
    fi

    # Staged in the install directory, then renamed over the old binary: a
    # rename within one filesystem is atomic, so the destination is always
    # either the old binary or the whole verified new one.
    mkdir -p "${install_dir}"
    staged="$(mktemp "${install_dir}/.${BINARY}.XXXXXX")"
    cat "${tmp}/${asset}" > "${staged}"
    chmod 0755 "${staged}"
    mv -f "${staged}" "${install_dir}/${BINARY}"
    staged=""

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
