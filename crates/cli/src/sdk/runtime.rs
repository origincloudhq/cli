//! What the generated commands share: the SDK clients, resource names from
//! their parts, durations, and the output.

use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::net::IpAddr;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context as _};
use origincloud_core::credentials::{Credentials, TokenFuture};
use origincloud_core::{Client, Operation};
use serde::de::DeserializeOwned;
use serde::Serialize;

use super::credentials::CliSessionCredentials;
use crate::config::ClientProfile;
use crate::confirm::Confirmed;
use crate::handler::tag_args::TagArg;
use crate::identity::{is_resource_id, RESOURCE_ID_RULE};
pub use crate::output::Listing;
use crate::output::{CommandResult, SerializableOutput, SerializationError, Shape};

/// The variables that hold credentials for the SDK. When any is set, the
/// generated commands use them, as every SDK does, instead of the login
/// session.
const CREDENTIAL_VARIABLES: [&str; 3] = [
    "ORIGINCLOUD_ACCESS_TOKEN",
    "ORIGINCLOUD_CLIENT_ID",
    "ORIGINCLOUD_CLIENT_SECRET",
];

/// What the runtime knows of a generated group, which the generator writes
/// in `generated/services.rs`.
pub struct Group {
    /// `pvs`, as in `origin pvs`.
    pub name: &'static str,
    /// The environment variable of its `--endpoint`, its own:
    /// `ORIGINCLOUD_PVS_ENDPOINT`.
    pub endpoint_variable: &'static str,
    /// Whether, with a region, its calls go to the region's host, as the
    /// SDK's clients say, and as its `--endpoint` help says.
    #[allow(dead_code)]
    pub regional: bool,
    /// The endpoint its calls go to through `client`.
    #[allow(dead_code)]
    pub service: fn(&Client) -> origincloud_core::Service,
}

/// The group whose `--endpoint` redirects the zone lookup of a list given no
/// `--zone`.
const LOCATION: &str = "location";

/// What a command runs with: the profile, its group, and the group's
/// endpoint override.
pub struct Context<'a> {
    profile: &'a ClientProfile,
    group: &'static Group,
    /// The base URL every call of the group goes to instead of the
    /// service's own host.
    endpoint: Option<String>,
}

impl<'a> Context<'a> {
    /// The context of a command of `group`, with the endpoint its
    /// `--endpoint`, or its own variable, names, which must be `https://`,
    /// or `http://` to this machine. Using one is said on standard error, so
    /// that it is never forgotten in a shell's environment.
    pub fn new(
        profile: &'a ClientProfile,
        group: &'static Group,
        endpoint: Option<&str>,
    ) -> anyhow::Result<Self> {
        let endpoint = endpoint.map(check_endpoint).transpose()?;
        if let Some(endpoint) = &endpoint {
            eprintln!(
                "Sending the `origin {}` calls to {endpoint}, from --endpoint or {}, instead of \
                 the service's own host.",
                group.name, group.endpoint_variable
            );
        }
        Ok(Self {
            profile,
            group,
            endpoint,
        })
    }

    /// The client of a command that changes nothing irreversibly.
    pub fn client(&self) -> anyhow::Result<ReadClient> {
        Ok(ReadClient(self.build(self.endpoint.as_deref())?))
    }

    /// The client of a delete, or of an irreversible change: only to be had
    /// with the proof that `--yes` was given.
    pub fn confirmed_client(&self, _confirmed: Confirmed) -> anyhow::Result<ConfirmedClient> {
        Ok(ConfirmedClient(self.build(self.endpoint.as_deref())?))
    }

    /// The client of the location service, for the zone lookup of a list
    /// given no `--zone`: not the group's endpoint override, which is for
    /// the group's own service, but the location group's, from its
    /// variable, when one is set.
    fn location_client(&self) -> anyhow::Result<ReadClient> {
        if self.group.name == LOCATION {
            return self.client();
        }
        let variable = super::generated::GROUPS
            .iter()
            .find(|g| g.name == LOCATION)
            .map(|g| g.endpoint_variable);
        let endpoint = variable.and_then(|v| {
            std::env::var(v)
                .ok()
                .filter(|e| !e.is_empty())
                .map(|e| (v, e))
        });
        match endpoint {
            Some((variable, endpoint)) => {
                let endpoint = check_endpoint(&endpoint).with_context(|| variable.to_owned())?;
                eprintln!(
                    "Looking up the zones at {endpoint}, from {variable}, instead of the location \
                     service's own host."
                );
                Ok(ReadClient(self.build(Some(&endpoint))?))
            }
            None => {
                if self.endpoint.is_some() {
                    eprintln!(
                        "Looking up the zones at the location service's own host{}.",
                        variable
                            .map(|v| format!(", which {v} would redirect"))
                            .unwrap_or_default()
                    );
                }
                Ok(ReadClient(self.build(None)?))
            }
        }
    }

