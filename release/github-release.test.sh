#!/usr/bin/env bash
#
# Covers release/github-release.sh against a fake `gh`, which keeps the
# repository's releases and tags in files and answers the calls the script
# makes, applying the script's own --jq filters with jq. It checks the
# decisions: a release is made as a draft and published only with all its
# assets; a prerelease is marked as one and never the latest; a rerun
# replaces a leftover draft (keeping its notes), keeps a release with the
# same assets and refuses one with others; a tag at another commit is
# refused, before and after publishing; and a failed request is never taken
# for an answer.
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
# {id, tag_name, name, body, draft, prerelease, target_commitish,
# make_latest, html_url, assets: [{id, name}]}; assets/<id>, each asset's
# content; tags/<tag>, the commit a tag points at; calls, one line per call.
#   FAKE_GH_FAIL     a regular expression: a call matching it fails with 502
#   FAKE_GH_TAG_AT   the commit publishing creates a missing tag at, instead
#                    of the release's target
cat > "${work}/bin/gh" <<'FAKE'
#!/usr/bin/env bash
set -euo pipefail
state="${FAKE_GH}"
printf '%s\n' "$*" >> "${state}/calls"
releases="${state}/releases.json"
update() { jq "$@" "${releases}" > "${releases}.new" && mv "${releases}.new" "${releases}"; }
not_found() { echo '{"message":"Not Found"}'; echo 'gh: Not Found (HTTP 404)' >&2; exit 1; }
if [[ -n "${FAKE_GH_FAIL:-}" && "$*" =~ ${FAKE_GH_FAIL} ]]; then
  echo 'gh: Server Error (HTTP 502)' >&2
  exit 1
fi

if [[ "$1" == api ]]; then
  shift
  method=GET jq_filter="." endpoint="" fields=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --paginate) shift ;;
      -X) method="$2"; shift 2 ;;
      -H) shift 2 ;;
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
      [[ -n "${found}" ]] || not_found
      jq -r "${jq_filter}" <<< "${found}" ;;
    "GET releases/assets/"*)
      [[ -f "${state}/assets/${endpoint#releases/assets/}" ]] || not_found
      cat "${state}/assets/${endpoint#releases/assets/}" ;;
    "GET releases/"*)
      found="$(jq --argjson id "${endpoint#releases/}" '.[] | select(.id == $id)' "${releases}")"
      [[ -n "${found}" ]] || not_found
      jq -r "${jq_filter}" <<< "${found}" ;;
    "DELETE releases/"*) update --argjson id "${endpoint#releases/}" 'map(select(.id != $id))' ;;
    "PATCH releases/"*)
      id="${endpoint#releases/}"
      for field in "${fields[@]}"; do
        case "${field}" in
          draft=false)
            update --argjson id "${id}" 'map(if .id == $id then .draft = false else . end)'
            tag="$(jq -r --argjson id "${id}" '.[] | select(.id == $id) | .tag_name' "${releases}")"
            target="$(jq -r --argjson id "${id}" '.[] | select(.id == $id) | .target_commitish' "${releases}")"
            [[ -f "${state}/tags/${tag}" ]] || echo "${FAKE_GH_TAG_AT:-${target}}" > "${state}/tags/${tag}" ;;
          make_latest=*) update --argjson id "${id}" --arg l "${field#make_latest=}" 'map(if .id == $id then .make_latest = $l else . end)' ;;
        esac
      done ;;
    "GET git/ref/tags/"*)
      tag="${endpoint#git/ref/tags/}"
      [[ -f "${state}/tags/${tag}" ]] || not_found
      jq -n --arg sha "$(cat "${state}/tags/${tag}")" '{object: {type: "commit", sha: $sha}}' | jq -r "${jq_filter}" ;;
    *) echo "fake gh: unexpected api call: ${method} ${endpoint}" >&2; exit 2 ;;
  esac
