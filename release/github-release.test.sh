#!/usr/bin/env bash
#
# Covers release/github-release.sh against a fake `gh`, which keeps the
# repository's releases and tags in files and answers the calls the script
# makes, applying the script's own --jq filters with jq. It checks the
# decisions: a release is made as a draft and published only with all its
# assets; a prerelease is marked as one and never the latest; a rerun
# replaces a leftover draft, keeps a release with the same assets and refuses
# one with others; and a tag at another commit is refused.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ROOT
readonly RELEASE="${ROOT}/release/github-release.sh"
readonly COMMIT=1111111111111111111111111111111111111111
readonly OTHER=2222222222222222222222222222222222222222

failures=0
ok()  { printf '  ok   %s\n' "$1"; }
bad() { printf '  FAIL %s\n' "$1"; failures=$((failures + 1)); sed 's/^/         /' "${work}/out" || true; }

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
mkdir -p "${work}/bin"

# The fake gh. State, under $FAKE_GH: releases.json, an array of
# {id, tag_name, draft, prerelease, target, make_latest, assets}; sums/<id>,
# each release's SHA256SUMS; tags/<tag>, the commit a tag points at; calls,
# one line per call.
cat > "${work}/bin/gh" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
state="${FAKE_GH}"
printf '%s\n' "$*" >> "${state}/calls"
releases="${state}/releases.json"
update() { jq "$@" "${releases}" > "${releases}.new" && mv "${releases}.new" "${releases}"; }

if [[ "$1" == api ]]; then
  shift
  method=GET jq_filter="." endpoint="" fields=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --paginate) shift ;;
      -X) method="$2"; shift 2 ;;
      --jq) jq_filter="$2"; shift 2 ;;
      -F|-f) fields+=("$2"); shift 2 ;;
      *) endpoint="$1"; shift ;;
    esac
  done
  endpoint="${endpoint#repos/${GH_REPO}/}"
  case "${method} ${endpoint}" in
    "GET releases?per_page=100") jq -r "${jq_filter}" "${releases}" ;;
    "GET releases/tags/"*)
      tag="${endpoint#releases/tags/}"
      found="$(jq --arg t "${tag}" '[.[] | select(.tag_name == $t and (.draft | not))][0] // empty' "${releases}")"
      [[ -n "${found}" ]] || { echo '{"message":"Not Found"}'; exit 1; }
      jq -r "${jq_filter} | if type == \"string\" then . else tostring end" <<< "${found}" ;;
    "GET releases/"*)
      jq -r --argjson id "${endpoint#releases/}" ".[] | select(.id == \$id) | ${jq_filter}" "${releases}" ;;
    "DELETE releases/"*) update --argjson id "${endpoint#releases/}" 'map(select(.id != $id))' ;;
    "PATCH releases/"*)
      id="${endpoint#releases/}"
      for field in "${fields[@]}"; do
        case "${field}" in
          draft=false) update --argjson id "${id}" 'map(if .id == $id then .draft = false else . end)' ;;
          make_latest=*) update --argjson id "${id}" --arg l "${field#make_latest=}" 'map(if .id == $id then .make_latest = $l else . end)' ;;
        esac
      done ;;
    "GET git/ref/tags/"*)
      tag="${endpoint#git/ref/tags/}"
      [[ -f "${state}/tags/${tag}" ]] || { echo '{"message":"Not Found"}'; exit 1; }
      jq -n --arg sha "$(cat "${state}/tags/${tag}")" '{object: {type: "commit", sha: $sha}}' ;;
    *) echo "fake gh: unexpected api call: ${method} ${endpoint}" >&2; exit 2 ;;
  esac
elif [[ "$1 $2" == "release download" ]]; then
  tag="$3" dir=""
  shift 3
  while [[ $# -gt 0 ]]; do
    case "$1" in --dir) dir="$2"; shift 2 ;; *) shift ;; esac
  done
  id="$(jq -r --arg t "${tag}" '[.[] | select(.tag_name == $t and (.draft | not))][0].id // empty' "${releases}")"
  [[ -n "${id}" && -f "${state}/sums/${id}" ]] || exit 1
  cp "${state}/sums/${id}" "${dir}/SHA256SUMS"
elif [[ "$1 $2" == "release create" ]]; then
  tag="$3" draft=false prerelease=false target="" files=()
  shift 3
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --draft) draft=true; shift ;;
      --prerelease) prerelease=true; shift ;;
      --target) target="$2"; shift 2 ;;
      --title) shift 2 ;;
      --generate-notes) shift ;;
      *) files+=("$1"); shift ;;
    esac
  done
  id=$((RANDOM + 1000))
  names="$(for f in "${files[@]}"; do basename "${f}"; done | jq -R . | jq -sc .)"
  update --argjson id "${id}" --arg t "${tag}" --argjson d "${draft}" --argjson p "${prerelease}" \
    --arg target "${target}" --argjson a "${names}" \
    '. + [{id: $id, tag_name: $t, draft: $d, prerelease: $p, target: $target, make_latest: null, assets: $a}]'
  for f in "${files[@]}"; do
    [[ "$(basename "${f}")" == SHA256SUMS ]] && cp "${f}" "${state}/sums/${id}"
  done
  echo "https://github.com/${GH_REPO}/releases/tag/untagged-${id}"