    fn build(&self, endpoint: Option<&str>) -> anyhow::Result<Client> {
        let mut builder = Client::builder()
            .user_agent(format!("origin-cli/{}", env!("CARGO_PKG_VERSION")))
            .region(
                self.profile
                    .region_in_effect()
                    .map(|(region, _)| region)
                    .unwrap_or_default(),
            );
        let from_environment = CREDENTIAL_VARIABLES
            .iter()
            .any(|name| std::env::var(name).is_ok_and(|v| !v.is_empty()));
        builder = if from_environment {
            let credentials = origincloud_core::default_credentials().map_err(api_error)?;
            builder.credentials(Shared(credentials))
        } else {
            builder.credentials(CliSessionCredentials::default())
        };
        if let Some(endpoint) = endpoint {
            builder = builder.endpoint(endpoint);
        }
        builder.build().map_err(api_error)
    }

    /// The resource names of a command, from its name-part flags (such as
    /// `--project`), each `None` when not given.
    pub fn names(&self, parts: &[(&'static str, Option<&str>)]) -> Names {
        let profile = |var: &str, value: &Option<String>| {
            value.clone().map(|v| {
                let from = format!(
                    "ORIGINCLOUD_{}, or the profile's default-{var}",
                    var.to_uppercase()
                );
                (var.to_owned(), (v, from))
            })
        };
        let region = self
            .profile
            .region_in_effect()
            .map(|(v, from)| ("region".to_owned(), (v, from.describe().to_owned())));
        Names {
            given: parts
                .iter()
                .filter_map(|(var, value)| value.map(|v| ((*var).to_owned(), v.to_owned())))
                .collect(),
            learned: BTreeMap::new(),
            defaults: [
                profile("project", &self.profile.default_project),
                profile("zone", &self.profile.default_zone),
                region,
            ]
            .into_iter()
            .flatten()
            .collect(),
        }
    }
}

/// The client of `origin mcp serve`'s calls to `service`, made as the
/// group's commands make theirs: the region in effect, the SDK's
/// credentials from the environment or the login session, and the group's
/// endpoint variable, such as `ORIGINCLOUD_PVS_ENDPOINT`. It goes around
/// [`Context::confirmed_client`]: the MCP client's approval of a tool
/// marked destructive stands in for `--yes`, which an agent can't give.
pub(crate) fn mcp_client(profile: &ClientProfile, service: &str) -> anyhow::Result<Client> {
    let group = super::generated::GROUPS
        .iter()
        .copied()
        .find(|g| g.name == service)
        .ok_or_else(|| anyhow!("the CLI has no group for the service {service}"))?;
    let endpoint = std::env::var(group.endpoint_variable)
        .ok()
        .filter(|e| !e.is_empty());
    let ctx = Context::new(profile, group, endpoint.as_deref())
        .with_context(|| group.endpoint_variable.to_owned())?;
    ctx.build(ctx.endpoint.as_deref())
}

/// `endpoint`, if it is an `https://` URL, or an `http://` one to this
/// machine.
fn check_endpoint(endpoint: &str) -> anyhow::Result<String> {
    let url = url::Url::parse(endpoint)
        .ok()
        .filter(|u| u.has_host())
        .ok_or_else(|| anyhow!("--endpoint {endpoint:?} isn't an absolute URL"))?;
    let loopback = match url.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => IpAddr::V4(ip).is_loopback(),
        Some(url::Host::Ipv6(ip)) => IpAddr::V6(ip).is_loopback(),
        None => false,
    };
    match url.scheme() {
        "https" => Ok(endpoint.to_owned()),
        "http" if loopback => Ok(endpoint.to_owned()),
        "http" => bail!(
            "--endpoint {endpoint}: http:// is only for this machine (localhost, 127.0.0.1, [::1]); use https://"
        ),
        other => bail!("--endpoint {endpoint}: {other}:// isn't supported; use https://"),
    }
}

/// Credentials the SDK found in the environment.
#[derive(Debug)]
struct Shared(Arc<dyn Credentials>);

impl Credentials for Shared {
    fn token(&self) -> TokenFuture<'_> {
        self.0.token()
    }
}

/// The client every call that changes nothing irreversibly goes through.
pub struct ReadClient(Client);

impl ReadClient {
    pub(super) fn sdk(&self) -> &Client {
        &self.0
    }
}

/// The client deletes and irreversible changes go through, which only
/// [`Context::confirmed_client`] makes, from a [`Confirmed`].
pub struct ConfirmedClient(Client);

impl ConfirmedClient {
    pub(super) fn sdk(&self) -> &Client {
        &self.0
    }
}

/// The zones that offer `service`, by id, in catalog order, from the
/// location service.
pub async fn zones(ctx: &Context<'_>, service: &str) -> anyhow::Result<Vec<String>> {
    use origincloud_location::v1beta1::{ListZonesRequest, LocationClient};
    let client = ctx.location_client()?;
    let listed = LocationClient::new(client.sdk())
        .list_zones(ListZonesRequest::default())
        .await
        .map_err(api_error)
        .context("listing the zones")?;
    Ok(listed
        .zones
        .into_iter()
        .filter(|z| z.services.iter().any(|s| s == service))
        .filter_map(|z| z.name.rsplit('/').next().map(str::to_owned))
        .collect())
}

