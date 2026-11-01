#!/usr/bin/env bash
#
# Builds `origin` for one published platform and puts it into <out-dir> under
# its published name, origin-<platform>, after checking that it is that
# platform's binary (release/check-arch.sh).
#
#   release/build.sh <platform> <out-dir>
#
# <platform> is linux-amd64, linux-arm64, darwin-arm64 or darwin-amd64: the
# os-arch spelling install.sh derives from `uname -s` and `uname -m`.
#
# Linux builds are static musl binaries, so that one file runs on any
# distribution. They build in a musl cross image, which has the C cross
# toolchain and the cmake that aws-lc-sys (rustls' crypto provider) needs for
# the musl targets; that only needs Docker, on any Linux x86_64 host. The
# image is pinned by digest: it compiles what customers run.
#
# macOS builds are native, on a macOS host with Xcode's command-line tools,
# which build both architectures; there is no usable cross path to Darwin
# from Linux.
#
# The Rust toolchain is the one rust-toolchain.toml pins, in the image too,
# and Cargo.lock is used as it is (--locked).
#
# Environment:
#   CARGO_TARGET_DIR   optional; default target/ at the root. The Linux build
#                      uses <it>/musl, so that its build scripts, compiled in
#                      the image, never mix with the host's.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT

fail() { printf '::error::%s\n' "$*" >&2; exit 1; }

[[ $# -eq 2 ]] || fail "usage: $0 <platform> <out-dir>"
readonly PLATFORM="$1" OUT_DIR="$2"

# messense/rust-musl-cross, the x86_64-musl and aarch64-musl tags as of
# 2026-11-01. To move them: docker pull the tag, then
# docker image inspect --format '{{index .RepoDigests 0}}' <tag>.
readonly IMAGE_AMD64="messense/rust-musl-cross@sha256:3f1010eb48311dfde5d51f011163451940af34bd317355c4a8652ce6b4b61051"
readonly IMAGE_ARM64="messense/rust-musl-cross@sha256:02e790506c5f15a533bc05b3af8719b16a8a2314480f6f0fdc0166432035d9b4"

case "${PLATFORM}" in
  linux-amd64)  triple=x86_64-unknown-linux-musl  image="${IMAGE_AMD64}" ;;
  linux-arm64)  triple=aarch64-unknown-linux-musl image="${IMAGE_ARM64}" ;;
  darwin-arm64) triple=aarch64-apple-darwin       image="" ;;
  darwin-amd64) triple=x86_64-apple-darwin        image="" ;;
  *) fail "unknown platform '${PLATFORM}'; one of linux-amd64 linux-arm64 darwin-arm64 darwin-amd64" ;;
esac

target_dir="${CARGO_TARGET_DIR:-${ROOT}/target}"
mkdir -p "${target_dir}" "${OUT_DIR}"
target_dir="$(cd "${target_dir}" && pwd)"

if [[ -n "${image}" ]]; then
  [[ "$(uname -s)" == Linux ]] || fail "${PLATFORM} builds on Linux, with Docker"
  # The image runs as root, so what it writes is handed back to the caller
  # before it exits, failed or not: a root-owned target directory would break
  # the next build, and the next checkout on a reused runner.
  docker run --rm \
    -v "${ROOT}:/src:ro" \
    -v "${target_dir}:/target" \
    -w /src \
    -e CARGO_TARGET_DIR=/target/musl \
    -e CARGO_TERM_COLOR \
    -e CARGO_BUILD_JOBS \
    -e TRIPLE="${triple}" \
    -e OWNER="$(id -u):$(id -g)" \
    "${image}" \
    bash -c 'status=0
      { rustup toolchain install \
          && rustup target add "${TRIPLE}" \
          && cargo build --locked --release --target "${TRIPLE}" -p origincloud-cli --bin origin; } || status=$?
      chown -R "${OWNER}" /target/musl
      exit "${status}"'
  binary="${target_dir}/musl/${triple}/release/origin"
else
  [[ "$(uname -s)" == Darwin ]] || fail "${PLATFORM} builds on macOS"
  # From the root, so that rustup installs the toolchain rust-toolchain.toml
  # pins and adds the target to it rather than to the default one.
  cd "${ROOT}"
  rustup toolchain install
  rustup target add "${triple}"
  CARGO_TARGET_DIR="${target_dir}" cargo build --locked --release --target "${triple}" -p origincloud-cli --bin origin
  binary="${target_dir}/${triple}/release/origin"
fi

[[ -f "${binary}" ]] || fail "the build succeeded, but ${binary} doesn't exist"
install -m 0755 "${binary}" "${OUT_DIR}/origin-${PLATFORM}"
"${ROOT}/release/check-arch.sh" "${PLATFORM}" "${OUT_DIR}/origin-${PLATFORM}"
