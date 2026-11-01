#!/usr/bin/env bash
#
# Publishes the GitHub release <tag> at <commit>, with every file of
# <assets-dir> attached, as a prerelease when <prerelease> is true. Run by
# release.yml once every asset is built.
#
#   GH_TOKEN=<token> GH_REPO=<owner/name> \
#     release/github-release.sh <tag> <commit> <prerelease> <assets-dir>
#
# packages.origincloud.eu downloads the release by its tag, which a draft
# doesn't have yet, so the release is made a draft first, gets its assets,
# and is published last: a published release always has all of them.
#
# A rerun resumes rather than adding a second release:
#   - a draft for the tag, left by an interrupted run or written by hand, is
#     deleted and made again; its notes, when it has any, become the new
#     release's, and what was deleted is logged;
#   - a published release for the tag that holds exactly these assets (the
#     same SHA256SUMS) is left as it is, and the run succeeds;
#   - one with other assets fails the run: a published version is immutable.
# The tag must point at <commit>, so that a release never names binaries
# built from another commit than its tag's: an existing tag is checked
# before anything is created, and the tag is checked again once the release
# is published.
#
# Only a 404 means that something doesn't exist; any other failed request
# fails the run, saying so, rather than being taken for an answer.
#
# Needs gh and jq, which GitHub's hosted runners have.
set -euo pipefail

fail() { printf '::error::%s\n' "$*" >&2; exit 1; }
log()  { printf '%s\n' "$*"; }

[[ $# -eq 4 ]] || fail "usage: $0 <tag> <commit> <prerelease> <assets-dir>"
readonly TAG="$1" COMMIT="$2" PRERELEASE="$3" ASSETS="$4"
[[ -n "${GH_REPO:-}" ]] || fail "GH_REPO is unset or empty"
[[ "${TAG}" =~ ^v[0-9A-Za-z.-]+$ ]] || fail "'${TAG}' isn't a release tag"
[[ "${COMMIT}" =~ ^[0-9a-f]{40}$ ]] || fail "'${COMMIT}' isn't a full commit hash"
[[ "${PRERELEASE}" == true || "${PRERELEASE}" == false ]] || fail "<prerelease> is true or false, not '${PRERELEASE}'"
[[ -f "${ASSETS}/SHA256SUMS" ]] || fail "${ASSETS} has no SHA256SUMS"
readonly API="repos/${GH_REPO}"

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

# request <what> <gh api arguments...>: prints the response. Returns 4 for a
# 404; any other failure fails the run, naming <what>. Call it as
# `out="$(request ...)" || status=$?`, so that the 404 can be told apart.
request() {
  local what="$1"; shift
  if gh api "$@" 2>"${work}/gh.err"; then return 0; fi
  if grep -q '(HTTP 404)' "${work}/gh.err"; then return 4; fi
  fail "${what} failed, which a rerun of the job may fix: $(tr '\n' ' ' < "${work}/gh.err")"
}

# The commit the tag points at, or a 404 (status 4) when there is no such
# tag. An annotated tag points at a tag object, which points at the commit.
tag_commit() {
  local ref type sha
  ref="$(request "looking up the tag ${TAG}" "${API}/git/ref/tags/${TAG}")" || return
  type="$(jq -r .object.type <<< "${ref}")"
  sha="$(jq -r .object.sha <<< "${ref}")"
  if [[ "${type}" == tag ]]; then
    sha="$(request "reading the tag object ${sha}" "${API}/git/tags/${sha}" --jq .object.sha)" || return
  fi
  printf '%s' "${sha}"
}

# Every release of the tag, drafts included, as `<id> <draft>` lines. The
# releases/tags endpoint doesn't return drafts.
releases_of_tag() {
  request "listing the releases" --paginate "${API}/releases?per_page=100" \
    --jq ".[] | select(.tag_name == \"${TAG}\") | \"\(.id) \(.draft)\""
}

notes="${work}/notes.md"
existing="$(releases_of_tag)"
while read -r id draft; do
  [[ -n "${id}" ]] || continue
  release="$(request "reading release ${id}" "${API}/releases/${id}")" \
    || fail "release ${id} of ${TAG} disappeared while it was being read: rerun the job"
  if [[ "${draft}" == true ]]; then
    body="$(jq -r '.body // ""' <<< "${release}")"
    log "deleting draft release ${id} of ${TAG}: '$(jq -r '.name // ""' <<< "${release}")', $(jq '.assets | length' <<< "${release}") assets, ${#body} characters of notes"
    if [[ -n "${body}" ]]; then
      printf '%s\n' "${body}" > "${notes}"
      log "its notes become the release's"
    fi
    request "deleting draft release ${id}" -X DELETE "${API}/releases/${id}" >/dev/null
    continue
  fi
  asset="$(jq -r '[.assets[] | select(.name == "SHA256SUMS")][0].id // empty' <<< "${release}")"
  [[ -n "${asset}" ]] || fail "${TAG} is released already, without a SHA256SUMS: release a new version"
  request "downloading the SHA256SUMS of the published ${TAG}" \
    -H 'Accept: application/octet-stream' "${API}/releases/assets/${asset}" > "${work}/SHA256SUMS" \
    || fail "the SHA256SUMS of the published ${TAG} disappeared while it was being downloaded: rerun the job"
  if cmp -s "${work}/SHA256SUMS" "${ASSETS}/SHA256SUMS"; then
    log "${TAG} is released already with exactly these assets"
    exit 0
  fi
  fail "${TAG} is released already, with other assets. A published version is immutable: release a new version. (Rerunning only the failed jobs of a run keeps its binaries; a new run builds new ones.)"
done <<< "${existing}"

status=0
sha="$(tag_commit)" || status=$?
case "${status}" in
  0) [[ "${sha}" == "${COMMIT}" ]] \
       || fail "the tag ${TAG} exists and points at ${sha}, not at ${COMMIT}, which was built: delete the tag or release from its commit" ;;
  4) ;;
  *) exit 1 ;;