/// Resource names from their parts. A part comes from its flag
/// (`--project`), else from a whole name given to another flag, else from
/// the environment or the profile.
#[derive(Clone)]
pub struct Names {
    given: BTreeMap<String, String>,
    /// Parts learned from whole names, with the flag and value each came
    /// from.
    learned: BTreeMap<String, (String, String)>,
    /// The defaults of `project`, `zone` and `region`, with where each came
    /// from.
    defaults: BTreeMap<String, (String, String)>,
}

/// The flag of a name part: `--project`.
fn flag(var: &str) -> String {
    format!("--{}", var.replace('_', "-"))
}

/// The variable of a pattern segment, `{project}`, or `None` for a literal.
fn variable(segment: &str) -> Option<&str> {
    segment.strip_prefix('{').and_then(|s| s.strip_suffix('}'))
}

/// `value`, given to `source`, if it can be one segment of a name.
fn checked(source: &str, value: &str) -> anyhow::Result<String> {
    if is_resource_id(value) {
        Ok(value.to_owned())
    } else {
        bail!("{source}: {value:?} isn't an id: {RESOURCE_ID_RULE}")
    }
}

impl Names {
    /// The names with the part `var` set to `value`, as if given by its flag.
    pub fn with_part(&self, var: &str, value: &str) -> Names {
        let mut names = self.clone();
        names.given.insert(var.to_owned(), value.to_owned());
        names
    }

    /// Checks the parts of `pattern` other than `var` are at hand, before
    /// `var` is known.
    pub fn check_without(&self, pattern: &str, var: &str) -> anyhow::Result<()> {
        self.with_part(var, "placeholder").name(pattern).map(drop)
    }

    /// Learns the parts of `value`, given to the flag `--{leaf_flag}`, when
    /// it is a whole name of one of `patterns`, the name patterns of the
    /// resource the flag refers to. A part that disagrees with its flag, or
    /// with another whole name, is refused.
    pub fn learn(
        &mut self,
        patterns: &[&str],
        leaf_flag: &str,
        value: Option<&str>,
    ) -> anyhow::Result<()> {
        let Some(value) = value.filter(|v| v.contains('/')) else {
            return Ok(());
        };
        let (pattern, parts) =
            parse_any(patterns, value).map_err(|e| anyhow!("--{leaf_flag}: {e}"))?;
        for (var, part) in parts.into_iter().take(variables(pattern).len() - 1) {
            if let Some(given) = self.given.get(var.as_str()) {
                if *given != part {
                    bail!(
                        "{} {given} differs from the {var} of --{leaf_flag} {value}",
                        flag(&var)
                    );
                }
            }
            if let Some((other, from)) = self.learned.get(&var) {
                if *other != part {
                    bail!("the {var} of --{leaf_flag} {value} differs from the {var} of {from}");
                }
            }
            self.learned
                .insert(var, (part, format!("--{leaf_flag} {value}")));
        }
        Ok(())
    }

    /// The name of `pattern`, every part from its flag.
    pub fn name(&self, pattern: &str) -> anyhow::Result<String> {
        self.compose(pattern, None)
    }

    /// The name `value`, given to `--{leaf_flag}`, names, of a resource with
    /// `patterns`: a whole name of any of them, taken as it is once checked,
    /// or the resource's id, completed into the first with the other parts'
    /// flags.
    pub fn reference(
        &self,
        patterns: &[&str],
        leaf_flag: &str,
        value: &str,
    ) -> anyhow::Result<String> {
        if value.contains('/') {
            parse_any(patterns, value).map_err(|e| anyhow!("--{leaf_flag}: {e}"))?;
            return Ok(value.to_owned());
        }
        let pattern = patterns
            .first()
            .ok_or_else(|| anyhow!("--{leaf_flag}: its resource has no name pattern"))?;
        self.compose(pattern, Some((leaf_flag, value)))
    }

    /// The name of `pattern`, its last part the id `leaf` (`(flag, value)`)
    /// when given, and the others from their flags.
    fn compose(&self, pattern: &str, leaf: Option<(&str, &str)>) -> anyhow::Result<String> {
        let segments: Vec<&str> = pattern.split('/').collect();
        let last = segments.iter().rposition(|s| variable(s).is_some());
        let mut out = Vec::with_capacity(segments.len());
        for (i, segment) in segments.iter().enumerate() {
            let Some(var) = variable(segment) else {
                out.push((*segment).to_owned());
                continue;
            };
            match leaf {
                Some((leaf_flag, value)) if Some(i) == last => {
                    out.push(checked(&format!("--{leaf_flag}"), value)?);
                }
                _ => out.push(self.part(var)?),
            }
        }
        Ok(out.join("/"))
    }

    fn part(&self, var: &str) -> anyhow::Result<String> {
        if let Some(value) = self.given.get(var) {
            return checked(&flag(var), value);
        }
        if let Some((value, _)) = self.learned.get(var) {
            return Ok(value.clone());
        }
        if let Some((value, from)) = self.defaults.get(var) {
            return checked(from, value);
        }
        let key = match var {
            "project" | "zone" => format!("default-{var}"),
            "region" => "region".to_owned(),
            _ => bail!("{} is required", flag(var)),
        };
        bail!(
            "{} is required: give it, set ORIGINCLOUD_{}, or set a default with \
             `origin config profile-set {key} <{var}>`",
            flag(var),
            var.to_uppercase()
        )
    }
}

