# Origin Cloud CLI

`origin` is the command-line interface for [Origin Cloud](https://origincloud.eu):
log in, then manage organisations, projects, access, instances, networks and
volumes, and look up regions and zones, from a terminal or a script. Every
command prints JSON.

## Install

The CLI is published at `packages.origincloud.eu`, as one binary per platform
(`origin-linux-amd64`, `origin-linux-arm64`, `origin-darwin-arm64`,
`origin-darwin-amd64`) with a `SHA256SUMS` file and a GPG signature over it.
The Linux builds are statically linked.

With the install script, which picks the binary for your machine, checks it
against `SHA256SUMS`, and `SHA256SUMS` against the release key's signature
when `gpg` is installed, and installs it to `~/.local/bin`:

```sh
curl -fsSL https://packages.origincloud.eu/install.sh | sh
origin --version
```

`ORIGIN_VERSION` (such as `v1.10.0`) installs a specific version,
`ORIGIN_INSTALL_DIR` another directory, and `ORIGIN_BASE_URL` downloads the
binary from an HTTPS mirror instead; with a mirror, `SHA256SUMS` still comes
from packages.origincloud.eu and its signature must verify, so `gpg` is
required.

With apt, on Debian and Ubuntu (`amd64` and `arm64`):

```sh
sudo curl -fsSLo /usr/share/keyrings/origincloud-archive-keyring.gpg \
  https://packages.origincloud.eu/apt/origincloud-archive-keyring.gpg
echo "deb [signed-by=/usr/share/keyrings/origincloud-archive-keyring.gpg] https://packages.origincloud.eu/apt stable main" \
  | sudo tee /etc/apt/sources.list.d/origincloud.list
sudo apt-get update
sudo apt-get install origincloud-cli
```

The release key's fingerprint is
`FEE6 FD74 A88C 5DB8 959F  5A1B 0BE6 AF84 2E0B 034A`. Each version lives under
`https://packages.origincloud.eu/cli/<version>/` for a manual download.

To build it yourself instead, with the Rust toolchain `rust-toolchain.toml`
names:

```sh
cargo install --locked --path crates/cli
```

## Use

```sh
origin id login                      # log in through your browser
origin id login --device             # or with a code, on a machine without one
origin admin list-organisations
origin location list-zones
origin pvs list-volumes --project my-project-123 --zone eu-central1-az1
origin --help                        # every command group
origin pvs create-volume --help      # every flag of a command
```

The login session is stored in `~/.local/share/origincloud/session.json`
(under `$XDG_DATA_HOME` when set), readable by you only, and renewed as it
expires; the profile, with your defaults, in
`~/.config/origincloud/default-profile.json` (under `$XDG_CONFIG_HOME`), or the
file `--config` names:

```sh
origin config profile-set default-project my-project-123
origin config profile-set default-zone eu-central1-az1
origin config profile
```

| Profile key | Environment | Used for |
|---|---|---|
| `default-username` | `ORIGINCLOUD_USERNAME` | Prefilled on the login page. |
| `output-format` | `ORIGINCLOUD_OUTPUT_FORMAT` | `json`, the only format; `--output` overrides it. |
| `default-project` | `ORIGINCLOUD_PROJECT` | `--project` of the generated commands, such as `origin pvs`'s, when not given. |
| `default-zone` | `ORIGINCLOUD_ZONE` | `--zone` of the generated commands, when not given; not of the lists, which list every zone without `--zone`. |
| `region` | `ORIGINCLOUD_REGION` | The region whose front door the `pvs` commands call, `pvs.{region}.origincloudapis.eu`; the regionless `pvs.origincloudapis.eu`, which serves every resource, when unset. The `location` commands always call `location.origincloudapis.eu`. |

An environment variable overrides the profile's value while a command runs;
`origin config profile-set` changes the file alone, and checks the value
first. A command that fails
prints `{"code": "failure", "description": "..."}` and exits with status 1;
a command line the CLI can't parse exits with status 2 before sending
anything.

Irreversible commands (every delete, `terminate-instance`,
`release-floating-ip`, `revoke-service-account`, and growing a volume with
`origin pvs update-volume --size-gib`) refuse to run without `--yes` and say
what they would have done; the refusal happens before any request.

## The generated commands: `pvs` and `location`

`origin pvs` covers the persistent volume service: volumes, snapshots and
storage classes, and the operations its changes answer with. `origin location`
covers the regions and zones, and which services each zone offers. Their
commands are generated from the API definitions in
[origincloudhq/api](https://github.com/origincloudhq/api) and call the
[Rust SDK](https://github.com/origincloudhq/sdk-rust), so there is one for
every method of the API, with the API's own descriptions as help:

- **Command names.** The service's name, then the method's name in kebab
  case: `CreateVolume` is `origin pvs create-volume`, `ListZones` is
  `origin location list-zones`.
- **Resource names.** A resource is named by its parts, each an id of
  letters, digits, `-` and `_`. `--project` and
  `--zone` are shared by every name a command builds, and default to the
  profile's `default-project` and `default-zone`; the resource's own id is the
  flag named after it (`--volume vol-123`, `--zone eu-central1-az1` for
  `origin location get-zone`), or after the field for a reference
  (`--source-snapshot snapshot-123`, `--storage-class gp1`). That flag also
  takes a whole name, such as
  `--volume projects/my-project-123/zones/eu-central1-az1/volumes/vol-123`,
  whose project and zone then fill in the others: the `name` of a listed
  volume can be passed straight on.
- **Other flags.** Each other field of the request, and of the volume or
  snapshot it carries, is the flag of its name in kebab case: `size_gib` is
  `--size-gib`. Output-only fields aren't flags, and neither is a field the
  method doesn't take although its request has it (`import-volume` has no
  `--source-snapshot`). A boolean takes its value, `--flag true` or
  `--flag false`, and a create always states it. A duration is given as
  `30s`, `2m` or `1m30s`.
- **Tags.** `--tag KEY=VALUE`, repeated for more. On an update, `--tag`
  replaces all of a resource's tags, and `--no-tags` removes them all; the two
  exclude each other, so the tags are only emptied when you say so. To set or
  remove one tag and keep the others, use `origin storage set-resource-tags`
  and `origin storage remove-resource-tags`.
- **Updates.** An update changes only what its flags name: `update-volume
  --volume vol-123 --tag team=web` replaces its tags and keeps its size. It
  needs at least one. Growing a volume can't be undone and is charged, so
  `--size-gib` needs `--yes`.
- **Waiting.** A command whose method answers with a long-running operation
  (most creates, updates and deletes) waits until it is done and prints the
  result; `--no-wait` prints the operation as soon as
  the API accepts it. Ctrl-C stops the waiting, not the operation.
- **Operations.** A service whose methods answer with operations has two
  commands for them, which call its own host: `get-operation` prints an
  operation as it is now, and `wait-operation` waits until it is done and
  prints it, with its `response` or its `error`, exiting with status 1 when
  it failed. `--timeout` (such as `5m`) bounds the wait: when it passes
  first, `wait-operation` prints the operation as it is and exits with
  status 1. Both take the operation's `name`, as `--no-wait` printed it, or
  its id with `--project` and `--zone`:

  ```sh
  origin pvs create-volume --size-gib 8 --storage-class gp1 --no-wait
  origin pvs wait-operation --operation projects/my-project-123/zones/eu-central1-az1/operations/op-123
  ```

- **Lists.** A list command lists every page; `--page-size` or `--page-token`
  asks for one page, whose `nextPageToken` names the next. Without `--zone`,
  `list-volumes` and `list-snapshots` list every zone that offers the service,
  in catalog order, as one list; one page needs `--zone`. An empty list prints
  as `[]`.
- **Output.** The API's JSON: the resource as the API returns it, with its
  fields in camel case, such as `sizeGib`.

- **Credentials.** The login session, or, when they are set, the variables
  every Origin Cloud SDK reads: `ORIGINCLOUD_ACCESS_TOKEN`, or
  `ORIGINCLOUD_CLIENT_ID` and `ORIGINCLOUD_CLIENT_SECRET` for a service
  account.

`--endpoint URL`, an option of each of these groups, such as
`origin pvs --endpoint URL`, sends that group's calls to another base URL,
such as a test server's: `https://`, or `http://` to this machine only. Each
group has its own environment variable for it, `ORIGINCLOUD_PVS_ENDPOINT` and
`ORIGINCLOUD_LOCATION_ENDPOINT`, so that redirecting one service leaves the
others at their own hosts. A `pvs` list without `--zone` looks up the zones at
the location service, so `ORIGINCLOUD_LOCATION_ENDPOINT`, not the `pvs`
endpoint, redirects that lookup. The CLI says on standard error whenever an
endpoint is used.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md), which describes how the `pvs` and
`location` commands are generated.

## License

Apache License 2.0; see [LICENSE](LICENSE).