elif [[ "$1 $2" == "release create" ]]; then
  tag="$3" draft=false prerelease=false target="" body="" files=()
  shift 3
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --draft) draft=true; shift ;;
      --prerelease) prerelease=true; shift ;;
      --target) target="$2"; shift 2 ;;
      --title) shift 2 ;;
      --generate-notes) body="generated notes"; shift ;;
      --notes-file) body="$(cat "$2")"; shift 2 ;;
      *) files+=("$1"); shift ;;
    esac
  done
  id=$((RANDOM + 1000)) n=0 assets='[]'
  for f in "${files[@]}"; do
    n=$((n + 1))
    cp "${f}" "${state}/assets/${id}${n}"
    assets="$(jq -c --argjson a "${id}${n}" --arg n "$(basename "${f}")" '. + [{id: $a, name: $n}]' <<< "${assets}")"
  done
  update --argjson id "${id}" --arg t "${tag}" --argjson d "${draft}" --argjson p "${prerelease}" \
    --arg target "${target}" --arg body "${body}" --argjson a "${assets}" \
    '. + [{id: $id, tag_name: $t, name: $t, body: $body, draft: $d, prerelease: $p, target_commitish: $target,
           make_latest: null, html_url: ("https://example.com/" + $t), assets: $a}]'
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

# fresh: a repository without releases or tags, and no injected failures.
fresh() {
  export FAKE_GH="${work}/state"
  rm -rf "${FAKE_GH}" && mkdir -p "${FAKE_GH}/assets" "${FAKE_GH}/tags"
  echo '[]' > "${FAKE_GH}/releases.json"
  : > "${FAKE_GH}/calls"
  unset FAKE_GH_FAIL FAKE_GH_TAG_AT
}

run() {
  PATH="${work}/bin:${PATH}" GH_REPO=origincloudhq/cli GH_TOKEN=fake \
    "${RELEASE}" "$1" "${COMMIT}" "$2" "${work}/assets" > "${work}/out" 2>&1
}

releases() { jq -c "$1" "${FAKE_GH}/releases.json"; }
created() { grep -c '^release create' "${FAKE_GH}/calls" || true; }

# --- releases ------------------------------------------------------------------

fresh
if run v1.10.0 false \
   && [[ "$(releases 'map(select(.tag_name == "v1.10.0")) | length')" == 1 ]] \
   && [[ "$(releases '.[0] | [.draft, .prerelease, .target_commitish, .make_latest, (.assets | length), .body]')" \
         == "[false,false,\"${COMMIT}\",\"legacy\",${ASSET_COUNT},\"generated notes\"]" ]] \
   && grep -q '^release create v1.10.0 --draft ' "${FAKE_GH}/calls" \
   && [[ "$(cat "${FAKE_GH}/tags/v1.10.0")" == "${COMMIT}" ]]; then
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

# --- reruns --------------------------------------------------------------------

if run v1.11.0-rc.1 true && [[ "$(created)" == 1 ]] \
   && [[ "$(releases 'length')" == 1 ]] && grep -q 'released already with exactly these assets' "${work}/out"; then
  ok "a release that already holds exactly these assets is kept, and the run succeeds"
else
  bad "a rerun with the same assets should succeed without creating anything"
fi

export FAKE_GH_FAIL='releases/assets/'
if ! run v1.11.0-rc.1 true && [[ "$(created)" == 1 ]] \
   && grep -q 'a rerun of the job may fix' "${work}/out" && ! grep -q 'without a SHA256SUMS' "${work}/out"; then
  ok "a failed download of the published SHA256SUMS fails as a failed request, not as a missing SHA256SUMS"
else
  bad "a 502 downloading the published SHA256SUMS must fail as a request to retry"
fi
unset FAKE_GH_FAIL

