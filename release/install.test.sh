#!/usr/bin/env bash
#
# Covers release/install.sh.
#
# The installer is the first thing a customer runs, it is piped into a shell,
# and it fetches a binary over the network. The cases worth testing are the
# refusals, not the happy path: an installer that quietly installs something
# it could not verify is worse than no installer at all.
#
# Serves a synthetic release over loopback rather than mocking curl, so the
# download path, the SHA256SUMS parsing and the checksum comparison are all
# exercised as written.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly INSTALL_SH="${ROOT}/release/install.sh"
readonly PORT="${INSTALL_TEST_PORT:-8731}"

failures=0
ok()  { printf '  ok   %s\n' "$1"; }
bad() { printf '  FAIL %s\n' "$1"; failures=$((failures + 1)); }

serve_dir="$(mktemp -d)"
work="$(mktemp -d)"
server_pid=""
cleanup() {
  if [[ -n "${server_pid}" ]]; then kill "${server_pid}" 2>/dev/null || true; fi
  rm -rf "${serve_dir}" "${work}"
}
trap cleanup EXIT

printf '#!/bin/sh\necho stub\n' > "${serve_dir}/payload"
chmod +x "${serve_dir}/payload"

detect() {
  local os arch
  case "$(uname -s)" in Linux) os=linux ;; Darwin) os=darwin ;; *) printf 'unsupported'; return ;; esac
  case "$(uname -m)" in x86_64|amd64) arch=amd64 ;; aarch64|arm64) arch=arm64 ;; *) printf 'unsupported'; return ;; esac
  printf '%s-%s' "$os" "$arch"
}
asset="origin-$(detect)"

# The packages.origincloud.eu layout: cli/latest names the version, and the
# version's files live under cli/<version>/.
readonly VERSION=v0.0.1
version_dir="${serve_dir}/cli/${VERSION}"

publish() {
  mkdir -p "${version_dir}"
  printf '%s\n' "${VERSION}" > "${serve_dir}/cli/latest"
  cp "${serve_dir}/payload" "${version_dir}/${asset}"
  ( cd "${version_dir}" && sha256sum "${asset}" > SHA256SUMS )
  case "${1}" in
    tampered) printf 'extra' >> "${version_dir}/${asset}" ;;
    unlisted) : > "${version_dir}/SHA256SUMS" ;;
  esac
}

python3 -m http.server "${PORT}" --directory "${serve_dir}" >/dev/null 2>&1 &
server_pid=$!
for _ in $(seq 1 40); do
  curl -fsS "http://127.0.0.1:${PORT}/" >/dev/null 2>&1 && break
  sleep 0.25
done

run_install() {
  ORIGIN_BASE_URL="http://127.0.0.1:${PORT}" ORIGIN_INSTALL_DIR="${1}" \
    sh "${INSTALL_SH}" >/dev/null 2>&1
}

publish good
dir="${work}/ok"
if run_install "${dir}" && [[ -x "${dir}/origin" ]]; then
  ok "a verified binary is installed and executable"
else
  bad "the happy path should install an executable binary"
fi

publish tampered
dir="${work}/tampered"
if run_install "${dir}"; then
  bad "a checksum mismatch must fail"
elif [[ -e "${dir}/origin" ]]; then
  bad "a checksum mismatch must install NOTHING - a partial install is the dangerous outcome"
else
  ok "a tampered binary is refused and nothing is installed"
fi

publish unlisted
dir="${work}/unlisted"
if run_install "${dir}"; then
  bad "an asset absent from SHA256SUMS must fail"
elif [[ -e "${dir}/origin" ]]; then
  bad "an unlisted asset must install nothing"
else
  ok "an asset missing from SHA256SUMS is refused"
fi

rm -f "${version_dir}/${asset}"
dir="${work}/missing"
if run_install "${dir}"; then
  bad "a 404 on the binary must fail"
else
  ok "a missing asset fails rather than installing an error page"
fi

# Verification must not be skippable: with no sha256sum/shasum on PATH the
# installer refuses. A fallback to "install it anyway" would defeat every
# case above.
publish good
dir="${work}/nochecksum"
stub_path="${work}/stubbin"
mkdir -p "${stub_path}"
for tool in curl uname mktemp cut grep mkdir chmod mv rm sed tr; do
  src="$(command -v "${tool}" 2>/dev/null || true)"
  if [[ -n "${src}" ]]; then ln -sf "${src}" "${stub_path}/${tool}"; fi
done
if env -i PATH="${stub_path}" HOME="${work}" \
     ORIGIN_BASE_URL="http://127.0.0.1:${PORT}" ORIGIN_INSTALL_DIR="${dir}" \
     /bin/sh "${INSTALL_SH}" >/dev/null 2>&1; then
  bad "with no checksum tool the installer must refuse"
elif [[ -e "${dir}/origin" ]]; then
  bad "with no checksum tool nothing may be installed"
else
  ok "no checksum tool means refusal, not an unverified install"
fi

# Pinning a version skips cli/latest entirely: a pinned install must keep
# working even when the pointer has moved on or is unreadable.
publish good
printf 'garbage\n' > "${serve_dir}/cli/latest"
dir="${work}/pinned"
if ORIGIN_VERSION="${VERSION}" run_install "${dir}" && [[ -x "${dir}/origin" ]]; then
  ok "ORIGIN_VERSION installs that version without reading cli/latest"
else
  bad "a pinned version must install regardless of cli/latest"
fi

dir="${work}/badlatest"
if run_install "${dir}"; then
  bad "a cli/latest that is not a version must be refused"
elif [[ -e "${dir}/origin" ]]; then
  bad "an unreadable cli/latest must install nothing"
else
  ok "a cli/latest that is not a version string is refused"
fi

if command -v dash >/dev/null 2>&1; then
  publish good
  dir="${work}/dash"
  if ORIGIN_BASE_URL="http://127.0.0.1:${PORT}" ORIGIN_INSTALL_DIR="${dir}" \
       dash "${INSTALL_SH}" >/dev/null 2>&1 && [[ -x "${dir}/origin" ]]; then
    ok "runs under dash (a minimal container's /bin/sh)"
  else
    bad "must run under dash - a customer's /bin/sh is often not bash"
  fi
else
  ok "dash not present, skipped"
fi

if sh -n "${INSTALL_SH}"; then ok "parses as POSIX sh"; else bad "does not parse as POSIX sh"; fi

if (( failures > 0 )); then
  printf "::error::%d case(s) failed - release/install.sh doesn't behave as documented\n" "${failures}" >&2
  exit 1
fi
printf 'release/install.sh: all cases passed\n'
