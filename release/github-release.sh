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
#   - a draft for the tag, left by an interrupted run, is deleted and made
#     again;
#   - a published release for the tag that holds exactly these assets (the
#     same SHA256SUMS) is left as it is, and the run succeeds;
#   - one with other assets fails the run: a published version is immutable.
# The tag, when it exists already, must point at <commit>, so that a release
# never names binaries built from another commit than its tag's.
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

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT

# Every release of the tag, drafts included, as `<id> <draft>` lines. The
# releases/tags endpoint doesn't return drafts.
releases_of_tag() {
  gh api --paginate "repos/${GH_REPO}/releases?per_page=100" \
    --jq ".[] | select(.tag_name == \"${TAG}\") | \"\(.id) \(.draft)\""
}

existing="$(releases_of_tag)"
while read -r id draft; do
  [[ -n "${id}" ]] || continue
  if [[ "${draft}" == true ]]; then
    log "deleting draft release ${id} of ${TAG}, left by an earlier run"
    gh api -X DELETE "repos/${GH_REPO}/releases/${id}" >/dev/null
    continue
  fi
  gh release download "${TAG}" --pattern SHA256SUMS --dir "${work}" \
    || fail "${TAG} is released already, without a SHA256SUMS: release a new version"
  if cmp -s "${work}/SHA256SUMS" "${ASSETS}/SHA256SUMS"; then
    log "${TAG} is released already with exactly these assets"
    exit 0
  fi
  fail "${TAG} is released already, with other assets. A published version is immutable: release a new version. (Rerunning only the failed jobs of a run keeps its binaries; a new run builds new ones.)"
done <<< "${existing}"

# An existing tag must be the commit that was built. An annotated tag points
# at a tag object, which points at the commit.
if ref="$(gh api "repos/${GH_REPO}/git/ref/tags/${TAG}" 2>/dev/null)"; then
  type="$(jq -r .object.type <<< "${ref}")"
  sha="$(jq -r .object.sha <<< "${ref}")"
  if [[ "${type}" == tag ]]; then
    sha="$(gh api "repos/${GH_REPO}/git/tags/${sha}" --jq .object.sha)"
  fi
  [[ "${sha}" == "${COMMIT}" ]] \
    || fail "the tag ${TAG} exists and points at ${sha}, not at ${COMMIT}, which was built: delete the tag or release from its commit"
fi

flags=(--draft --target "${COMMIT}" --title "${TAG}" --generate-notes)
[[ "${PRERELEASE}" == true ]] && flags+=(--prerelease)
assets=()
for file in "${ASSETS}"/*; do assets+=("${file}"); done
log "creating draft release ${TAG} at ${COMMIT} with ${#assets[@]} assets"
gh release create "${TAG}" "${flags[@]}" "${assets[@]}" >/dev/null

created="$(releases_of_tag)"
id="$(awk '$2 == "true" { print $1; exit }' <<< "${created}")"
[[ -n "${id}" ]] || fail "the draft release of ${TAG} that was just created can't be found"
count="$(gh api "repos/${GH_REPO}/releases/${id}" --jq '.assets | length')"
[[ "${count}" -eq "${#assets[@]}" ]] || fail "the draft release of ${TAG} has ${count} assets, not ${#assets[@]}"

# A prerelease is never the latest. For a release, `legacy` lets GitHub
# weigh the version as well as the date; the API's default, `true`, would
# make a patch of an older line the latest.
if [[ "${PRERELEASE}" == true ]]; then latest=false; else latest=legacy; fi
gh api -X PATCH "repos/${GH_REPO}/releases/${id}" -F draft=false -f make_latest="${latest}" >/dev/null
gh api "repos/${GH_REPO}/releases/tags/${TAG}" --jq '"released: \(.html_url)"'
