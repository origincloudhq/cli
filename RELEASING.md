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
   that each binary is its platform's (`release/build.sh`,
   `release/check-arch.sh`). Then it builds the Debian packages
   (`release/deb.sh`), adds `install.sh` and `LICENSE`, writes `SHA256SUMS`
   over all of them (`release/assemble.sh`), and installs the amd64 package on
   a clean Debian, where it must report its version.
3. **release**: creates the GitHub release for the tag, at the commit that was
   built, as a draft; attaches the assets; then publishes it, as a prerelease
   for a suffixed version (`release/github-release.sh`). The tag is created
   then.
4. **publish**: starts origincloudhq/packages' **Publish** workflow for this
   repository and the tag, with a token of the dispatch App, and waits for
   it. That repository holds the signing key and the bucket's credentials,
   checks every asset against `SHA256SUMS`, and publishes.

## When something fails

- **Rerun the failed jobs** (Re-run jobs → Re-run failed jobs), never the
  whole run: the jobs that passed keep their artifacts, so the binaries stay
  the same bytes. A whole new run builds new binaries, which GitHub and
  packages.origincloud.eu refuse under a version they already have.
- **release** resumes: a draft an earlier attempt left is replaced, and a
  release that already holds exactly these assets is kept.
- **publish** resumes: whatever is published already with the same bytes is
  skipped. If it failed on the publisher's side, its run (linked in the log)
  says why.
- A version that was published broken is not replaced: bump to the next
  version and release that.

## One-time setup

The **publish** job needs the dispatch App of origincloudhq/packages, whose
ID and private key are set in this repository, and this repository has to be
the producer of the `cli` channel in that repository's `producers.conf`. Its
[`docs/github-apps.md`](https://github.com/origincloudhq/packages/blob/main/docs/github-apps.md)
describes the Apps; with the App's private key downloaded as a `.pem` file:

```sh
gh variable set PACKAGES_DISPATCH_APP_ID --repo origincloudhq/cli --body <dispatch app id>
gh secret set PACKAGES_DISPATCH_APP_PRIVATE_KEY --repo origincloudhq/cli < origincloud-packages-dispatch.*.private-key.pem
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