esac

flags=(--draft --target "${COMMIT}" --title "${TAG}")
if [[ -s "${notes}" ]]; then flags+=(--notes-file "${notes}"); else flags+=(--generate-notes); fi
[[ "${PRERELEASE}" == true ]] && flags+=(--prerelease)
assets=()
for file in "${ASSETS}"/*; do assets+=("${file}"); done
log "creating draft release ${TAG} at ${COMMIT} with ${#assets[@]} assets"
gh release create "${TAG}" "${flags[@]}" "${assets[@]}" >/dev/null

created="$(releases_of_tag)"
id="$(awk '$2 == "true" { print $1; exit }' <<< "${created}")"
[[ -n "${id}" ]] || fail "the draft release of ${TAG} that was just created can't be found"
count="$(request "reading the draft release ${id}" "${API}/releases/${id}" --jq '.assets | length')"
[[ "${count}" -eq "${#assets[@]}" ]] || fail "the draft release of ${TAG} has ${count} assets, not ${#assets[@]}"

# A prerelease is never the latest. For a release, `legacy` lets GitHub
# weigh the version as well as the date; the API's default, `true`, would
# make a patch of an older line the latest.
if [[ "${PRERELEASE}" == true ]]; then latest=false; else latest=legacy; fi
request "publishing the draft release ${id}" -X PATCH "${API}/releases/${id}" \
  -F draft=false -f make_latest="${latest}" >/dev/null

# Publishing creates the tag at <commit>, unless a tag was pushed meanwhile,
# which GitHub then uses as it is. The release is public by now, so this can
# only shout.
status=0
sha="$(tag_commit)" || status=$?
case "${status}" in
  0) [[ "${sha}" == "${COMMIT}" ]] \
       || fail "${TAG} is published, but its tag points at ${sha}, not at ${COMMIT}, whose binaries it carries: delete the release and the tag, then release again" ;;
  4) fail "${TAG} is published, but its tag doesn't exist" ;;
  *) exit 1 ;;
esac
request "reading the release ${TAG}" "${API}/releases/tags/${TAG}" --jq '"released: \(.html_url) at \(.target_commitish)"'