fn variables(pattern: &str) -> Vec<&str> {
    pattern.split('/').filter_map(variable).collect()
}

/// The parts of `name`, a whole name of `pattern`, by variable, or `None`
/// when it isn't one. The SDK's check of a name against its path template
/// decides: each literal segment as the pattern has it, and each part one
/// segment, not empty, `.` or `..`.
fn parse(pattern: &str, name: &str) -> Option<Vec<(String, String)>> {
    let template: Vec<&str> = pattern
        .split('/')
        .map(|s| if variable(s).is_some() { "*" } else { s })
        .collect();
    origincloud_core::path_value("name", name, &template.join("/")).ok()?;
    Some(
        pattern
            .split('/')
            .zip(name.split('/'))
            .filter_map(|(p, n)| variable(p).map(|var| (var.to_owned(), n.to_owned())))
            .collect(),
    )
}

/// The pattern of `patterns` that `name` is a whole name of, the first that
/// fits, and its parts by variable.
fn parse_any<'p>(
    patterns: &[&'p str],
    name: &str,
) -> anyhow::Result<(&'p str, Vec<(String, String)>)> {
    patterns
        .iter()
        .find_map(|pattern| parse(pattern, name).map(|parts| (*pattern, parts)))
        .ok_or_else(|| {
            let forms = match patterns {
                [] => String::new(),
                [only] => (*only).to_owned(),
                [first @ .., last] => format!("{} or {last}", first.join(", ")),
            };
            anyhow!("{name:?} isn't a name of the form {forms}")
        })
}

/// A duration flag's value: whole numbers of hours, minutes, seconds and
/// milliseconds, largest first, such as `30s`, `2m`, `1m30s` or `500ms`.
pub fn duration(value: &str) -> Result<origincloud_core::protobuf::Duration, String> {
    let invalid = || format!("{value:?} isn't a duration: expected one such as 30s, 2m or 1m30s");
    let mut total = std::time::Duration::ZERO;
    let mut rest = value;
    // The units in the order they may appear, each at most once.
    let mut units: &[(&str, u64)] = &[("h", 3_600_000), ("m", 60_000), ("s", 1_000), ("ms", 1)];
    while !rest.is_empty() {
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return Err(invalid());
        }
        let number: u64 = rest[..digits].parse().map_err(|_| invalid())?;
        let after = &rest[digits..];
        let letters = after.bytes().take_while(u8::is_ascii_alphabetic).count();
        let unit = &after[..letters];
        let at = units
            .iter()
            .position(|(u, _)| *u == unit)
            .ok_or_else(invalid)?;
        let millis = number.checked_mul(units[at].1).ok_or_else(invalid)?;
        total = total
            .checked_add(std::time::Duration::from_millis(millis))
            .ok_or_else(invalid)?;
        units = &units[at + 1..];
        rest = &after[letters..];
    }
    if value.is_empty() {
        return Err(invalid());
    }
    Ok(total.into())
}

// What the flags of nested messages, JSON values and timestamps use.
#[allow(unused_imports)]
pub use super::nested::{
    json_arg, key_bool, key_duration, key_enum, key_scalar, key_timestamp, key_values,
    notice_replaced, notice_reset, required_keys, timestamp, unique, unknown_key, JsonArg, Keyed,
};

/// `--tag KEY=VALUE` values as a map field.
pub fn tags(tags: &[TagArg]) -> BTreeMap<String, String> {
    tags.iter()
        .map(|t| (t.key.clone(), t.value.clone()))
        .collect()
}

/// Whether `mask` names one of `paths`, or everything.
pub fn names_any(mask: Option<&origincloud_core::protobuf::FieldMask>, paths: &[&str]) -> bool {
    mask.is_some_and(|m| {
        m.paths
            .iter()
            .any(|p| p == "*" || paths.contains(&p.as_str()))
    })
}

/// A response printed as its JSON mapping, and what it is for the human
/// formats.
struct Json(serde_json::Value, Shape);

impl SerializableOutput for Json {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError> {
        Ok(self.0.clone())
    }

    fn shape(&self) -> Shape {
        self.1.clone()
    }
}

/// An operation a command waited for, printed as the API sends it, with
/// exit status 1 when it failed or isn't done.
struct Waited {
    json: serde_json::Value,
    succeeded: bool,
}

impl SerializableOutput for Waited {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError> {
        Ok(self.json.clone())
    }

    fn shape(&self) -> Shape {
        Shape::Message(OPERATION)
    }

    fn exit_code(&self) -> i32 {
        if self.succeeded {
            0
        } else {
            1
        }
    }
}

/// `operation`, which a command waited for: printed whatever its outcome,
/// the command failing unless it is done without an error.
pub fn waited(operation: &origincloud_core::longrunning::Operation) -> Box<dyn SerializableOutput> {
    use origincloud_core::longrunning::OperationResult;
    let succeeded = operation.done && !matches!(operation.result, Some(OperationResult::Error(_)));
    Box::new(Waited {
        json: serde_json::to_value(operation).unwrap_or(serde_json::Value::Null),
        succeeded,
    })
}

