#!/usr/bin/env bash
#
# Covers release/install.sh.
#
# The installer is the first thing a customer runs, it is piped into a shell,
# and it fetches a binary over the network. The cases worth testing are the
# refusals, not the happy path: an installer that quietly installs something
# it could not verify is worse than no installer at all.
#
# Serves a synthetic release over HTTPS on loopback, with a certificate made
# here (curl trusts it through CURL_CA_BUNDLE), rather than mocking curl, so
# the downloads, the signature check, the SHA256SUMS parsing and the checksum
# comparison are exercised as written. The canonical host is the server's
# root, a mirror its /mirror. The script under test is a copy whose two
# constants, the canonical URL and the release key's fingerprint, point at
# that server and at a key made here; nothing else of it is changed.
#
# Needs python3, openssl, gpg and curl.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT

failures=0
ok()  { printf '  ok   %s\n' "$1"; }
bad() { printf '  FAIL %s\n' "$1"; failures=$((failures + 1)); sed 's/^/         /' "${work}/out" 2>/dev/null || true; }

work="$(mktemp -d)"
serve="${work}/serve"
server_pid=""
cleanup() {
  if [[ -n "${server_pid}" ]]; then kill "${server_pid}" 2>/dev/null || true; fi
  for home in "${work}"/gnupg-*; do gpgconf --homedir "${home}" --kill all >/dev/null 2>&1 || true; done
  rm -rf "${work}"
}
trap cleanup EXIT
mkdir -p "${serve}/mirror"

# --- the server ----------------------------------------------------------------

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 1 \
  -subj /CN=127.0.0.1 -addext subjectAltName=IP:127.0.0.1 \
  -keyout "${work}/tls.key" -out "${work}/tls.crt" >/dev/null 2>&1
python3 - "${serve}" "${work}/tls.crt" "${work}/tls.key" "${work}/port" <<'PY' >/dev/null 2>&1 &
import functools, http.server, os, ssl, sys
directory, cert, key, port_file = sys.argv[1:]
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=directory)
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(cert, key)
server.socket = context.wrap_socket(server.socket, server_side=True)
with open(port_file + ".tmp", "w") as f:
    f.write(str(server.server_address[1]))
os.rename(port_file + ".tmp", port_file)
server.serve_forever()
PY
server_pid=$!
for _ in $(seq 1 100); do [[ -s "${work}/port" ]] && break; sleep 0.1; done
[[ -s "${work}/port" ]] || { echo "::error::the test server didn't start" >&2; exit 1; }
URL="https://127.0.0.1:$(cat "${work}/port")"
readonly URL
export CURL_CA_BUNDLE="${work}/tls.crt"

# --- the keys and the script under test ------------------------------------------

# new_key <name>: a signing key in its own home; prints its fingerprint.
new_key() {
  local home="${work}/gnupg-$1"
  mkdir -m 700 "${home}"
  gpg --homedir "${home}" --batch --quiet --pinentry-mode loopback --passphrase '' \
    --quick-generate-key "Release Test $1 <release-$1@example.com>" ed25519 sign never 2>/dev/null
  gpg --homedir "${home}" --batch --with-colons --list-secret-keys 2>/dev/null | awk -F: '$1 == "fpr" { print $10; exit }'
}
release_fpr="$(new_key release)"
new_key rogue >/dev/null

install_sh="${work}/install.sh"
sed -e "s|^BASE_URL=.*|BASE_URL=\"${URL}\"|" \
    -e "s|^RELEASE_KEY_FINGERPRINT=.*|RELEASE_KEY_FINGERPRINT=\"${release_fpr}\"|" \
    "${ROOT}/release/install.sh" > "${install_sh}"
[[ "$(diff "${ROOT}/release/install.sh" "${install_sh}" | grep -c '^>' || true)" -eq 2 ]] \
  || { echo "::error::the copy of install.sh should differ in its two constants only" >&2; exit 1; }

detect() {
  local os arch
  case "$(uname -s)" in Linux) os=linux ;; Darwin) os=darwin ;; *) printf 'unsupported'; return ;; esac
  case "$(uname -m)" in x86_64|amd64) arch=amd64 ;; aarch64|arm64) arch=arm64 ;; *) printf 'unsupported'; return ;; esac
  printf '%s-%s' "$os" "$arch"
}
asset="origin-$(detect)"
readonly VERSION=v0.0.1
version_dir="${serve}/cli/${VERSION}"
mirror_dir="${serve}/mirror/cli/${VERSION}"

