# Contributing

Thank you for helping improve the Origin Cloud CLI.

## Issues

Open an issue for anything that is wrong, unclear or missing: a command that
behaves differently from its help, a flag you need, help that doesn't explain
enough. Say which command it is about, and include the command line and its
output where you can, with tokens and account details removed.

To report a security vulnerability, don't open an issue: use **Report a
vulnerability** on this repository's Security tab, which reaches us privately.

## Pull requests

Pull requests are reviewed and merged here. For anything larger than a small
fix, open an issue first, so that we can agree on the change before you spend
time on it. CI must pass: the checks below, run on every pull request.

## How the CLI is built

- `crates/cli` is the `origin` binary. The command groups `admin`, `iam`,
  `compute`, `network`, `storage`, `id` and `config` are written by hand
  (`src/handler/`, `src/service_apis/`) and call the GraphQL API with the
  queries in `graphql/`, checked against `graphql/schema.graphql`, the schema
  `graph.origincloudapis.eu` serves.
- A group per service of the API, such as `origin pvs` and
  `origin location`, is generated (`crates/cli/src/sdk/generated/`) from the
  API definitions in [origincloudhq/api](https://github.com/origincloudhq/api),
  at the commit `API_REVISION` names, by this repository's generator,
  `generator/`, and calls the [Rust SDK](https://github.com/origincloudhq/sdk-rust).
  `crates/cli/src/sdk/runtime.rs` holds what the generated commands share.
- `origin mcp serve` (`src/handler/mcp/`) serves the MCP tools of
  [origincloudhq/mcp](https://github.com/origincloudhq/mcp) on standard
  input and output, with the CLI's profile. The tools, their generator and
  their table are in that repository; the CLI pins it at a commit built on
  the same SDK commit as its own commands, which `scripts/update.sh` keeps
  and `scripts/generate.sh --check` checks.
- `crates/oauth-login` is the browser and device-code login.

The generator downloads the API's dependencies from the Buf Schema Registry at
the commits its `buf.lock` pins, checks them against the digests it pins, and
compiles the definitions with [protox](https://docs.rs/protox). For each
service, the packages `origincloud.{service}.{version}`, it writes
`{service}.rs`, a command per method of the newest version's services: its
flags from the method's request message (its `google.api.field_behavior` and
resource annotations decide which fields are flags, which are required, and
how resource names are put together), its help from the definitions'
comments. The operation methods, `origincloud.longrunning`'s
`GetOperation` and `WaitOperation`, are commands of each service whose methods
answer with long-running operations (`origin pvs get-operation`), because
every service serves the operations its methods return at its own host; the
SDKs bind them to that host the same way. The generator also writes
`services.rs`, which makes each service a group of `origin` with its
`--endpoint` and that option's own variable (`ORIGINCLOUD_PVS_ENDPOINT`),
whose help names the service's host: regional (`pvs.{region}.origincloudapis.eu`
with a region) or not as the Rust SDK's clients say, which
`scripts/generate.sh` hands the generator at the commit `Cargo.lock` pins. It
writes the descriptors of every package it read, `api.binpb`, from
which the tests check that every method has a command, and that every delete
and every one-way change is refused without `--yes` before any request. The
README describes the scheme for users; `generator/src/commands.rs` implements
it.

What the API definitions don't express is in `generator/commands.toml`, by
service, method and request field (`pvs`, `UpdateVolume`, `volume.size_gib`):

- `[[service]]` gives a group's help, for a package with several services;
  a package with one takes it from that service's comment;
- `[[no_command]]` leaves a method without a command, with the reason. The
  coverage test fails for any method of the API that has neither a command
  nor an entry here, which is what keeps the CLI at parity with the API;
- `[[exclude]]` leaves a field out of a method's command, when the method
  doesn't take it although its request message has it, with the reason;
- `[[one_way]]` marks a change that can't be undone, with the refusal's verb
  and consequence: a command setting that field requires `--yes`, like every
  delete. The generated code reaches the SDK's method only through a wrapper
  that takes the client `--yes` makes.

The coverage test fails when an entry names a service, a method or a field
the API doesn't have, so an entry can't silently stop applying; regenerate
after changing the table.

Everything the generator runs is pinned by `Cargo.lock`, so generating from
the same API always gives the same code. The commands use the SDK's request
and response types, so the SDK commit `Cargo.toml` pins must have been
generated from the same API revision; the scripts check that.

So a change to what a generated command offers starts in the API, and
reaches the CLI through the SDK; a change to how every generated command
behaves is a change to the generator or the runtime. Never edit the
generated files.

The regeneration workflow (`.github/workflows/regenerate.yml`) moves the CLI
to the newest SDK, and the API revision it was generated from, daily, and opens
a pull request when that changes anything. If the API gained a method the
generator can't make a command for, the generator says so on standard error
and the coverage test fails: extend the generator on that pull request, or
list the method under `[[no_command]]` with the reason.

### A new service

When the API gains a service, regeneration writes its group, and the
regeneration pull request fails to build until the group is wired up, on that
pull request:

1. Add the service's SDK crate, `origincloud-{service}` from
   `origincloudhq/sdk-rust` at the same `rev` as the others, to
   `[workspace.dependencies]` in `Cargo.toml`, and
   `origincloud-{service}.workspace = true` to `crates/cli/Cargo.toml`, then
   run `cargo update --workspace`, which adds it to `Cargo.lock` and moves
   nothing else. `scripts/update.sh` keeps it at the SDK's head from then on.
2. If the service's package has several services, give the group's help a
   `[[service]]` entry in `generator/commands.toml`, and regenerate.
3. If a hand-written group has the service's name (such as `compute`), the
   two collide and the command tree test fails: retire the hand-written group
   in the same pull request, or rename it.
4. Mention the group in the README.

Nothing else names a service: the scripts, the generator and the tests find
every service in the API. A file under `origincloud/` whose package isn't
shaped `origincloud.{service}.{version}` fails generation, rather than being
left out unnoticed.

## Checks

Rust (the version `rust-toolchain.toml` names), git and network access are all
you need.

```sh
scripts/generate.sh            # generate the commands of every service again
scripts/generate.sh --check    # check the checked-in commands are what the API generates
scripts/update.sh              # move to the newest SDK and its API revision, and generate
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI also checks the scripts with shellcheck, runs the release scripts' tests
(`release/*.test.sh`), and builds everything a release publishes, for every
platform, without publishing it.

## Releases

Releases are made from `main` by the Release workflow; [RELEASING.md](RELEASING.md)
says how, and what `release/` holds.

## License

By contributing, you agree that your contribution is licensed under the
Apache License 2.0, like the rest of this repository.