cp "${work}/assets/SHA256SUMS" "${work}/SHA256SUMS.orig"
printf 'other\n' > "${work}/assets/LICENSE"
(cd "${work}/assets" && sha256sum -- install.sh LICENSE origin-linux-amd64 origin-linux-arm64 > SHA256SUMS)
if ! run v1.11.0-rc.1 true && [[ "$(created)" == 1 ]] && grep -q 'with other assets' "${work}/out"; then
  ok "a release that holds other assets is refused, and nothing is created"
else
  bad "a rerun with other assets should fail without creating anything"
fi
printf 'LICENSE\n' > "${work}/assets/LICENSE"
cp "${work}/SHA256SUMS.orig" "${work}/assets/SHA256SUMS"

update_state() { jq "$1" "${FAKE_GH}/releases.json" > "${FAKE_GH}/r.new" && mv "${FAKE_GH}/r.new" "${FAKE_GH}/releases.json"; }
update_state 'map(.assets |= map(select(.name != "SHA256SUMS")))'
if ! run v1.11.0-rc.1 true && [[ "$(created)" == 1 ]] && grep -q 'without a SHA256SUMS' "${work}/out"; then
  ok "a published release without SHA256SUMS is refused as such"
else
  bad "a published release without SHA256SUMS should be refused, saying so"
fi

fresh
echo '[{"id": 7, "tag_name": "v1.10.0", "name": "v1.10.0", "body": "", "draft": true, "prerelease": false,
        "target_commitish": "x", "make_latest": null, "html_url": "", "assets": [{"id": 71, "name": "origin-linux-amd64"}]}]' \
  > "${FAKE_GH}/releases.json"
if run v1.10.0 false && grep -q '^api -X DELETE repos/origincloudhq/cli/releases/7$' "${FAKE_GH}/calls" \
   && grep -q 'deleting draft release 7 of v1.10.0' "${work}/out" \
   && [[ "$(releases '[length, .[0].draft, (.[0].assets | length), .[0].body]')" == "[1,false,${ASSET_COUNT},\"generated notes\"]" ]]; then
  ok "a draft an earlier run left is deleted, logged, and the release made again"
else
  bad "a leftover draft should be deleted and logged, and one complete release published"
fi

fresh
jq -n '[{id: 8, tag_name: "v1.10.0", name: "Hand-written", body: "## Highlights\n\nWritten by hand.", draft: true,
         prerelease: false, target_commitish: "main", make_latest: null, html_url: "", assets: []}]' > "${FAKE_GH}/releases.json"
if run v1.10.0 false && grep -q "deleting draft release 8 of v1.10.0: 'Hand-written', 0 assets, 31 characters of notes" "${work}/out" \
   && [[ "$(releases '.[0].body')" == '"## Highlights\n\nWritten by hand."' ]] \
   && ! grep -q -- '--generate-notes' "${FAKE_GH}/calls"; then
  ok "a draft's hand-written notes become the release's, and the deletion is logged"
else
  bad "the notes of a deleted draft should carry over to the new release"
fi

# --- the tag -------------------------------------------------------------------

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

fresh
export FAKE_GH_FAIL='git/ref/tags/'
if ! run v1.10.0 false && [[ "$(created)" == 0 ]] && grep -q 'looking up the tag v1.10.0 failed' "${work}/out"; then
  ok "a failed tag lookup fails the run, rather than passing for no tag"
else
  bad "a 502 looking up the tag must fail the run before anything is created"
fi

fresh
export FAKE_GH_FAIL='per_page'
if ! run v1.10.0 false && [[ "$(created)" == 0 ]] && grep -q 'listing the releases failed' "${work}/out"; then
  ok "a failed release listing fails the run"
else
  bad "a 502 listing the releases must fail the run before anything is created"
fi

fresh
export FAKE_GH_TAG_AT="${OTHER}"
if ! run v1.10.0 false && grep -q "is published, but its tag points at ${OTHER}" "${work}/out"; then
  ok "a published release whose tag ended up at another commit fails the run loudly"
else
  bad "the tag must be checked again after publishing"
fi

# --- arguments -----------------------------------------------------------------

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
