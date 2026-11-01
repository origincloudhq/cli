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
use crate::identity::is_resource_id;
use crate::output::{CommandResult, SerializableOutput, SerializationError};

/// The variables that hold credentials for the SDK. When any is set, the
/// generated commands use them, as every SDK does, instead of the login
/// session.
const CREDENTIAL_VARIABLES: [&str; 3] = [
    "ORIGINCLOUD_ACCESS_TOKEN",
    "ORIGINCLOUD_CLIENT_ID",
    "ORIGINCLOUD_CLIENT_SECRET",
];

/// What a command runs with: the profile, and the endpoint override.
pub struct Context<'a> {
    profile: &'a ClientProfile,
    /// The base URL every call goes to instead of the service's own host.
    endpoint: Option<String>,
}

impl<'a> Context<'a> {
    /// The context of a command, with the endpoint `--endpoint` names, which
    /// must be `https://`, or `http://` to this machine. Using one is said on
    /// standard error, so that it is never forgotten in a shell's
    /// environment.
    pub fn new(profile: &'a ClientProfile, endpoint: Option<&str>) -> anyhow::Result<Self> {
        let endpoint = endpoint.map(check_endpoint).transpose()?;
        if let Some(endpoint) = &endpoint {
            eprintln!("Sending the calls to {endpoint} instead of the service's own host.");
        }
        Ok(Self { profile, endpoint })
    }

    /// The client of a command that changes nothing irreversibly.
    pub fn client(&self) -> anyhow::Result<ReadClient> {
        Ok(ReadClient(self.build()?))
    }

    /// The client of a delete, or of an irreversible change: only to be had
    /// with the proof that `--yes` was given.
    pub fn confirmed_client(&self, _confirmed: Confirmed) -> anyhow::Result<ConfirmedClient> {
        Ok(ConfirmedClient(self.build()?))
    }

    fn build(&self) -> anyhow::Result<Client> {
        let mut builder = Client::builder()
            .user_agent(format!("origin-cli/{}", env!("CARGO_PKG_VERSION")))
            .region(self.profile.region.clone().unwrap_or_default());
        let from_environment = CREDENTIAL_VARIABLES
            .iter()
            .any(|name| std::env::var(name).is_ok_and(|v| !v.is_empty()));
        builder = if from_environment {
            let credentials = origincloud_core::default_credentials().map_err(api_error)?;
            builder.credentials(Shared(credentials))
        } else {
            builder.credentials(CliSessionCredentials::default())
        };
        if let Some(endpoint) = &self.endpoint {
            builder = builder.endpoint(endpoint);
        }
        builder.build().map_err(api_error)
    }