# sign <key>: SHA256SUMS.asc, by that key, over the version's SHA256SUMS;
# the key's public half is served as the release key.
sign() {
  local home="${work}/gnupg-$1"
  gpg --homedir "${home}" --batch --yes --quiet --pinentry-mode loopback --passphrase '' \
    --armor --detach-sign -o "${version_dir}/SHA256SUMS.asc" "${version_dir}/SHA256SUMS"
  gpg --homedir "${home}" --batch --armor --export > "${serve}/origincloud-release.asc"
}

# publish <variant>: the packages.origincloud.eu layout, cli/latest naming the
# version and its files under cli/<version>/, signed by the release key, and
# the same binary on the mirror; then the variant's change to it.
publish() {
  rm -rf "${serve}/cli" "${serve}/mirror"
  mkdir -p "${version_dir}" "${mirror_dir}"
  printf '%s\n' "${VERSION}" > "${serve}/cli/latest"
  printf '#!/bin/sh\necho "origin %s"\n' "${VERSION}" > "${version_dir}/${asset}"
  ( cd "${version_dir}" && sha256sum "${asset}" > SHA256SUMS )
  sign release
  cp "${version_dir}/${asset}" "${mirror_dir}/${asset}"
  case "$1" in
    good) ;;
    tampered) printf 'extra' >> "${version_dir}/${asset}" ;;
    unlisted) : > "${version_dir}/SHA256SUMS"; sign release ;;
    missing) rm -f "${version_dir}/${asset}" ;;
    rogue) sign rogue ;;
    unsigned) rm -f "${version_dir}/SHA256SUMS.asc" ;;
    resigned) printf '%s  other\n' "$(printf x | sha256sum | cut -d' ' -f1)" >> "${version_dir}/SHA256SUMS" ;;
    mirror-only) rm -f "${version_dir}/${asset}" ;;
    mirror-tampered)
      printf '#!/bin/sh\necho evil\n' > "${mirror_dir}/${asset}"
      ( cd "${mirror_dir}" && sha256sum "${asset}" > SHA256SUMS ) ;;
    *) echo "unknown variant $1" >&2; exit 2 ;;
  esac
}

# install_into <dir> [VAR=value...]: runs the installer with this PATH.
install_into() {
  local dir="$1"; shift
  env "$@" ORIGIN_INSTALL_DIR="${dir}" HOME="${work}" sh "${install_sh}" > "${work}/out" 2>&1
}

# stub_path <name> <tool>...: a PATH of these tools only, to take one away.
stub_path() {
  local stub="${work}/stub-$1" tool src; shift
  mkdir -p "${stub}"
  for tool in "$@"; do
    src="$(command -v "${tool}" 2>/dev/null || true)"
    if [[ -n "${src}" ]]; then ln -sf "${src}" "${stub}/${tool}"; fi
  done
  printf '%s' "${stub}"
}
readonly BASE_TOOLS="sh curl uname mktemp cut grep mkdir chmod mv rm tr cat awk"

installed() { [[ -x "$1/origin" ]] && [[ "$("$1/origin")" == "origin ${VERSION}" ]]; }
untouched() { [[ ! -e "$1/origin" ]] && [[ -z "$(find "$1" -name '.origin.*' 2>/dev/null)" ]]; }

# --- the cases -------------------------------------------------------------------

publish good
dir="${work}/ok"
if install_into "${dir}" && installed "${dir}" && grep -q "signed by the release key ${release_fpr}" "${work}/out" \
   && [[ -z "$(find "${dir}" -name '.origin.*')" ]]; then
  ok "a binary whose SHA256SUMS is signed by the release key is installed, with no staging file left"
else
  bad "the happy path should verify the signature and install an executable binary"
fi

printf 'old\n' > "${dir}/origin"
if install_into "${dir}" && installed "${dir}"; then
  ok "an installed binary is replaced"
else
  bad "an existing binary should be replaced by the verified one"
fi

for variant in tampered unlisted missing rogue unsigned resigned; do
  publish "${variant}"
  dir="${work}/${variant}"
  case "${variant}" in
    tampered) what="a binary that doesn't match SHA256SUMS" ;;
    unlisted) what="a binary SHA256SUMS doesn't list" ;;
    missing)  what="a missing binary (rather than installing an error page)" ;;
    rogue)    what="SHA256SUMS signed by another key, served as the release key" ;;
    unsigned) what="SHA256SUMS without its signature, with gpg installed" ;;
    resigned) what="SHA256SUMS changed after it was signed" ;;
  esac
  if ! install_into "${dir}" && untouched "${dir}"; then
    ok "refuses ${what}, and installs nothing"
  else
    bad "must refuse ${what}, and install nothing"
  fi
