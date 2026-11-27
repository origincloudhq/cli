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

- `crates/cli` is the `origin` binary. The command groups `id`, `config`,
  `mcp` and `completion` are written by hand (`src/handler/`). `id login`
  calls IAM's `ActivatePendingRoleBindings` once after a user login, as
  the person who signed in, which grants the roles they were invited to
  (`src/handler/id/login.rs`).
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
  `origin mcp install` and `uninstall` add that server to the coding
  agents of the table in `src/handler/agents.rs`, and remove it, with
  the agent's own command or by editing its file (`config_file.rs`): an
  agent is a row of the table, plus a format adapter when it brings a
  format of its own. The commands' help, the README's table of agents (a
  test checks it is the table's) and the tests read it. The edits are
  tested against golden files in `crates/cli/tests/agent-config/`;
  `UPDATE_GOLDEN=1 cargo test` writes those and the README's tables anew,
  for review in the diff.
- `origin skill install`, `list` and `update` (`src/handler/skill/`)
  install the agent skills of
  [origincloudhq/skills](https://github.com/origincloudhq/skills) into the
  skill directories of the same table's agents (its `skills` field). The
  skills are those of the commit `SKILLS_REVISION` names: `scripts/generate.sh`
  copies them, without their `evals/`, into `crates/cli/skills/`, which
  `crates/cli/build.rs` embeds, and `--check` checks the copy;
  `scripts/update.sh` moves the pin to the head of origincloudhq/skills'
  main. Never edit the copy: a change to a skill is a change in
  origincloudhq/skills. An installed copy's `SKILL.md` carries its version
  and digest in its `metadata` (`src/handler/skill/marker.rs`), which tell
  `origin`'s copies from anyone else's.
- `origin __commands` (`src/handler/command_tree.rs`) prints the command
  tree as JSON, which each release attaches as `commands.json` for tools
  that name the CLI's commands and flags (README). It isn't a command of
  the tree, so neither the help nor the completion offers it. A generated
  command's method is found in `api.binpb` by the same naming the coverage
  test checks; `crates/cli/tests/command_tree.rs` fixes its fields, which
  tools outside this repository read: add fields, never rename or remove
  one.
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

A resource name is checked as the SDK checks the names it sends
(`origincloud_core::path_value`), never against an alphabet of the CLI's:
each id is one segment, not empty, `.` or `..`, and without `/`, and
anything else, such as the dots of `gp1.core.2`, is the API's to judge.

Every flag naming a resource takes its full name (AIP-122),
`//{service}.origincloudapis.eu/{relative}`, parsed with the SDK's
`origincloud_core::names::FullName`, whose host must be the service of the
type the field refers to (the domain of its `google.api.resource_reference`
type, `pvs` for `pvs.origincloudapis.eu/Volume`), and its relative name
too. The command's own target (`name`, or the resource of an update) and
its parents' flags (`--instance` of a volume attachment, any part of the
pattern but `--project`, `--zone`, `--region` and `--organization`) also
take ids, completed from those flags and the profile (`runtime.rs`'s
`Names`); a whole name given to one fills in the other parts, and the
request's path is the relative name. Of those four, a part that is a
resource of its own at the root of the pattern (`projects/{project}`,
`organizations/{organization}`) takes that resource's whole name too, on
its own service's host (`//resourcemanager.origincloudapis.eu/…`); the
others, such as the zone of `projects/{project}/zones/{zone}`, are ids. A reference to another resource, in a
flag, a key or JSON at any depth, is never made of parts (`runtime.rs`'s
`reference`, `nested.rs`'s `full_names`): it is a full name, sent as
given, as the API requires of a reference, and a relative name or an id is
refused, before any request, with the full name's form.
A reference whose resource declares several patterns
(`google.api.resource`'s `pattern` list) takes a full name of each, its
help naming them all.

The tests' scratch service isn't one of the platform's, so its full names
aren't (`FullName` knows the platform's services only): its tests give
full names in the same form, `//scratch.origincloudapis.eu/…`, which, with
the `test-services` feature alone, are accepted, and the refusal of a
relative name is tested through the published services.

A duration or a timestamp is a flag of its own form (`30s`, RFC 3339). A
`bytes` field is `--{field}-file`, taking a path or `-` for standard input,
whose content is sent byte for byte (`runtime.rs`'s `read_bytes_file`),
wherever a field is a flag: in a request, a create's or an update's
resource, a oneof, with explicit presence or without; the response's bytes
are printed in base64, as the API's JSON has them. In a nested message, a
`bytes` field is set through the JSON form, in base64. Any
other field holding a message is a flag taking comma-separated `KEY=VALUE`
pairs, one per field of the message that a flag could set on its own (a
number, a string, a boolean, an enum, a duration, a timestamp, a resource
name), each parsed and checked as such, and its required fields enforced; a
repeated one is that flag repeated. Every message field also has
a `--{field}-json` flag taking its JSON, the only form of what pairs can't
say, such as a message nested deeper or a list inside a message; a message
with a required field of that kind has the JSON flag alone, unless that field is
its only one and a repeated scalar (`BasicRecordData.values`): the message's field
is then that list's repeatable flag (`--value`), beside the JSON one; a well-known
type, such as a struct, takes its own JSON form. That flag checks its value
against the message's descriptor, from `api.binpb` (and `test-api.binpb` for
the test services), at every depth (`crates/cli/src/sdk/nested.rs`): a key
that is neither a field's name nor its JSON name, a key given twice, a value
of the wrong type, an enum value that isn't one (or is the unspecified one),
an element given twice in an `UNORDERED_LIST`, and an id where nothing
completes it are refused, naming the path; the SDK's decoder would drop or
default most of them silently. A resource name in a field of the message
itself, in either form, is completed and checked against `--project` and
`--zone` like a flag's. In an update, setting any of a field's flags names
the field itself in `update_mask` and sends it whole, as the API design
requires of nested and repeated fields, which the flags' one-line help says
and the command repeats on standard error, naming the keys it resets;
`--no-{field}` empties a field that isn't required. A repeated scalar or
resource name is a repeatable flag, refusing an element given twice in an
`UNORDERED_LIST`.

A field with explicit presence (proto3 `optional`) is an `Option` flag, and
a key of a nested message the same: sent when given, zero included, left
out when not; in an update, `--no-{field}` unsets it, naming it in the mask
as setting it does. A oneof of a request or of the resource it carries is
one flag per member, in a clap group allowing one at most; a member that is
a message has both of a message field's forms, both in the group. A oneof
whose members are all required is a choice a create must make (a required
group); one mixing required and optional members breaks the API's rules and
fails generation. The request carries the member given; an update names
every member in the mask, so that the one it sets replaces whichever was
set, and empties an optional oneof with `--no-{oneof}`. A oneof inside a nested message is set through the JSON form, which
refuses two of its members. A field the generator can't make a flag of,
such as a map other than `map<string, string>` or an optional resource
name, fails generation, naming the method and the field: nothing is left
out silently. So does a method answering with a file (`google.api.HttpBody`)
rather than JSON, which no command prints yet.

Every delete needs `--yes`. A delete answering with nothing says it deleted
the resource and that it can't be restored; one answering with the
resource, whose service has an `Undelete{Resource}` (AIP-164's soft
delete), prints the resource and names that command as what restores it.

The published API doesn't have such fields yet, so the tests use a service
of their own, `testing/scratch/`: a firewall whose rules are a repeated
nested message, with repeated scalars and references, a list whose order the
API doesn't keep, a nested message, timestamps, a struct, optional fields,
oneofs of the request, of the resource and of a nested message, `bytes`
fields of the request and of the resource, a soft delete, and a message
nested two levels deep, with enums, maps, lists and references of its own;
the networks it references have two name patterns.
`scripts/generate.sh` generates its SDK crate's code with the Rust SDK's own
generator, at the commit `Cargo.lock` pins, into
`testing/scratch/sdk/src/generated/`, the crate's manifest from the one that
generator scaffolds, and its group, `origin scratch`, beside
the others; that group is compiled only with the CLI's `test-services`
feature, never in a release, and isn't in `api.binpb`. Its tests,
`crates/cli/tests/scratch.rs`, run its commands against the SDK's fake
server; the generator's own tests compile it, and definitions it must
refuse, with the parts of googleapis in `generator/testdata/`.

What the API definitions don't express is in `generator/commands.toml`, by
service, method and request field (`pvs`, `UpdateVolume`, `volume.size_gib`):

- `hand_written` lists the hand-written groups the CLI still has (the
  coverage test checks it is the CLI's), `[[replacement]]` the services
  that replace one under another name (`resourcemanager` replaces `admin`),
  written before the API has them, and `[[retired]]` the hand-written
  commands dropped with nothing of the same name in their place, with the
  reason: see "A service replacing a hand-written group" below;
- `[[service]]` gives a group's help, for a package with several services;
  a package with one takes it from that service's comment. The generator
  warns, and the coverage test fails, when a package with several services
  has no entry;
- `[[no_command]]` leaves a method without a command, with the reason. The
  coverage test fails for any method of the API that has neither a command
  nor an entry here, which is what keeps the CLI at parity with the API;
- `[[exclude]]` leaves a field out of a method's command, when the method
  doesn't take it although its request message has it, with the reason;
- `[[shown_once]]` names a field of a method's response holding a secret
  the API returns in that response alone, such as `CreateKeyPair`'s
  `private_key`: the command prints the response as any other, and its
  `notice` on standard error when the field isn't empty;
- `[[to_file]]` names a string field of a method's response holding a
  secret the API returns in that response alone, which the command writes to
  a file instead of printing it, such as `CreateServiceAccountKey`'s
  `credentials_file`: its required flag `--{flag}` names the file, which is
  claimed before the request (created `0600`, an existing one refused
  unless `--overwrite`, then replaced only once the response is in, and
  removed again when the request fails, `runtime.rs`'s `SecretFile`); the
  printed response leaves the field out, and standard error names the file
  and the resource;
- `[[from_file]]` names a string field of a method's request, such as
  `AssumeServiceAccount`'s `oidc_token`, that its command reads from a file, or
  from standard input with `-`, rather than from the command line, where a
  credential would be in the shell's history and in every process listing:
  its flag is `--{field}-file`, and the value is the file's content without
  its final newline. Generation fails for a field that isn't a string: a
  `bytes` field is read from a file without an entry;
- `[[partial]]` names a message field of a method's request, such as
  `LaunchInstance`'s `launch_template_data`, whose `KEY=VALUE` form requires
  none of the keys its message requires, because the field overrides another
  message's, such as a launch template version's, and sets only what it
  changes. Generation fails for a field that has no `KEY=VALUE` form;
- `[[shortcut]]` adds a flag to a method that sets one field of a message
  field of its request, however deep in it (`LaunchInstance`'s `--subnet`
  sets `launch_template_data.network_interfaces[0].spec.subnet`), so that the
  common case needs no `KEY=VALUE` pairs or JSON. Its `path` goes from the
  message through its fields, a list of messages standing for its first
  element, and ends at a string or a list of strings; `conflicts_with` names
  flags it can't be given with. The values are put into the message
  as its JSON and checked as `--{field}-json` is (`nested.rs`'s
  `with_shortcuts`); a command refuses a shortcut whose field, or whose
  top field of the path, the message's own flags set too. Generation fails
  for a path the message doesn't have;
- `[[also_names]]` names a reference field, by its full name
  (`origincloud.compute.v1beta1.Instance.image`), that takes the names of
  another resource type too (`compute.origincloudapis.eu/PublicImage`),
  which its `google.api.resource_reference`, naming one type, can't say:
  its flag, key or JSON value takes a full name of either, on that
  type's service's host. Generation fails when the
  field isn't a reference to a type the API declares, or the other type
  isn't one;
- `[[exclusive]]` names fields of a method's request, by their paths from
  the request (`role_binding.project`, `role_binding.service_account`), at
  most one of which can be set, although the API doesn't make them a
  `oneof`, which an existing field can't join without breaking the wire
  format: their flags are an argument group, as a oneof's are, so that the
  command refuses them together before sending anything. Generation fails
  for a field that has no flag;
- `[[project_flag]]` names a method's `parent` that is a reference to a
  project (`observability`, `QueryTimeSeries`): its command takes it as
  `--project`, as every command does, an id defaulting to
  `ORIGINCLOUD_PROJECT` and the profile's `default-project`, instead of a
  flag taking a full name only;
- `[[default]]` gives an enum field of a request (`statistic`) the value its
  flag has when not given, which the API's definitions don't say. Generation
  fails for a field that isn't an enum;
- `[[window]]` names the three fields of a request's time window, its start
  and end (timestamps) and its period (a duration): the command takes the
  end as `--end-time`, now when not given, the start as `--start-time` or,
  before the end, `--since`, and the period as `--period`, the window's
  length divided by 60 (at least a minute) when not given
  (`runtime.rs`'s `time_window`);
- `[[view]]` names the columns of the `table` a list of a message is
  printed as (`compute`, `Instance`, `interface_addresses.ipv4_address`),
  as paths from the message through messages and lists of them; without
  one, the table has every field the message defines that fits in a cell,
  but its `urn`, its `creator` and its maps, and `wide` adds those after a
  view's. Generation and the coverage test fail for a view of a message or
  a field the API doesn't have.

The changes that can't be undone, such as growing a volume, aren't in
`commands.toml`: their one list is `[[one_way]]` in origincloudhq/mcp's
`generator/tools.toml`, which the MCP tools and the agent skills read too.
`scripts/generate.sh` hands the generator that table at the commit
`Cargo.lock` pins for `origincloud-mcp`, and copies it to
`crates/cli/src/sdk/generated/mcp-tools.toml` for the tests. Each entry
names the field, the refusal's verb and the consequence: a command setting
that field requires `--yes`, like every delete, and without it refuses
before sending anything ("refusing to grow volume …"). The generated code
reaches the SDK's method only through a wrapper that takes the client
`--yes` makes. A new one-way change is added there, and reaches the CLI
with `scripts/update.sh`; `commands.toml` refuses a `[[one_way]]` of its
own.

The human output formats (`crates/cli/src/output/`) are made from the
JSON a command prints, with what the API's descriptors, `api.binpb`, say
of each field: a timestamp, a reference, an enum, a map. A generated
command passes the full name of the message it prints (`json`), or of the
items of the list (`list_json`, with the view's columns, the parents
listed and the command creating one), and a delete the kind and name of
what it deleted (`deleted`).

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
to the newest origincloudhq/mcp, the SDK commit that is built on, and the API
revision it was generated from, and the skills to the newest
origincloudhq/skills, when origincloudhq/mcp's or origincloudhq/skills' main
moves, and daily, and opens a pull request when that changes anything. Once its CI
passes, `merge-regeneration.yml` merges it, unless someone added commits to
it, which then wait for review; the merge uses a GitHub App's token (the
variable `REGENERATE_APP_ID` and the secret `REGENERATE_APP_PRIVATE_KEY`),
and without it the pull request is merged by hand. If the API gained a method the
generator can't make a command for, generation fails, naming the method and
the field: the run opens or refreshes the pull request all the same, with
the SDK and `API_REVISION` moved alone and the generator's error in its
description, then fails. Extend the generator on that pull request's branch,
or leave the method (`[[no_command]]`) or the field (`[[exclude]]`) out with
the reason; the commits added there are kept, and the next run generates
with them.

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
   `[[service]]` entry in `generator/commands.toml`, and regenerate; the
   generator warns, and the coverage test fails, until it has one.
3. If the service replaces a hand-written group, by name (as `iam` did)
   or by a `[[replacement]]` entry (`resourcemanager` replaces `admin`),
   generation fails until that group is retired in the same pull request, as
   the next section says.
4. Mention the group in the README.

A list without a parent (a top-level collection, such as
`ListOrganizations`) and one filtering on other fields (`ListProjects`'s
required `organization`) page through the request as its flags make it; a
client-chosen id (AIP-133, `CreateProject`'s `project_id`) is a flag like any
request field, required when the API requires it. A reference to a resource
type the API doesn't declare, such as one of a package not published yet, is
a string flag taking the whole name, unchecked.

### A service replacing a hand-written group

A hand-written group is replaced by a service of the API in time, as `admin`, `compute`, `network` and `iam` were.
When the API gains the service that replaces one, the generated group lands
and the hand-written one goes in the same change, so that `origin` never has
both. A service replaces the group of its name; one replacing a group under
another name has a `[[replacement]]` entry in `generator/commands.toml`,
written before the API has the service, so that nobody has to remember it.
Generation fails until the group is retired, naming it, which on the
regeneration pull request puts the switch on that pull request. The switch:

1. In `crates/cli/src/root_command.rs`, remove the group's variant of
   `Commands` and its `use`; in `crates/cli/src/run_cli.rs`, its arm; in
   `crates/cli/src/handler/mod.rs`, its module, and delete
   `crates/cli/src/handler/{group}/`.
2. Delete whatever only the group used (clippy names what nothing uses
   any more).
3. Take the group off `hand_written` in `generator/commands.toml`, and add a
   `[[retired]]` entry, with the reason and what to use instead, for each of
   its commands no command of the same name replaces (such as `admin
   register-organisation`, or `iam`'s invitations): a generated `origin
   compute get-instance` replaces the hand-written one.
4. Regenerate, and update the README's list of groups.

`crates/cli/tests/hand_written.txt` lists every command the hand-written
groups have had, and a line is never removed from it. The coverage test
checks that `hand_written` is the CLI's hand-written groups; that every
hand-written command is in the list; that every command in the list is still
a command, hand-written or generated under the same name, or has a
`[[retired]]` entry; that every `[[retired]]` entry names a command in the
list that is gone; and that every `[[replacement]]` names a group in the list,
not still hand-written once its service is generated. A hand-written command
added to the CLI is added to the list in the same change.

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
scripts/update.sh              # move to the newest SDK, its API revision and the newest skills, and generate
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# the scratch service's group, and the tests of nested and repeated fields
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p origincloud-cli --all-features
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
