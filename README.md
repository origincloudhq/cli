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

### Shell completion

`origin` completes its commands, flags and their values in bash, zsh, fish,
PowerShell and elvish. Run `origin completion` on a terminal: it says how to
install the completion in your shell (`SHELL`'s, or the one you name, as in
`origin completion zsh`). Into a file or a pipe, it prints the script
itself. For bash with the bash-completion package:

```sh
mkdir -p ~/.local/share/bash-completion/completions
origin completion bash > ~/.local/share/bash-completion/completions/origin
```

without it, add `eval "$(origin completion bash)"` to `~/.bashrc`; for zsh,
add `eval "$(origin completion zsh)"` to `~/.zshrc`, after `compinit`; for
fish:

```sh
mkdir -p ~/.config/fish/completions
origin completion fish > ~/.config/fish/completions/origin.fish
```

Then open a new shell.

## Use

```sh
origin id login                      # log in through your browser
origin id login --device             # or with a code, on a machine without one
origin id login --oidc-token-file t  # or as a service account, with your identity provider's token
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
file `--config` names.

`origin id login --oidc-token-file PATH` (or `-` for standard input) logs in as a
`WORKLOAD` service account instead. It assumes the account with a token from
your own identity provider, which one of the account's federated credentials
trusts, at the security token service; `--service-account` names the
account, when more than one could trust the token. Once the account's access
token expires, the CLI reads the file again and assumes the account again,
so a file the identity provider or a credential helper keeps current keeps
the session going; a token from standard input is read once.

The profile's defaults:

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
| `region` | `ORIGINCLOUD_REGION` | The region the generated commands work in: the `--region` of the `network` commands' regional resources, when not given, and the front door the `compute`, `network`, `pvs` and `observability` commands call, such as `pvs.{region}.origincloudapis.eu`. Without it, the region of the default zone (`eu-central1` for `eu-central1-az1`); without either, `--region` must be given, and the calls go to the regionless host, such as `pvs.origincloudapis.eu`, which serves every resource. The `location`, `resourcemanager`, `iam`, `billing`, `support`, `dns` and `sts` commands always call their regionless host. |

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
| `table` | A list as a table of the columns that tell its resources apart, fitted to the terminal's width (or `COLUMNS`); a resource as its fields, one per line, under their JSON names; what a delete did, such as `✓ Deleted volume v1.` An empty list says on standard error where it looked and the command creating one; the `--page-token` of the next page is said there too. |
| `wide` | As `table`, a list with every other field that fits in a cell after those, none cut. |
| `json` | The response as the API sends it, pretty-printed. |
| `name` | Each resource's whole name, one per line, the form every resource flag takes: `origin pvs list-volumes -o name \| xargs -n1 origin pvs get-volume --volume`. |

In `table` and `wide`, a reference to another resource is shown by its id
(`gp1` for `storageClasses/gp1`) and a time by how long ago it was (`3d
ago`); a resource's own lines show both whole, the time beside how long ago.
A state is in colour: green for what is up and in use (`RUNNING`,
`AVAILABLE`), yellow for what is changing (`STARTING`, `PENDING`), red for
what failed, dim for what is stopped or gone.

`table`, `wide` and `name` are in colour on a terminal, unless `NO_COLOR` is
set; `--color always` or `never` decides instead. `json` is never in colour.

On a terminal, `table` and `wide` output taller than the screen (or `LINES`)
goes through a pager, colour kept: `ORIGINCLOUD_PAGER`, else `PAGER`, else
`less`, given `LESS=FRX` unless `LESS` is set. A pager variable set to
nothing or `cat`, or `--no-pager`, prints it as it is.

A command that fails exits with status 1: in `json`, it prints
`{"code": "failure", "description": "..."}` on standard output, as it
always has; in the other formats, `error: ...` on standard error. A call the
API refused adds what it said of it, each left out when it sent none:
`reason`, its error code such as `CONFLICT`; `metadata`, what the reason
names, such as the service account under `serviceAccount`; and `requestId`,
to quote to support. A script reads these instead of parsing the
description. A command line the CLI can't parse exits with status 2 before
sending anything.

Irreversible commands (every delete, and growing a volume with
`origin pvs update-volume --size-gib`) refuse to run without `--yes` and say
what they would have done; the refusal happens before any request.

`origin compute create-key-pair` without `--public-key` prints the private
key it generates as `privateKey`, which the API returns this once, and says
so on standard error: save it, readable by you alone, such as with
`origin compute create-key-pair | (umask 077; jq -r .privateKey > my-key)`.
`origin iam create-service-account` does the same with the service
account's client secret, `clientSecret`.

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
- `origin iam`: role bindings, which grant a role to a member
  (`user:{email}` or `serviceAccount:{id}`) in an organization or one of its
  projects; service accounts; the roles and permissions on offer, and which
  permissions you hold; your user; and an organization's audit log.
- `origin location`: the regions and zones, and which services each zone
  offers.
- `origin billing`: billing accounts and the projects they pay for, payment
  methods, cost and usage, spend limits and the price list.
- `origin support`: support cases and their messages.
- `origin observability`: the metrics of a project's compute instances,
  volumes and network interfaces.
- `origin dns`: hosted zones, public or private, their record sets, and
  the VPCs a private zone is attached to.
- `origin sts`: the security token service, which turns an identity
  provider's token, such as a CI job's OIDC token, into the access token of
  a service account whose federated credential trusts it
  (`assume-service-account --oidc-token-file`). It needs no login.

Their commands are generated from the API definitions in
[origincloudhq/api](https://github.com/origincloudhq/api) and call the
[Rust SDK](https://github.com/origincloudhq/sdk-rust), so there is one for
every method of the API, with the API's own descriptions as help:

- **Command names.** The service's name, then the method's name in kebab
  case: `CreateVolume` is `origin pvs create-volume`, `ListZones` is
  `origin location list-zones`.
- **Resource names.** A resource is named by its parts, each an id, such as
  `vol-123` or `gp1.core.2`: anything but empty, `.` or `..`, and without
  `/`. `--project` and
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
  volume can be passed straight on. Where a flag takes more than one form
  of name, its help names each, and an id is the first: `--image` of
  `origin compute create-instance` takes one of the project's own images,
  `projects/{project}/zones/{zone}/images/{image}`, which its id
  completes to, or a public one, `zones/{zone}/images/{image}`. A part of a parent's name without a
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
  the API accepts it. On a terminal, standard error shows what it waits for
  and how long it has, such as `⠼ Creating volume v1… 14s`, on a line
  cleared when the wait is over. Ctrl-C stops the waiting, not the
  operation: the command says how to wait for it again with
  `wait-operation`, and exits with status 130.
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
  account. In a CI job, the job's OIDC token, exchanged at the security
  token service for the service account a federated credential trusts the
  job as: GitHub Actions' token in a job granted
  `permissions: id-token: write`, the token in the file
  `ORIGINCLOUD_OIDC_TOKEN_FILE` names, or the token `ORIGINCLOUD_OIDC_TOKEN`
  holds, as GitLab CI's `id_tokens` sets it (a token given that way can't be
  renewed, and an expired one is refused before the exchange). It comes before the login session,
  covers every command and `origin id token`, and a refused exchange is the
  command's error; `ORIGINCLOUD_OIDC_DISABLED=true` leaves it out. Without
  any of these, on an instance that runs as a service account,
  the credential of that service account from the instance's metadata
  service: `origin` works there with nothing to configure, and
  `origin id token` prints that credential for other tools. Off an
  instance this last step gives up within about two seconds;
  `ORIGINCLOUD_METADATA_DISABLED=true` leaves it out.

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

### Adding it to a coding agent: `origin mcp install`

`origin mcp install` adds that entry to a coding agent's MCP servers, and
`origin mcp uninstall` removes it again:

```sh
origin mcp install --agent claude-code                    # for every project, with every tool
origin mcp install --agent codex,cursor --read-only       # only the tools that change nothing
origin mcp install --agent github-copilot --scope project --toolsets pvs,location
origin mcp install --agent cursor --print                 # print the entry, change nothing
origin mcp uninstall --agent claude-code
```

- **Agents.** `--agent` takes the agents of the table below, separated by
  commas. On a terminal without it, `origin mcp install` offers the agents
  it finds on the machine; without a terminal, `--agent` is required.
- **Scope.** `--scope user` (the default) is the agent's own
  configuration, for every project; `--scope project` is the project in
  the current directory, in a file that is usually committed with it.
- **The entry.** Named `origincloud` (`--name` names it otherwise), it
  starts `origin mcp serve` with `--toolsets` and `--read-only` as given,
  and with `--config` when `origin --config FILE mcp install` named one. It
  holds no credentials, nor anything else from the login or the
  environment: `origin mcp serve` uses the login and the profile as the
  other commands do, so a project's file can be committed.
- **How.** Where the agent has a command for it (`claude mcp add`, `codex
  mcp add`, `gemini mcp add`) and it is on `PATH`, `origin mcp install`
  runs that, so that the agent writes its own file; then it checks the
  entry is in the file the table names. Otherwise it edits the file: the
  entry alone, keeping the other servers and settings, their order, and
  the comments where the format has them, after copying the file to
  `FILE.origin.bak`. Running it again updates the entry; a file that
  doesn't parse, or an entry of the name that doesn't start `origin mcp
  serve`, is left as it is, and the command fails.
- **`PATH`.** An entry names `origin`, for the agent to find on its `PATH`;
  Claude Desktop, started from a desktop launcher without your shell's
  `PATH`, gets the binary's absolute path instead, as any agent does with
  `--absolute-path`.
- **`--print`** prints the entry as the agent's file would hold it, or in
  the `mcpServers` form most other MCP clients read without `--agent`, and
  changes nothing.

The agents, and where each keeps its servers (`origin mcp install --help`
lists the same):

<!-- agents: generated from crates/cli/src/handler/agents.rs -->
| `--agent` | Agent | User scope | Project scope | Format | Added with |
|---|---|---|---|---|---|
| `claude-code` | Claude Code | `~/.claude.json`, or `$CLAUDE_CONFIG_DIR/.claude.json` | `.mcp.json` | JSON, `mcpServers` | `claude mcp add` when `claude` is on PATH, else the file |
| `codex` | OpenAI Codex | `~/.codex/config.toml`, or `$CODEX_HOME/config.toml` | `.codex/config.toml` | TOML, `mcp_servers` | `codex mcp add` (user scope) when `codex` is on PATH, else the file |
| `cursor` | Cursor | `~/.cursor/mcp.json` | `.cursor/mcp.json` | JSON, `mcpServers` | the file |
| `gemini-cli` | Gemini CLI | `~/.gemini/settings.json` | `.gemini/settings.json` | JSON with comments, `mcpServers` | `gemini mcp add` when `gemini` is on PATH, else the file |
| `github-copilot` | GitHub Copilot (VS Code, Copilot CLI) | `~/.copilot/mcp-config.json`, or `$COPILOT_HOME/mcp-config.json` | `.mcp.json` | JSON, `mcpServers` | the file |
| `claude-desktop` | Claude Desktop | `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS), `~/.config/Claude/claude_desktop_config.json` (Linux) | – | JSON, `mcpServers`; absolute path of `origin` | the file |
<!-- /agents -->