    /// The resource names of a command, from its name-part flags (such as
    /// `--project`), each `None` when not given.
    pub fn names(&self, parts: &[(&'static str, Option<&str>)]) -> Names {
        let defaults = [
            ("project", &self.profile.default_project),
            ("zone", &self.profile.default_zone),
        ];
        Names {
            given: parts
                .iter()
                .filter_map(|(var, value)| value.map(|v| ((*var).to_owned(), v.to_owned())))
                .collect(),
            learned: BTreeMap::new(),
            defaults: defaults
                .into_iter()
                .filter_map(|(var, value)| value.clone().map(|v| (var.to_owned(), v)))
                .collect(),
        }
    }
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

/// The zones that offer `service`, by id, in catalog order.
pub async fn zones(client: &ReadClient, service: &str) -> anyhow::Result<Vec<String>> {
    use origincloud_location::v1beta1::{ListZonesRequest, LocationClient};
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
/// the profile.
#[derive(Clone)]
pub struct Names {
    given: BTreeMap<String, String>,
    /// Parts learned from whole names, with the flag and value each came
    /// from.
    learned: BTreeMap<String, (String, String)>,
    defaults: BTreeMap<String, String>,
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
        bail!("{source}: {value:?} isn't an id: expected letters, digits, `-` and `_`")
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
        self.with_part(var, "placeholder")
            .name(pattern, None)
            .map(drop)
    }

    /// Learns the parts of `value`, given to the flag `--{leaf_flag}`, when
    /// it is a whole name of `pattern`. A part that disagrees with its flag,
    /// or with another whole name, is refused.
    pub fn learn(
        &mut self,
        pattern: &str,
        leaf_flag: &str,
        value: Option<&str>,
    ) -> anyhow::Result<()> {
        let Some(value) = value.filter(|v| v.contains('/')) else {
            return Ok(());
        };
        let parts = parse(pattern, value).map_err(|e| anyhow!("--{leaf_flag}: {e}"))?;
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

    /// The name of `pattern`, its last part from `leaf` (`(flag, value)`)
    /// when the pattern names a resource, and the others from their flags.
    /// A leaf value that is a whole name is taken as it is, once checked.
    pub fn name(&self, pattern: &str, leaf: Option<(&str, &str)>) -> anyhow::Result<String> {
        if let Some((leaf_flag, value)) = leaf {
            if value.contains('/') {
                parse(pattern, value).map_err(|e| anyhow!("--{leaf_flag}: {e}"))?;
                return Ok(value.to_owned());
            }
        }
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
        if let Some(value) = self.defaults.get(var) {
            return checked(
                &format!(
                    "the profile's default-{var}, or ORIGINCLOUD_{}",
                    var.to_uppercase()
                ),
                value,
            );
        }
        let hint = match var {
            "project" | "zone" => {
                format!(", or set a default with `origin config profile-set default-{var} <{var}>`")
            }
            _ => String::new(),
        };
        bail!("{} is required{hint}", flag(var))
    }
}

fn variables(pattern: &str) -> Vec<&str> {
    pattern.split('/').filter_map(variable).collect()
}

/// The parts of `name`, a whole name of `pattern`, by variable. Each part
/// must be an id: no `.`, `..` or empty segment gets through.
fn parse(pattern: &str, name: &str) -> anyhow::Result<Vec<(String, String)>> {
    let expected: Vec<&str> = pattern.split('/').collect();
    let got: Vec<&str> = name.split('/').collect();
    let refused = || anyhow!("{name:?} isn't a name of the form {pattern}");
    if expected.len() != got.len() {
        return Err(refused());
    }
    let mut parts = Vec::new();
    for (e, g) in expected.iter().zip(&got) {
        match variable(e) {
            Some(var) if is_resource_id(g) => parts.push((var.to_owned(), (*g).to_owned())),
            Some(_) => return Err(refused()),
            None if e == g => {}
            None => return Err(refused()),
        }
    }
    Ok(parts)
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

/// A response printed as its JSON mapping.
struct Json(serde_json::Value);

impl SerializableOutput for Json {
    fn to_json(&self) -> Result<serde_json::Value, SerializationError> {
        Ok(self.0.clone())
    }
}

/// `value` in its proto3 JSON mapping, as the API sends it.
pub fn json(value: &impl Serialize) -> anyhow::Result<Box<dyn SerializableOutput>> {
    Ok(Box::new(Json(serde_json::to_value(value)?)))
}

/// A list response in its JSON mapping, with its list `field` as `[]` when
/// it is empty, rather than left out as the mapping leaves out every empty
/// field: `{"volumes": []}`.
pub fn list_json(
    value: &impl Serialize,
    field: &str,
) -> anyhow::Result<Box<dyn SerializableOutput>> {
    let mut value = serde_json::to_value(value)?;
    if let Some(object) = value.as_object_mut() {
        object
            .entry(field)
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
    }
    Ok(Box::new(Json(value)))
}

/// The result of a command whose method has none, such as a delete.
pub fn success() -> Box<dyn SerializableOutput> {
    Box::new(CommandResult::Success)
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
        let profile = Box::leak(Box::new(profile));
        Context::new(profile, None).unwrap().names(given)
    }

    const VOLUME: &str = "projects/{project}/zones/{zone}/volumes/{volume}";
    const ZONE: &str = "projects/{project}/zones/{zone}";

    #[test]
    fn a_name_is_made_of_its_flags_and_the_profile() {
        let n = names(&[("project", None), ("zone", Some("z1"))]);
        assert_eq!(
            n.name(VOLUME, Some(("volume", "v1"))).unwrap(),
            "projects/default-project/zones/z1/volumes/v1"
        );
        assert_eq!(
            n.name(ZONE, None).unwrap(),
            "projects/default-project/zones/z1"
        );
        assert_eq!(
            n.with_part("zone", "z2").name(ZONE, None).unwrap(),
            "projects/default-project/zones/z2"
        );
        assert_eq!(
            n.name(
                "storageClasses/{storage_class}",
                Some(("storage-class", "gp1"))
            )
            .unwrap(),
            "storageClasses/gp1"
        );
    }

    #[test]
    fn a_missing_part_names_its_flag_and_the_profile_key() {
        let n = names(&[("project", Some("p")), ("zone", None)]);
        let err = n.name(ZONE, None).unwrap_err().to_string();
        assert!(err.contains("--zone is required"), "{err}");
        assert!(err.contains("profile-set default-zone"), "{err}");
        assert!(n.check_without(ZONE, "zone").is_ok());
        let n = names(&[("project", Some("a/b")), ("zone", None)]);
        assert!(n.check_without(ZONE, "zone").is_err());
    }

    #[test]
    fn a_whole_name_fills_in_the_other_parts() {
        let mut n = names(&[("project", None), ("zone", None)]);
        n.learn(
            VOLUME,
            "source-volume",
            Some("projects/p/zones/z/volumes/v"),
        )
        .unwrap();
        assert_eq!(n.name(ZONE, None).unwrap(), "projects/p/zones/z");
        assert_eq!(
            n.name(
                VOLUME,
                Some(("source-volume", "projects/p/zones/z/volumes/v"))
            )
            .unwrap(),
            "projects/p/zones/z/volumes/v"
        );
    }

    #[test]
    fn a_whole_name_disagreeing_with_a_flag_is_refused() {
        let mut n = names(&[("project", Some("other")), ("zone", None)]);
        let err = n
            .learn(VOLUME, "volume", Some("projects/p/zones/z/volumes/v"))
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
            assert!(n.name(VOLUME, Some(("volume", bad))).is_err(), "{bad}");
        }
    }

    /// No part can climb out of its place in the path: `.`, `..`, a `/` or
    /// anything outside the id alphabet is refused, from a flag, a whole
    /// name or the profile.
    #[test]
    fn a_part_outside_the_id_alphabet_is_refused() {
        let n = names(&[("zone", Some("z"))]);
        for bad in ["..", ".", "", "a b", "v?x", "v%2F"] {
            let err = n
                .name(VOLUME, Some(("volume", bad)))
                .unwrap_err()
                .to_string();
            assert!(err.starts_with("--volume:"), "{bad:?}: {err}");
        }
        let err = names(&[("project", Some("a/b")), ("zone", Some("z"))])
            .name(ZONE, None)
            .unwrap_err()
            .to_string();
        assert!(err.starts_with("--project:"), "{err}");
        let err = names(&[("project", Some("..")), ("zone", Some("z"))])
            .name(ZONE, None)
            .unwrap_err()
            .to_string();
        assert!(err.contains("isn't an id"), "{err}");
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
        let printed = list_json(&empty, "volumes").unwrap().to_json().unwrap();
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