done

publish mirror-only
dir="${work}/mirror"
if install_into "${dir}" ORIGIN_BASE_URL="${URL}/mirror/" && installed "${dir}"; then
  ok "ORIGIN_BASE_URL serves the binary, checked against the canonical, signed SHA256SUMS"
else
  bad "a mirror holding the release's binary should install it"
fi

publish mirror-tampered
dir="${work}/mirror-tampered"
if ! install_into "${dir}" ORIGIN_BASE_URL="${URL}/mirror" && untouched "${dir}"; then
  ok "refuses a mirror serving another binary with a SHA256SUMS of its own"
else
  bad "a mirror must not be able to install another binary than the release's"
fi

publish good
dir="${work}/mirror-http"
if ! install_into "${dir}" ORIGIN_BASE_URL="http://127.0.0.1:1/mirror" && untouched "${dir}"; then
  ok "refuses a mirror that isn't https"
else
  bad "an http mirror must be refused"
fi

# shellcheck disable=SC2086 # a list of tools on purpose
nogpg="$(stub_path nogpg ${BASE_TOOLS} sha256sum)"
dir="${work}/nogpg"
if env -i PATH="${nogpg}" HOME="${work}" CURL_CA_BUNDLE="${CURL_CA_BUNDLE}" ORIGIN_INSTALL_DIR="${dir}" \
     sh "${install_sh}" > "${work}/out" 2>&1 \
   && installed "${dir}" && grep -q 'gpg not found' "${work}/out"; then
  ok "without gpg, checks the binary against SHA256SUMS over HTTPS, and says the signature wasn't checked"
else
  bad "without gpg, an install from the canonical host should go ahead, saying the signature wasn't checked"
fi

dir="${work}/nogpg-mirror"
if ! env -i PATH="${nogpg}" HOME="${work}" CURL_CA_BUNDLE="${CURL_CA_BUNDLE}" ORIGIN_INSTALL_DIR="${dir}" \
       ORIGIN_BASE_URL="${URL}/mirror" sh "${install_sh}" > "${work}/out" 2>&1 \
   && untouched "${dir}" && grep -q 'needs gpg' "${work}/out"; then
  ok "refuses a mirror without gpg"
else
  bad "ORIGIN_BASE_URL without gpg must be refused"
fi

# Verification must not be skippable: with no sha256sum/shasum on PATH the
# installer refuses. A fallback to "install it anyway" would defeat every
# case above.
# shellcheck disable=SC2086 # a list of tools on purpose
nosum="$(stub_path nosum ${BASE_TOOLS} gpg gpgconf)"
dir="${work}/nochecksum"
if ! env -i PATH="${nosum}" HOME="${work}" CURL_CA_BUNDLE="${CURL_CA_BUNDLE}" ORIGIN_INSTALL_DIR="${dir}" \
       sh "${install_sh}" > "${work}/out" 2>&1 && untouched "${dir}"; then
  ok "no checksum tool means refusal, not an unverified install"
else
  bad "with no checksum tool nothing may be installed"
fi

# Pinning a version skips cli/latest entirely: a pinned install must keep
# working even when the pointer has moved on or is unreadable.
printf 'garbage\n' > "${serve}/cli/latest"
dir="${work}/pinned"
if install_into "${dir}" ORIGIN_VERSION="${VERSION}" && installed "${dir}"; then
  ok "ORIGIN_VERSION installs that version without reading cli/latest"
else
  bad "a pinned version must install regardless of cli/latest"
fi

dir="${work}/badlatest"
if ! install_into "${dir}" && untouched "${dir}"; then
  ok "refuses a cli/latest that is not a version string"
else
  bad "a cli/latest that is not a version must be refused, with nothing installed"
fi

if command -v dash >/dev/null 2>&1; then
  publish good
  dir="${work}/dash"
  if ORIGIN_INSTALL_DIR="${dir}" HOME="${work}" dash "${install_sh}" > "${work}/out" 2>&1 && installed "${dir}"; then
    ok "runs under dash (a minimal container's /bin/sh)"
  else
    bad "must run under dash - a customer's /bin/sh is often not bash"
  fi
else
  ok "dash not present, skipped"
fi

if sh -n "${ROOT}/release/install.sh"; then ok "parses as POSIX sh"; else bad "does not parse as POSIX sh"; fi

if (( failures > 0 )); then
  printf "::error::%d case(s) failed - release/install.sh doesn't behave as documented\n" "${failures}" >&2
  exit 1
fi
printf 'release/install.sh: all cases passed\n'