Restart the agent, or start a new session, for it to start the server. Claude
Code and GitHub Copilot share a project's `.mcp.json`, and with it the entry.

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

## Agent skills: `origin skill install`

The [agent skills](https://agentskills.io) of
[origincloudhq/skills](https://github.com/origincloudhq/skills) teach a coding
agent to use Origin Cloud: `origin` for this CLI, `origincloud-terraform` and
`origincloud-sdk`. `origin` carries them, from the origincloudhq/skills commit
`SKILLS_REVISION` names, so the `origin` skill describes the commands of the
`origin` you have, and installing needs no network access:

```sh
origin skill install --agent claude-code                  # the origin skill, for every project
origin skill install --all --agent codex,cursor --scope project
origin skill install origincloud-terraform --dir ./skills # a directory of your own
origin skill list                                         # what is installed where, in which version
origin skill update                                       # bring the installed copies to this version
```

- **Agents and scope.** `--agent` takes the agents of the table below,
  separated by commas; on a terminal without it, `origin skill install`
  offers the agents it finds on the machine, and without a terminal,
  `--agent` (or `--dir`) is required. `--scope user` (the default) is the
  agent's own skill directory; `--scope project` the project in the current
  directory, where the agents that read `.agents/skills` share one copy.
- **The copy.** Each skill is a directory, `SKILL.md` and its references,
  written beside the old one and renamed over it. Its `SKILL.md` gets two
  keys in its frontmatter's `metadata`: `origin-skills-revision`, the
  origincloudhq/skills commit, and `origin-skills-digest`, the digest of the
  skill's files. They say which version it is, and that no one changed it
  since `origin` wrote it.
- **Someone else's.** A skill's directory `origin` didn't install, such as
  one `gh skill install` did, or one changed since, is left as it is, and
  nothing is installed, unless `--force` is given; `origin skill update`
  skips it. The agent's other skills are never touched. A skill directory
  kept as a symbolic link stays one, and its target is written.
- **Versions.** `origin skill list` shows each skill's version and every
  copy: `current`, `outdated`, `modified`, or `unmanaged`, a copy `origin`
  didn't install. After an upgrade of `origin`, the first command run on a
  terminal says which copies are outdated, once, on standard error, and
  never in `json` output.

The skills are in the repository too, for other tools:
`gh skill install origincloudhq/skills origin --agent claude-code --scope
user` installs the same skill, with GitHub CLI's own keys in place of
`origin`'s, and `gh skill update` updates it.

The agents, and the directories they read skills from (`origin skill install
--help` lists the same):

<!-- skill-agents: generated from crates/cli/src/handler/agents.rs -->
| `--agent` | Agent | User scope | Project scope |
|---|---|---|---|
| `claude-code` | Claude Code | `~/.claude/skills`, or `$CLAUDE_CONFIG_DIR/skills` | `.claude/skills` |
| `codex` | OpenAI Codex | `~/.agents/skills` | `.agents/skills`, shared |
| `cursor` | Cursor | `~/.cursor/skills` | `.agents/skills`, shared |
| `gemini-cli` | Gemini CLI | `~/.gemini/skills` | `.agents/skills`, shared |
| `github-copilot` | GitHub Copilot (VS Code, Copilot CLI) | `~/.copilot/skills`, or `$COPILOT_HOME/skills` | `.agents/skills`, shared |
<!-- /skill-agents -->

Start a new session of the agent for it to find a new skill.

## The command tree, for tools

Each [release](https://github.com/origincloudhq/cli/releases) has a
`commands.json`: every visible command, groups and leaves, in the order
`origin` defines them, with its flags, and for a generated command the API
method it calls and whether it starts an operation. A tool naming
`origin`'s commands and flags, such as documentation or an agent's
instructions, reads it instead of parsing the help. `origin __commands`
prints it for the installed version.

```json
{
  "cli_version": "1.18.0",
  "api_revision": "<the origincloudhq/api commit the commands are generated from>",
  "commands": [
    {
      "path": ["pvs", "create-volume"],
      "about": "Creates a volume, empty or as a copy of a snapshot",
      "method": "origincloud.pvs.v1beta1.VolumeService.CreateVolume",
      "long_running": true,
      "args": [
        {"long": "size-gib", "short": null, "value_name": "SIZE_GIB", "required": true, "multiple": false,
         "global": false, "positional": false, "help": "The size in GiB, at least 1. …", "possible_values": []}
      ]
    }
  ],
  "global_args": [
    {"long": "output", "short": "o", "value_name": "FORMAT", "required": false, "multiple": false,
     "global": true, "positional": false, "help": "Output format. …", "possible_values": ["table", "wide", "json", "name"]}
  ]
}
```

- `method` and `long_running` are `null` for a group and for a hand-written
  command (`id login`, `config profile`, `mcp serve`, `completion`); the
  operation commands (`get-operation`, `wait-operation`) name
  `origincloud.longrunning`'s methods, and start no operation themselves.
- A leaf's `args` include the global flags of its group, such as
  `--endpoint`; a group's are empty. `global_args` are `origin`'s own
  options: those with `global` set go anywhere on the command line, the
  others (`--config`, `--api`) before the command.
- `about` and `help` are the first paragraph of the help, on one line.
  `--help` and `--version` aren't listed.

Fields may be added; none is renamed or removed within a major version.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md), which describes how the commands of
each service are generated.

## License

Apache License 2.0; see [LICENSE](LICENSE).
