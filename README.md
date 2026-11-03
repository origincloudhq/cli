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
origin resourcemanager list-organizations
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
origin config profile-set region eu-central1
origin config profile
```

| Profile key | Environment | Used for |
|---|---|---|
| `default-username` | `ORIGINCLOUD_USERNAME` | Prefilled on the login page. |
| `output-format` | `ORIGINCLOUD_OUTPUT_FORMAT` | `table`, `wide`, `json` or `name` (see [Output](#output)); `--output` (`-o`) overrides it. |
| `default-project` | `ORIGINCLOUD_PROJECT` | `--project` of the generated commands, such as `origin pvs`'s, when not given. |
| `default-zone` | `ORIGINCLOUD_ZONE` | `--zone` of the generated commands, when not given; not of the lists, which list every zone without `--zone`. |
| `region` | `ORIGINCLOUD_REGION` | The region the generated commands work in: the `--region` of the `network` commands' regional resources, when not given, and the front door the `compute`, `network` and `pvs` commands call, such as `pvs.{region}.origincloudapis.eu`. Without it, the region of the default zone (`eu-central1` for `eu-central1-az1`); without either, `--region` must be given, and the calls go to the regionless host, such as `pvs.origincloudapis.eu`, which serves every resource. The `location` and `resourcemanager` commands always call their regionless host. |

As in the AWS CLI, a flag comes first, then the environment variable, then
the profile's value: an environment variable overrides the profile while a
command runs, and `origin config profile-set` changes the file alone, and
checks the value first.

### Output

Unless `--output` (`-o`), `ORIGINCLOUD_OUTPUT_FORMAT` or the profile's
`output-format` says otherwise, `origin` prints for whoever reads it:
`table` on a terminal, and `json` when its output goes to a pipe or a file,
so that a script, such as `origin pvs list-volumes | jq .`, gets the JSON
the API sends whether or not it asks for it.

| Format | Prints |
|---|---|
| `table` | A list as a table, its resources by the last segment of their name, fitted to the terminal's width (or `COLUMNS`); a resource as its fields, one per line, under their JSON names; `Done.` for a command without a result, such as a delete. An empty list, and the `--page-token` of the next page, are said on standard error. |
| `wide` | As `table`, a list with every field that fits in a cell, none cut. |
| `json` | The response as the API sends it, pretty-printed. |
| `name` | Each resource's whole name, one per line, the form every resource flag takes: `origin pvs list-volumes -o name \| xargs -n1 origin pvs get-volume --volume`. |

`table`, `wide` and `name` are in colour on a terminal, unless `NO_COLOR` is
set; `--color always` or `never` decides instead. `json` is never in colour.

A command that fails exits with status 1: in `json`, it prints
`{"code": "failure", "description": "..."}` on standard output, as it
always has; in the other formats, `error: ...` on standard error. A command
line the CLI can't parse exits with status 2 before sending anything.

Irreversible commands (every delete, `origin iam revoke-service-account`,
and growing a volume with `origin pvs update-volume --size-gib`) refuse to run without `--yes` and say
what they would have done; the refusal happens before any request.

`origin compute create-key-pair` without `--public-key` prints the private
key it generates as `privateKey`, which the API returns this once, and says
so on standard error: save it, readable by you alone, such as with
`origin compute create-key-pair | (umask 077; jq -r .privateKey > my-key)`.

## The generated commands

A group per service of the API:

- `origin compute`: instances, images, key pairs, volume attachments, and the
  instance types and image families on offer.
- `origin network`: VPCs, subnets, route tables and routes, ACLs, security
  groups and their rules, internet and NAT gateways, floating IPs, network
  interfaces and DNS policies, and the associations between them.
- `origin pvs`: the persistent volume service's volumes, snapshots and
  storage classes.
- `origin resourcemanager`: organizations and projects, and their quotas.
- `origin location`: the regions and zones, and which services each zone
  offers.

Their commands are generated from the API definitions in
[origincloudhq/api](https://github.com/origincloudhq/api) and call the
[Rust SDK](https://github.com/origincloudhq/sdk-rust), so there is one for
every method of the API, with the API's own descriptions as help:

- **Command names.** The service's name, then the method's name in kebab
  case: `CreateVolume` is `origin pvs create-volume`, `ListZones` is
  `origin location list-zones`.
- **Resource names.** A resource is named by its parts, each an id of
  letters, digits, `-` and `_`. `--project` and
  `--zone` are shared by every name a command builds, and default to the
  profile's `default-project` and `default-zone`; a regional resource of
  `origin network`, such as a VPC, takes `--region` instead of `--zone`,
  which defaults to the profile's `region`, or the region of its
  `default-zone`; the resource's own id is the
  flag named after it (`--volume vol-123`, `--zone eu-central1-az1` for
  `origin location get-zone`), or after the field for a reference
  (`--source-snapshot snapshot-123`, `--storage-class gp1`). That flag also
  takes a whole name, such as
  `--volume projects/my-project-123/zones/eu-central1-az1/volumes/vol-123`,
  whose project and zone then fill in the others: the `name` of a listed
  volume can be passed straight on. A part of a parent's name without a
  default, such as `--instance` of `origin compute
  create-volume-attachment`, is required, unless a whole name given to
  another flag has it.
- **Other flags.** Each other field of the request, and of the volume or
  snapshot it carries, is the flag of its name in kebab case: `size_gib` is
  `--size-gib`. Output-only fields aren't flags, and neither is a field the
  method doesn't take although its request has it (`import-volume` has no
  `--source-snapshot`). A boolean takes its value, `--flag true` or
  `--flag false`; left out, one the API doesn't require is `false`. A
  duration is given as `30s`, `2m` or `1m30s`, a time as RFC 3339, such as
  `2026-11-01T12:00:00Z`.
- **Tags.** `--tag KEY=VALUE`, repeated for more. On an update, `--tag`
  replaces all of a resource's tags, and `--no-tags` removes them all; the two
  exclude each other, so the tags are only emptied when you say so. To set or
  remove one tag and keep the others, give every tag to keep with `--tag`:
  `origin pvs get-volume` and `get-snapshot` show the current ones.
- **Lists.** A repeated field is a flag named in the singular, repeated for
  each element, in order: `--security-group sg-1 --security-group sg-2`.
  Where the API doesn't keep a list's order, the flag's help says so, and an
  element given twice is refused.
- **Optional fields.** A field the API marks optional is left out when its
  flag isn't given, and sent when it is, zero or empty included
  (`--max-connections 0`). An update sets it, or with `--no-` and its name
  (`--no-max-connections`) unsets it.
- **Choices.** A oneof, a choice of fields of which a request carries at most
  one, is one flag per field, and giving two is refused: `--mirror-network`,
  `--mirror-address` or `--mirror-log`. Where the API requires a choice, a
  create must give exactly one. An update sets the one given, which replaces
  whichever was set, and `--no-` and the choice's name (`--no-mirror`) sets
  none of an optional one.
- **Nested fields.** A field holding a message is one flag whose value is
  comma-separated `KEY=VALUE` pairs, the keys being the message's field
  names, with `_` or `-`; a repeated one takes one flag per element:

  ```sh
  --rule protocol=TCP,port=443,cidr=0.0.0.0/0 --rule protocol=UDP,port=53
  ```

  Each key is typed and checked like a flag of its own (an enum's values, a
  number, `true` or `false`, a duration, a time, a resource name by its id or
  whole name), a required key must be given, and the flag's help lists the
  keys. Every such field also has a `-json` flag taking the field as the
  API's JSON, or `@FILE` to read it from a file, for what pairs can't say: a
  value with a comma, or a message nested deeper. Its fields are named as the
  command prints them, in camel case, or as the API definitions name them.
  Before anything is sent it is checked at every depth, each refusal naming
  its path: a field the message doesn't have, a key given twice, a value of
  the wrong type, an enum value that isn't one, an element given twice where
  the API keeps each once. A resource name in a field of the message itself
  is completed with `--project` and `--zone`, as in the pairs; one nested
  deeper must be whole:

  ```sh
  --rules-json '[{"protocol": "TCP", "match": {"headers": [{"name": "x-env", "value": "prod"}]}}]'
  ```

- **Updates.** An update changes only what its flags name: `update-volume
  --volume vol-123 --tag team=web` replaces its tags and keeps its size. It
  needs at least one. A list, a map or a nested message is replaced whole:
  give every element to keep, and the keys of a nested message not given are
  reset; the flags' one-line help says so, and the command says on standard
  error which keys it resets. `--no-` and the field's name (`--no-logging`)
  empties one that isn't required. Growing a volume can't be undone and is
  charged, so `--size-gib` needs `--yes`.
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
  a list of zonal resources, such as `list-volumes` or `list-instances`,
  lists every zone that offers the service, in catalog order, as one list;
  one page needs `--zone`. An empty list prints
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
group has its own environment variable for it, `ORIGINCLOUD_{SERVICE}_ENDPOINT`,
such as `ORIGINCLOUD_PVS_ENDPOINT`, so that redirecting one service leaves the
others at their own hosts. A list without `--zone` looks up the zones at
the location service, so `ORIGINCLOUD_LOCATION_ENDPOINT`, not the group's
endpoint, redirects that lookup. The CLI says on standard error whenever an
endpoint is used.

## AI agents: `origin mcp serve`

`origin mcp serve` serves the API to an AI agent as an
[MCP](https://modelcontextprotocol.io) server, on standard input and output.
An MCP client starts it as a local server, such as an AI coding assistant:

```sh
claude mcp add origincloud -- origin mcp serve
```

or, for a client configured in JSON:

```json
{ "mcpServers": { "origincloud": { "command": "origin", "args": ["mcp", "serve"] } } }
```

- **Tools.** A tool per method of the API, generated like the commands:
  `pvs_create_volume`, `location_list_zones`, and each service's
  `_get_operation` and `_wait_operation`. Each takes the method's request in
  the API's JSON, as its input schema describes, and answers with the
  response. Arguments the schema doesn't describe, such as a misspelt field,
  are refused, not dropped. `current_context` tells the agent the profile's
  `default-project` and `default-zone`, and the region the commands use.
- **Resource names.** Whole, such as
  `projects/my-project-123/zones/eu-central1-az1/volumes/vol-123`.
- **Operations.** A tool starting an operation waits for it up to 30
  seconds, and answers with the operation: done, with its result or its
  error, or not done yet, for the service's `_wait_operation` tool.
- **Confirmation.** The tools that delete something, or change it for good
  (growing a volume), are marked destructive, for the MCP client to have
  you confirm; they don't ask for `--yes`. `--read-only` serves only the
  tools that change nothing.
- **Toolsets.** A service's tools are its toolset: `--toolsets pvs,location`
  serves only those.
- **Credentials and endpoints.** As the generated commands': the login
  session or the SDK's variables, the region the commands use, and each
  service's endpoint variable, such as `ORIGINCLOUD_PVS_ENDPOINT`.

The tools are the library [origincloudhq/mcp](https://github.com/origincloudhq/mcp),
which a server on the network can serve as well.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md), which describes how the commands of
each service are generated.

## License

Apache License 2.0; see [LICENSE](LICENSE).