/// The most one wait call asks the API to wait: a minute, the most it waits.
const MOST_PER_CALL: std::time::Duration = std::time::Duration::from_secs(60);

/// The least time between two wait calls, should the API answer one early
/// with the operation not done.
const LEAST_BETWEEN_CALLS: std::time::Duration = std::time::Duration::from_secs(1);

/// When `--timeout`, a deadline of the command's, passes.
pub fn deadline_after(
    timeout: origincloud_core::protobuf::Duration,
) -> anyhow::Result<std::time::Instant> {
    let timeout = std::time::Duration::try_from(timeout)
        .ok()
        .filter(|t| !t.is_zero())
        .ok_or_else(|| anyhow!("--timeout must be longer than zero"))?;
    std::time::Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| anyhow!("--timeout is too long"))
}

/// Whether `deadline` has passed.
pub fn passed(deadline: std::time::Instant) -> bool {
    std::time::Instant::now() >= deadline
}

/// How long the next wait call asks the API to wait: until `deadline`, in
/// whole seconds rounded up, at least one and at most a minute.
pub fn wait_for(deadline: std::time::Instant) -> origincloud_core::protobuf::Duration {
    let left = deadline.saturating_duration_since(std::time::Instant::now());
    let seconds = left.as_secs() + u64::from(left.subsec_nanos() > 0);
    std::time::Duration::from_secs(seconds.clamp(1, MOST_PER_CALL.as_secs())).into()
}

/// Sleeps, before the next wait call, until a second after `started`, the
/// last call's start, without going past `deadline`: an API answering early
/// isn't called in a tight loop.
pub async fn pace(started: std::time::Instant, deadline: Option<std::time::Instant>) {
    let mut until = started + LEAST_BETWEEN_CALLS;
    if let Some(deadline) = deadline {
        until = until.min(deadline);
    }
    tokio::time::sleep_until(until.into()).await;
}

/// The message an operation is, whose JSON holds the message of its result
/// or its metadata by `@type`.
pub const OPERATION: &str = "google.longrunning.Operation";

/// `value`, the API's message `message` (in full:
/// `origincloud.pvs.v1beta1.Volume`), in its proto3 JSON mapping, as the
/// API sends it; the human formats print its fields by their kinds.
pub fn json(
    value: &impl Serialize,
    message: &'static str,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    Ok(Box::new(Json(
        serde_json::to_value(value)?,
        Shape::Message(message),
    )))
}

/// A list response in its JSON mapping, with its list, `listing`'s
/// `field`, as `[]` when it is empty, rather than left out as the mapping
/// leaves out every empty field: `{"volumes": []}`. The human formats
/// print its items as `listing` says, whatever else the response holds.
pub fn list_json(
    value: &impl Serialize,
    listing: Listing,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let mut value = serde_json::to_value(value)?;
    if let Some(object) = value.as_object_mut() {
        object
            .entry(listing.field)
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    }
    Ok(Box::new(Json(value, Shape::List(listing))))
}

/// The result of a command whose method has none and isn't a delete, of
/// which the API has none today: the generator writes it for one it gains.
#[allow(dead_code)]
pub fn success() -> Box<dyn SerializableOutput> {
    Box::new(CommandResult::Success)
}

/// The result of a delete of the `what` (`volume`) named `name`: in
/// `json`, the success envelope every command without a result prints; in
/// the human formats, `Deleted volume v1.`
pub fn deleted(what: &str, name: &str) -> Box<dyn SerializableOutput> {
    let id = name.rsplit('/').next().unwrap_or(name);
    Box::new(Done(format!("Deleted {what} {id}.")))
}

/// A command without a result, done, and what it did.
struct Done(String);

impl SerializableOutput for Done {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError> {
        CommandResult::Success.to_json()
    }

    fn shape(&self) -> Shape {
        Shape::Done(self.0.clone())
    }
}

/// Waits for `operation` to finish, and returns its result.
pub async fn wait<T: DeserializeOwned>(operation: &mut Operation<T>) -> anyhow::Result<T> {
    if !operation.done() {
        waiting(operation.name());
    }
    operation.wait().await.map_err(api_error)
}

/// Says on standard error, when someone is watching it, that the command
/// waits for the operation `name`.
pub fn waiting(name: &str) {
    if std::io::stderr().is_terminal() {
        eprintln!("Waiting for {name} (Ctrl-C stops waiting, not the operation)...");
    }
}

/// Says `notice` on standard error when `value`, a field of the response
/// holding a secret the API returns this once (`commands.toml`'s
/// `shown_once`), isn't empty: the output is its only copy.
pub fn shown_once(value: &str, notice: &str) {
    if !value.is_empty() {
        eprintln!("{notice}");
    }
}

