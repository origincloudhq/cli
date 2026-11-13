# Releasing

A release publishes `origin` at
[packages.origincloud.eu](https://packages.origincloud.eu): the binaries for
every platform, the `install.sh` that installs them, and the
`origincloud-cli` packages of the apt repository. It is made by the
**Release** workflow (`.github/workflows/release.yml`), from `main`.

## What a release is

The version is `crates/cli/Cargo.toml`'s, which `origin --version` reports,
and the release's tag is that version with a `v`:

- `vX.Y.Z` is a **release**. It becomes `cli/latest`, which `install.sh`
  installs by default, and enters the apt suite `stable`, unless a higher
  version is published already.
- `vX.Y.Z-<suffix>` is a **prerelease**, such as `v1.11.0-rc.1`: it is
  published under `cli/v1.11.0-rc.1/` for whoever asks for it
  (`ORIGIN_VERSION=v1.11.0-rc.1`), and moves neither `cli/latest` nor apt.
  The suffix is dot-separated letters and digits; its packages are versioned
  `1.11.0~rc.1`, which sorts before `1.11.0`.

Anything else is refused before anything is built (`release/version.sh`).

A published version is immutable, on GitHub and at packages.origincloud.eu:
a fix is a new version.

## Making a release

1. **Bump the version** in a pull request, unless `main` is at it already:

   ```sh
   git switch -c release/v1.11.0 origin/main
   sed -i 's/^version = ".*"/version = "1.11.0"/' crates/cli/Cargo.toml   # the [package] version
   cargo update --workspace                                               # its entry in Cargo.lock, nothing else
   git commit -am "Release v1.11.0"
   ```

   Merge it once CI is green. CI builds everything a release publishes, for
   every platform, so a green pull request is a release that builds.

2. **Run the workflow** on `main`, with the version:

   ```sh
   gh workflow run release.yml --repo origincloudhq/cli --ref main -f version=v1.11.0
   ```

   or Actions → Release → Run workflow.

3. **Check** what customers get:

   ```sh
   curl -fsS https://packages.origincloud.eu/cli/latest                  # v1.11.0, for a release
   curl -fsSL https://packages.origincloud.eu/install.sh | sh && origin --version
   ```

## What the workflow does

1. **version**: refuses a run from another branch than `main`, a tag that
   isn't `vX.Y.Z` or `vX.Y.Z-<suffix>`, and a tag that isn't
   `crates/cli/Cargo.toml`'s version.
2. **build** (`build.yml`, which CI runs too): builds `origin` for
   `linux-amd64` and `linux-arm64` (static musl binaries, in a cross image)
   and for `darwin-arm64` and `darwin-amd64` (on a macOS runner), and checks
   that each binary is its platform's, and each Linux one static
   (`release/build.sh`, `release/check-arch.sh`). Then it builds the Debian packages
   (`release/deb.sh`), adds `install.sh`, `LICENSE` and `commands.json`, the
   command tree the linux-amd64 binary prints (`origin __commands`), writes
   `SHA256SUMS` over all of them (`release/assemble.sh`), and installs
   the amd64 package on a clean Debian, where it must report its version.
3. **release**: creates the GitHub release for the tag, at the commit that was
   built, as a draft; attaches the assets; then publishes it, as a prerelease
   for a suffixed version (`release/github-release.sh`). The tag is created
   then, and checked to point at the commit that was built. The notes are
   GitHub's generated ones, unless a draft release for the tag exists: to
   write the notes by hand, create a draft for the tag before running the
   workflow; it is replaced, and its notes become the release's.
4. **publish**: starts origincloudhq/packages' **Publish** workflow for this
   repository and the tag, with a token of the dispatch App, and waits for
   it. That repository holds the signing key and the bucket's credentials,
   checks every asset against `SHA256SUMS`, and publishes; `commands.json`
   stays on the GitHub release.

## When something fails

- **Rerun the failed jobs** (Re-run jobs → Re-run failed jobs), never the
  whole run: the jobs that passed keep their artifacts, so the binaries stay
  the same bytes. A whole new run builds new binaries, which GitHub and
  packages.origincloud.eu refuse under a version they already have.
- **release** resumes: a draft an earlier attempt left is replaced (the log
  says what it deleted), and a release that already holds exactly these
  assets is kept. A failed request to GitHub fails the job, saying which;
  rerun it.
- **publish** resumes: whatever is published already with the same bytes is
  skipped. If it failed on the publisher's side, its run (linked in the log)
  says why.
- A version that was published broken is not replaced: bump to the next
  version and release that.

## One-time setup

The jobs that write, **release** and **publish**, run in the GitHub
environment `release`, which only `main` may use. The **publish** job needs
the dispatch App of origincloudhq/packages, `origincloud-packages-dispatch`:
a GitHub App owned by origincloudhq, installed on that repository only, with
the repository permissions Actions (read and write) and Contents (read). Its
ID is a repository variable here, its private key a secret of the
environment, so that a run from any other branch never gets it, whatever the
workflow file on that branch says. origincloudhq/packages has to list this
repository as the producer of the `cli` channel and of `origincloud-cli`.

Run these once, as a repository admin, in this order: the environment has
to exist, limited to `main`, before the first run, because a job naming an
environment that doesn't exist creates it without any limit.

```sh
# The environment `release`, deployable from main only
gh api -X PUT repos/origincloudhq/cli/environments/release \
  -F 'deployment_branch_policy[protected_branches]=false' \
  -F 'deployment_branch_policy[custom_branch_policies]=true'
gh api -X POST repos/origincloudhq/cli/environments/release/deployment-branch-policies \
  -f name=main -f type=branch

# The dispatch App: its ID, and a private key generated for this repository
# (the App's settings -> Private keys -> Generate a private key)
gh variable set PACKAGES_DISPATCH_APP_ID --repo origincloudhq/cli --body <dispatch app id>
gh secret set PACKAGES_DISPATCH_APP_PRIVATE_KEY --repo origincloudhq/cli --env release \
  < origincloud-packages-dispatch.*.private-key.pem
shred -u origincloud-packages-dispatch.*.private-key.pem

# main: changed by pull request only, after CI, never force-pushed or deleted.
# 15368 is GitHub Actions, so that only its checks count.
gh api -X POST repos/origincloudhq/cli/rulesets --input - <<'JSON'
{
  "name": "main",
  "target": "branch",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "pull_request", "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false } },
    { "type": "required_status_checks", "parameters": {
        "strict_required_status_checks_policy": false,
        "required_status_checks": [
          { "context": "check", "integration_id": 15368 },
          { "context": "build / assets", "integration_id": 15368 } ] } }
  ]
}
JSON

# Optional: release tags can't be moved or deleted. Creating them stays
# allowed, because the release job's token creates each one.
gh api -X POST repos/origincloudhq/cli/rulesets --input - <<'JSON'
{
  "name": "release tags",
  "target": "tag",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/tags/v*"], "exclude": [] } },
  "rules": [ { "type": "update" }, { "type": "deletion" } ]
}
JSON
```

To check it:

```sh
gh api repos/origincloudhq/cli/environments/release --jq '.deployment_branch_policy'
gh api repos/origincloudhq/cli/environments/release/deployment-branch-policies --jq '.branch_policies[].name'
gh secret list --repo origincloudhq/cli --env release
gh variable list --repo origincloudhq/cli
gh api repos/origincloudhq/cli/rulesets --jq '.[].name'
```

The release itself is made with the workflow's own token, which needs no
setup.

## The build images

The Linux binaries are built in `messense/rust-musl-cross`, pinned by digest
in `release/build.sh`; the Rust toolchain in it is still the one
`rust-toolchain.toml` pins. To move the images, pull the tags and put their
digests into the script, in a pull request, whose CI builds with them:

```sh
docker pull messense/rust-musl-cross:x86_64-musl
docker image inspect --format '{{index .RepoDigests 0}}' messense/rust-musl-cross:x86_64-musl
docker pull messense/rust-musl-cross:aarch64-musl
docker image inspect --format '{{index .RepoDigests 0}}' messense/rust-musl-cross:aarch64-musl
```

## Building locally

```sh
release/build.sh linux-amd64 dist          # Docker; linux-arm64 too
release/build.sh darwin-arm64 dist         # on macOS; darwin-amd64 too
for test in release/*.test.sh; do "${test}"; done
```

`release/assemble.sh <binary-dir> <out-dir>` needs all four binaries.
