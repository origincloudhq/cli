//! The identifiers the hand-written GraphQL commands take and send: URNs,
//! organisation and project ids, and principals.
//!
//! The GraphQL schema declares them as custom scalars, which the generated
//! query code expects as Rust types of the same names. They travel as
//! strings; the server validates them in full. The checks here only refuse
//! what can't be one of them, so that a mistyped argument fails before a
//! request is sent, with a message saying which argument it was.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Implements `Display`, `Serialize` and `Deserialize` through the type's
/// string form and `FromStr`.
macro_rules! string_form {
    ($type:ty) => {
        impl fmt::Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl Serialize for $type {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let s = String::deserialize(deserializer)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

/// Whether `s` is a non-empty run of lowercase letters, digits and `-`: the
/// alphabet organisation and project ids are written in.
fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// An organisation's id, such as `0a1b2c3d4e5f`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OrgId(String);

impl FromStr for OrgId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if is_id(s) {
            Ok(Self(s.to_owned()))
        } else {
            Err(format!(
                "{s:?} isn't an organisation id: expected lowercase letters and digits"
            ))
        }
    }
}

string_form!(OrgId);

/// A project's id, such as `my-project-123`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProjectId(String);

impl FromStr for ProjectId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if is_id(s) {
            Ok(Self(s.to_owned()))
        } else {
            Err(format!(
                "{s:?} isn't a project id: expected lowercase letters, digits and `-`"
            ))
        }
    }
}

string_form!(ProjectId);

/// Who a role is granted to: `{type}:{id}`, such as `user:<id>`,
/// `service_account:<id>` or `org:<id>`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Principal(String);

const PRINCIPAL_TYPES: [&str; 4] = ["user", "service_account", "org", "service"];

impl FromStr for Principal {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((kind, id)) if PRINCIPAL_TYPES.contains(&kind) && !id.is_empty() => {
                Ok(Self(s.to_owned()))
            }
            _ => Err(format!(
                "{s:?} isn't a principal: expected {{type}}:{{id}}, with a type of {}",
                PRINCIPAL_TYPES.join(", ")
            )),
        }
    }
}

string_form!(Principal);

/// A resource's URN:
/// `urn:origin:cloud:{service}:{region}:{org}:{project}:{resource}`, where
/// the region, organisation and project may be empty.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Urn(String);

/// The segments before the resource id, which may itself contain `:`.
const URN_SEGMENTS: usize = 8;

impl Urn {
    /// The segment at `index`, empty ones as `None`.
    fn segment(&self, index: usize) -> Option<&str> {
        self.0
            .splitn(URN_SEGMENTS, ':')
            .nth(index)
            .filter(|s| !s.is_empty())
    }

    /// The organisation the URN names, if any.
    pub fn org_id(&self) -> Option<OrgId> {
        self.segment(5).and_then(|s| s.parse().ok())
    }

    /// The project the URN names, if any.
    pub fn project_id(&self) -> Option<ProjectId> {
        self.segment(6).and_then(|s| s.parse().ok())
    }

    /// The URN of a project, as the administration service names it: with
    /// no region, `urn:origin:cloud:admin::{org}::{project}`.
    pub fn of_project(org_id: &OrgId, project_id: &ProjectId) -> Self {
        Self(format!("urn:origin:cloud:admin::{org_id}::{project_id}"))
    }
}

impl FromStr for Urn {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let segments: Vec<&str> = s.splitn(URN_SEGMENTS, ':').collect();
        let valid = segments.len() == URN_SEGMENTS
            && segments[..3] == ["urn", "origin", "cloud"]
            && !segments[3].is_empty()
            && !segments[7].is_empty();
        if valid {
            Ok(Self(s.to_owned()))
        } else {
            Err(format!(
                "{s:?} isn't a URN: expected urn:origin:cloud:{{service}}:{{region}}:{{org}}:{{project}}:{{resource}}"
            ))
        }
    }
}

string_form!(Urn);

/// Whether `s` can be one segment of a resource name: a non-empty run of
/// letters, digits, `-` and `_`, the alphabet of the API's resource ids
/// (`vol-6r4pYoPrWD2`, `my-project-123`, `eu-central1-az1`, `gp1`). It
/// leaves out `/`, which would add a segment, and `.`, so that no segment
/// can be `.` or `..`.
pub(crate) fn is_resource_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Whether `s` can be a region, such as `eu-central1`: lowercase letters,
/// digits and `-`, as the SDK requires of a host label.
pub(crate) fn is_region(s: &str) -> bool {
    is_id(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_ids_and_regions() {
        for ok in [
            "vol-6r4pYoPrWD2",
            "my-project-123",
            "eu-central1-az1",
            "gp1",
            "a_b",
        ] {
            assert!(is_resource_id(ok), "{ok}");
        }
        for bad in ["", ".", "..", "a/b", "a.b", "a b", "ä"] {
            assert!(!is_resource_id(bad), "{bad:?}");
        }
        assert!(is_region("eu-central1"));
        assert!(!is_region("EU-central1") && !is_region("eu.central1") && !is_region(""));
    }

    #[test]
    fn a_urn_names_its_organisation_and_project() {
        let urn: Urn =
            "urn:origin:cloud:compute:eu-central1:0a1b2c3d4e5f:my-project-123:instance/abc123"
                .parse()
                .unwrap();
        assert_eq!(urn.org_id().unwrap().to_string(), "0a1b2c3d4e5f");
        assert_eq!(urn.project_id().unwrap().to_string(), "my-project-123");
        assert_eq!(
            Urn::of_project(&urn.org_id().unwrap(), &urn.project_id().unwrap()).to_string(),
            "urn:origin:cloud:admin::0a1b2c3d4e5f::my-project-123"
        );
    }

    #[test]
    fn a_resource_id_may_contain_colons() {
        let urn: Urn = "urn:origin:cloud:iam::0a1b2c3d4e5f::role/a:b"
            .parse()
            .unwrap();
        assert_eq!(
            urn.to_string(),
            "urn:origin:cloud:iam::0a1b2c3d4e5f::role/a:b"
        );
        assert!(urn.project_id().is_none());
    }

    #[test]
    fn what_isnt_a_urn_is_refused() {
        for s in [
            "",
            "urn:origin:cloud",
            "urn:other:cloud:compute::o:p:r",
            "urn:origin:cloud:::o:p:r",
            "urn:origin:cloud:compute::o:p:",
        ] {
            assert!(s.parse::<Urn>().is_err(), "{s:?}");
        }
    }

    #[test]
    fn ids_and_principals_are_checked_and_travel_as_strings() {
        assert!("my-project-123".parse::<ProjectId>().is_ok());
        assert!("My-Project".parse::<ProjectId>().is_err());
        assert!("".parse::<OrgId>().is_err());
        assert!("user:abc".parse::<Principal>().is_ok());
        assert!("robot:abc".parse::<Principal>().is_err());
        assert!("user:".parse::<Principal>().is_err());
        let project: ProjectId = "p-1".parse().unwrap();
        assert_eq!(serde_json::to_string(&project).unwrap(), "\"p-1\"");
        assert_eq!(
            serde_json::from_str::<ProjectId>("\"p-1\"").unwrap(),
            project
        );
    }
}