else
  echo "fake gh: unexpected call: $*" >&2
  exit 2
fi
FAKE
chmod +x "${work}/bin/gh"

mkdir -p "${work}/assets"
for name in origin-linux-amd64 origin-linux-arm64 install.sh LICENSE; do
  printf '%s\n' "${name}" > "${work}/assets/${name}"
done
(cd "${work}/assets" && sha256sum -- * > SHA256SUMS)
readonly ASSET_COUNT=5

# fresh: a repository without releases or tags.
fresh() {
  export FAKE_GH="${work}/state"
  rm -rf "${FAKE_GH}" && mkdir -p "${FAKE_GH}/sums" "${FAKE_GH}/tags"
  echo '[]' > "${FAKE_GH}/releases.json"
  : > "${FAKE_GH}/calls"
}

run() {
  PATH="${work}/bin:${PATH}" GH_REPO=origincloudhq/cli GH_TOKEN=fake \
    "${RELEASE}" "$1" "${COMMIT}" "$2" "${work}/assets" > "${work}/out" 2>&1
}

releases() { jq -c "$1" "${FAKE_GH}/releases.json"; }
created() { grep -c '^release create' "${FAKE_GH}/calls" || true; }

fresh
if run v1.10.0 false \
   && [[ "$(releases 'map(select(.tag_name == "v1.10.0")) | length')" == 1 ]] \
   && [[ "$(releases '.[0] | [.draft, .prerelease, .target, .make_latest, (.assets | length)]')" \
         == "[false,false,\"${COMMIT}\",\"legacy\",${ASSET_COUNT}]" ]] \
   && grep -q '^release create v1.10.0 --draft ' "${FAKE_GH}/calls"; then
  ok "a release is created as a draft at the built commit with every asset, then published"
else
  bad "a release should be created as a draft at ${COMMIT} with ${ASSET_COUNT} assets, then published with make_latest=legacy"
fi

fresh
if run v1.11.0-rc.1 true \
   && [[ "$(releases '.[0] | [.draft, .prerelease, .make_latest]')" == '[false,true,"false"]' ]]; then
  ok "a prerelease is marked as one and never made the latest"
else
  bad "a prerelease should be published with prerelease=true and make_latest=false"
fi

# A rerun after the release was published: same assets, then others.
if run v1.11.0-rc.1 true && [[ "$(created)" == 1 ]] \
   && [[ "$(releases 'length')" == 1 ]] && grep -q 'released already with exactly these assets' "${work}/out"; then
  ok "a release that already holds exactly these assets is kept, and the run succeeds"
else
  bad "a rerun with the same assets should succeed without creating anything"
fi

printf 'other\n' > "${work}/assets/LICENSE"
(cd "${work}/assets" && sha256sum -- install.sh LICENSE origin-linux-amd64 origin-linux-arm64 > SHA256SUMS)
if ! run v1.11.0-rc.1 true && [[ "$(created)" == 1 ]] && grep -q 'with other assets' "${work}/out"; then
  ok "a release that holds other assets is refused, and nothing is created"
else
  bad "a rerun with other assets should fail without creating anything"
fi

fresh
echo '[{"id": 7, "tag_name": "v1.10.0", "draft": true, "prerelease": false, "target": "x", "make_latest": null, "assets": ["origin-linux-amd64"]}]' \
  > "${FAKE_GH}/releases.json"
if run v1.10.0 false && grep -q '^api -X DELETE repos/origincloudhq/cli/releases/7$' "${FAKE_GH}/calls" \
   && [[ "$(releases '[length, .[0].draft, (.[0].assets | length)]')" == "[1,false,${ASSET_COUNT}]" ]]; then
  ok "a draft an earlier run left is deleted and the release made again"
else
  bad "a leftover draft should be deleted, and one complete release published"
fi

fresh
echo "${OTHER}" > "${FAKE_GH}/tags/v1.10.0"
if ! run v1.10.0 false && [[ "$(created)" == 0 ]] && grep -q "points at ${OTHER}" "${work}/out"; then
  ok "a tag that points at another commit is refused, and nothing is created"
else
  bad "a tag at another commit than the built one should be refused before anything is created"
fi

fresh
echo "${COMMIT}" > "${FAKE_GH}/tags/v1.10.0"
if run v1.10.0 false && [[ "$(created)" == 1 ]]; then
  ok "a tag that points at the built commit is released"
else
  bad "a tag at the built commit should be released"
fi

for args in "1.10.0 false" "v1.10.0 maybe"; do
  fresh
  # shellcheck disable=SC2086 # two arguments on purpose
  if ! run ${args} && [[ "$(created)" == 0 ]]; then
    ok "refuses the arguments '${args}'"
  else
    bad "the arguments '${args}' should be refused"
  fi
done

if (( failures > 0 )); then
  printf "::error::%d case(s) failed - release/github-release.sh doesn't behave as documented\n" "${failures}" >&2
  exit 1
fi
printf 'release/github-release.sh: all cases passed\n'