/// An SDK error for the user: the API's own message and code, the request
/// id to quote to support, and each refused field.
pub fn api_error(err: origincloud_core::Error) -> anyhow::Error {
    let rendered = err.to_string();
    let mut message = rendered
        .strip_prefix("origincloud: ")
        .unwrap_or(&rendered)
        .to_owned();
    if let Some(api) = err.api() {
        for violation in &api.field_violations {
            if !message.contains(&violation.description) {
                message.push_str(&format!(
                    "\n  {}: {}",
                    violation.field, violation.description
                ));
            }
        }
    }
    anyhow!(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(given: &[(&'static str, Option<&'static str>)]) -> Names {
        let mut profile = ClientProfile::default();
        profile.default_project = Some("default-project".to_owned());
        names_with(profile, given)
    }

    fn names_with(profile: ClientProfile, given: &[(&'static str, Option<&'static str>)]) -> Names {
        let profile = Box::leak(Box::new(profile));
        Context::new(profile, &super::super::generated::PVS, None)
            .unwrap()
            .names(given)
    }

    const VOLUME: &str = "projects/{project}/zones/{zone}/volumes/{volume}";
    const ZONE: &str = "projects/{project}/zones/{zone}";

    #[test]
    fn a_name_is_made_of_its_flags_and_the_profile() {
        let n = names(&[("project", None), ("zone", Some("z1"))]);
        assert_eq!(
            n.reference(&[VOLUME], "volume", "v1").unwrap(),
            "projects/default-project/zones/z1/volumes/v1"
        );
        assert_eq!(n.name(ZONE).unwrap(), "projects/default-project/zones/z1");
        assert_eq!(
            n.with_part("zone", "z2").name(ZONE).unwrap(),
            "projects/default-project/zones/z2"
        );
        assert_eq!(
            n.reference(&["storageClasses/{storage_class}"], "storage-class", "gp1")
                .unwrap(),
            "storageClasses/gp1"
        );
    }

    #[test]
    fn a_missing_part_names_its_flag_and_the_profile_key() {
        let n = names(&[("project", Some("p")), ("zone", None)]);
        let err = n.name(ZONE).unwrap_err().to_string();
        assert!(err.contains("--zone is required"), "{err}");
        assert!(err.contains("ORIGINCLOUD_ZONE"), "{err}");
        assert!(
            err.contains("`origin config profile-set default-zone <zone>`"),
            "{err}"
        );
        assert!(n.check_without(ZONE, "zone").is_ok());
        let n = names(&[("project", Some("a/b")), ("zone", None)]);
        assert!(n.check_without(ZONE, "zone").is_err());
    }

    const REGIONAL: &str = "projects/{project}/regions/{region}";

    /// `--region` defaults to the region in effect: the profile's region,
    /// else the region of its default zone; without either, the refusal
    /// names the flag, the variable and the command setting a default.
    #[test]
    fn a_missing_region_is_the_profiles_or_its_zones() {
        let mut profile = ClientProfile::default();
        profile.default_project = Some("p".to_owned());
        let given = [("project", None), ("region", None)];
        let err = names_with(profile.clone(), &given)
            .name(REGIONAL)
            .unwrap_err()
            .to_string();
        for part in [
            "--region is required",
            "ORIGINCLOUD_REGION",
            "`origin config profile-set region <region>`",
        ] {
            assert!(err.contains(part), "{part}: {err}");
        }

        profile.default_zone = Some("eu-central1-az1".to_owned());
        let n = names_with(profile.clone(), &given);
        assert_eq!(n.name(REGIONAL).unwrap(), "projects/p/regions/eu-central1");
        profile.region = Some("eu-central2".to_owned());
        let n = names_with(profile.clone(), &given);
        assert_eq!(n.name(REGIONAL).unwrap(), "projects/p/regions/eu-central2");
        let n = names_with(
            profile.clone(),
            &[("project", None), ("region", Some("r9"))],
        );
        assert_eq!(n.name(REGIONAL).unwrap(), "projects/p/regions/r9");

        profile.region = Some("a/b".to_owned());
        let err = names_with(profile, &given)
            .name(REGIONAL)
            .unwrap_err()
            .to_string();
        assert!(
            err.starts_with("ORIGINCLOUD_REGION, or the profile's region:"),
            "{err}"
        );
    }

    #[test]
    fn a_whole_name_fills_in_the_other_parts() {
        let mut n = names(&[("project", None), ("zone", None)]);
        n.learn(
            &[VOLUME],
            "source-volume",
            Some("projects/p/zones/z/volumes/v"),
        )
        .unwrap();
        assert_eq!(n.name(ZONE).unwrap(), "projects/p/zones/z");
        assert_eq!(
            n.reference(&[VOLUME], "source-volume", "projects/p/zones/z/volumes/v")
                .unwrap(),
            "projects/p/zones/z/volumes/v"
        );
    }

    #[test]
    fn a_whole_name_disagreeing_with_a_flag_is_refused() {
        let mut n = names(&[("project", Some("other")), ("zone", None)]);
        let err = n
            .learn(&[VOLUME], "volume", Some("projects/p/zones/z/volumes/v"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("--project other differs"), "{err}");
    }

    #[test]
    fn a_name_of_the_wrong_form_is_refused() {
        let n = names(&[("zone", Some("z"))]);
        for bad in [
            "projects/p/volumes/v",
            "projects/p/zones/z/snapshots/s",
            "a/b",
            "projects/p/zones/z/volumes/..",
            "projects/../zones/z/volumes/v",
            "projects/p/zones/./volumes/v",
            "projects/p/zones/z/volumes/",
        ] {
            assert!(n.reference(&[VOLUME], "volume", bad).is_err(), "{bad}");
        }
    }

    /// No part can climb out of its place in the path: `.`, `..`, an empty
    /// part or a `/` is refused, from a flag, a whole name or the profile.
    #[test]
    fn a_part_that_could_leave_its_place_is_refused() {
        let n = names(&[("zone", Some("z"))]);
        for bad in ["..", ".", ""] {
            let err = n
                .reference(&[VOLUME], "volume", bad)
                .unwrap_err()
                .to_string();
            assert!(err.starts_with("--volume:"), "{bad:?}: {err}");
            assert!(err.contains("isn't an id"), "{bad:?}: {err}");
        }
        let err = names(&[("project", Some("a/b")), ("zone", Some("z"))])
            .name(ZONE)
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("--project:"), "{err}");
        let err = names(&[("project", Some("..")), ("zone", Some("z"))])
            .name(ZONE)
            .unwrap_err()
            .to_string();
        assert!(err.contains("isn't an id"), "{err}");
        let mut profile = ClientProfile::default();
        profile.default_project = Some(".".to_owned());
        let err = names_with(profile, &[("project", None), ("zone", Some("z"))])
            .name(ZONE)
            .unwrap_err()
            .to_string();
        assert!(err.contains("isn't an id"), "{err}");
    }

    const INSTANCE_TYPE: &str = "instanceTypes/{instance_type}";

    /// An id is what the SDK sends as one segment, whatever else it holds:
    /// an instance type's dots, bare or in a whole name, and characters
    /// the SDK escapes.
    #[test]
    fn an_id_may_hold_dots() {
        let mut n = names(&[("project", None), ("zone", Some("z"))]);
        for (value, name) in [
            ("gp1.core.2", "instanceTypes/gp1.core.2"),
            ("instanceTypes/gp1.core.2", "instanceTypes/gp1.core.2"),
            ("a..b", "instanceTypes/a..b"),
        ] {
            n.learn(&[INSTANCE_TYPE], "instance-type", Some(value))
                .unwrap();
            assert_eq!(
                n.reference(&[INSTANCE_TYPE], "instance-type", value)
                    .unwrap(),
                name
            );
        }
        for (value, name) in [
            ("v.1", "projects/default-project/zones/z/volumes/v.1"),
            ("v 1", "projects/default-project/zones/z/volumes/v 1"),
            (
                "projects/p.1/zones/z.1/volumes/v.1",
                "projects/p.1/zones/z.1/volumes/v.1",
            ),
        ] {
            assert_eq!(n.reference(&[VOLUME], "volume", value).unwrap(), name);
        }
        assert_eq!(
            names(&[("project", Some("p.1")), ("zone", Some("z.1"))])
                .name(ZONE)
                .unwrap(),
            "projects/p.1/zones/z.1"
        );
        for bad in [
            "instanceTypes/..",
            "instanceTypes/.",
            "instanceTypes/",
            "instanceTypes/gp1.core.2/x",
            "instancetypes/gp1.core.2",
        ] {
            let err = n
                .reference(&[INSTANCE_TYPE], "instance-type", bad)
                .unwrap_err()
                .to_string();
            assert!(err.contains("isn't a name of the form"), "{bad}: {err}");
        }
    }

    /// An image: the project's own, or a public one.
    const IMAGE: [&str; 2] = [
        "projects/{project}/zones/{zone}/images/{image}",
        "zones/{zone}/images/{image}",
    ];

    /// A reference whose resource has several name patterns takes a whole
    /// name of each, learning its parts; an id completes to the first.
    #[test]
    fn a_reference_takes_a_whole_name_of_each_pattern() {
        let mut n = names(&[("project", None), ("zone", None)]);
        n.learn(&IMAGE, "image", Some("zones/z/images/img-1"))
            .unwrap();
        assert_eq!(n.name(ZONE).unwrap(), "projects/default-project/zones/z");
        assert_eq!(
            n.reference(&IMAGE, "image", "zones/z/images/img-1")
                .unwrap(),
            "zones/z/images/img-1"
        );

        let mut n = names(&[("project", None), ("zone", None)]);
        n.learn(&IMAGE, "image", Some("projects/p/zones/z/images/img-1"))
            .unwrap();
        assert_eq!(n.name(ZONE).unwrap(), "projects/p/zones/z");
        assert_eq!(
            n.reference(&IMAGE, "image", "projects/p/zones/z/images/img-1")
                .unwrap(),
            "projects/p/zones/z/images/img-1"
        );

        // An id is the project's own image.
        let n = names(&[("project", None), ("zone", Some("z"))]);
        assert_eq!(
            n.reference(&IMAGE, "image", "img-1").unwrap(),
            "projects/default-project/zones/z/images/img-1"
        );

        // A public image's zone is checked against --zone too.
        let mut n = names(&[("project", None), ("zone", Some("other"))]);
        let err = n
            .learn(&IMAGE, "image", Some("zones/z/images/img-1"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("--zone other differs"), "{err}");

        // A name of neither form names both in its refusal.
        let n = names(&[("zone", Some("z"))]);
        for bad in [
            "images/img-1",
            "zones/z/images/..",
            "zones/../images/img-1",
            "projects/p/zones/z/snapshots/s",
        ] {
            let err = n.reference(&IMAGE, "image", bad).unwrap_err().to_string();
            assert!(
                err.contains(
                    "isn't a name of the form projects/{project}/zones/{zone}/images/{image} \
                     or zones/{zone}/images/{image}"
                ),
                "{bad}: {err}"
            );
        }
    }

    #[test]
    fn an_endpoint_is_https_or_this_machine() {
        for ok in [
            "https://pvs.example.com",
            "http://127.0.0.1:8080",
            "http://localhost:1",
            "http://[::1]:9",
        ] {
            assert!(check_endpoint(ok).is_ok(), "{ok}");
        }
        for bad in [
            "http://192.0.2.1:80",
            "http://pvs.example.com",
            "ftp://x",
            "nope",
        ] {
            assert!(check_endpoint(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn an_empty_list_prints_as_an_empty_array() {
        let empty = origincloud_pvs::v1beta1::ListVolumesResponse::default();
        let listing = Listing {
            field: "volumes",
            ..Listing::default()
        };
        let printed = list_json(&empty, listing).unwrap().to_json().unwrap();
        assert_eq!(printed, serde_json::json!({ "volumes": [] }));
    }

    #[test]
    fn a_duration_is_hours_minutes_seconds_and_milliseconds() {
        let seconds = |v: &str| {
            let d = duration(v).unwrap();
            (d.seconds, d.nanos)
        };
        assert_eq!(seconds("30s"), (30, 0));
        assert_eq!(seconds("2m"), (120, 0));
        assert_eq!(seconds("1m30s"), (90, 0));
        assert_eq!(seconds("1h"), (3600, 0));
        assert_eq!(seconds("500ms"), (0, 500_000_000));
        assert_eq!(seconds("1s500ms"), (1, 500_000_000));
        assert_eq!(seconds("0s"), (0, 0));
        for bad in [
            "",
            "30",
            "s",
            "1.5s",
            "30s1m",
            "1m1m",
            "-1s",
            "1 s",
            "1d",
            "18446744073709551616s",
        ] {
            assert!(duration(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_wait_call_asks_for_what_is_left_of_the_deadline_up_to_a_minute() {
        use std::time::{Duration, Instant};
        let secs = |d: origincloud_core::protobuf::Duration| d.seconds;
        assert_eq!(secs(wait_for(Instant::now() + Duration::from_secs(90))), 60);
        assert_eq!(
            secs(wait_for(Instant::now() + Duration::from_millis(2500))),
            3
        );
        assert_eq!(secs(wait_for(Instant::now())), 1);
        assert!(passed(Instant::now()));
        assert!(!passed(
            deadline_after(Duration::from_secs(5).into()).unwrap()
        ));
        assert!(deadline_after(Duration::ZERO.into()).is_err());
    }

    #[test]
    fn a_waited_operation_fails_the_command_unless_done_without_an_error() {
        let read = |json: &str| -> origincloud_core::longrunning::Operation {
            serde_json::from_str(json).unwrap()
        };
        let done = read(
            r#"{"name":"o","done":true,"response":{"@type":"type.googleapis.com/google.protobuf.Empty"}}"#,
        );
        let failed = read(r#"{"name":"o","done":true,"error":{"code":9,"message":"no"}}"#);
        let running = read(r#"{"name":"o"}"#);
        assert_eq!(waited(&done).exit_code(), 0);
        assert_eq!(waited(&failed).exit_code(), 1);
        assert_eq!(waited(&failed).to_json().unwrap()["error"]["message"], "no");
        assert_eq!(waited(&running).exit_code(), 1);
    }

    /// The help of each group's `--endpoint` says what the SDK does: a
    /// regional group's calls go to the region's host.
    #[test]
    fn a_groups_regional_host_is_the_sdks() {
        let client = Client::builder()
            .region("r9")
            .credentials(origincloud_core::AccessToken::new("t"))
            .build()
            .unwrap();
        for group in super::super::generated::GROUPS {
            let url = (group.service)(&client);
            let host = url.base_url().unwrap().host_str().unwrap().to_owned();
            let regional = host == format!("{}.r9.origincloudapis.eu", group.name);
            assert_eq!(regional, group.regional, "{}: {host}", group.name);
            assert_eq!(
                group.endpoint_variable,
                format!("ORIGINCLOUD_{}_ENDPOINT", group.name.to_uppercase())
            );
        }
    }

    #[test]
    fn a_mask_naming_a_one_way_field() {
        use origincloud_core::protobuf::FieldMask;
        let mask = |p: &[&str]| FieldMask {
            paths: p.iter().map(|s| (*s).to_owned()).collect(),
        };
        assert!(names_any(Some(&mask(&["size_gib"])), &["size_gib"]));
        assert!(names_any(Some(&mask(&["*"])), &["size_gib"]));
        assert!(!names_any(Some(&mask(&["tags"])), &["size_gib"]));
        assert!(!names_any(None, &["size_gib"]));
    }
}
