//! The ids and regions the CLI checks itself: a segment of a resource name,
//! as the SDK checks the names it sends, and a region, and the region of a
//! zone.

/// Whether `s` can be one segment of a resource name, by the SDK's own
/// check of the names it sends: not empty, `.` or `..`, and without `/`,
/// so that no segment can add one or climb out of its place in the path.
/// Every other character is the API's to accept, as in `gp1.core.2`; the
/// SDK escapes what a path can't hold.
pub(crate) fn is_resource_id(s: &str) -> bool {
    origincloud_core::path_value("id", s, "*").is_ok()
}

/// What [`is_resource_id`] refuses, for its errors.
pub(crate) const RESOURCE_ID_RULE: &str = "an id can't be empty, `.` or `..`, or hold a `/`";

/// Whether `s` can be a region, such as `eu-central1`: lowercase letters,
/// digits and `-`, as the SDK requires of a host label.
pub(crate) fn is_region(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The region of the zone `zone`, which the platform names `{region}-az{n}`:
/// `eu-central1` for `eu-central1-az1`. `None` for a zone not named so.
pub(crate) fn region_of_zone(zone: &str) -> Option<&str> {
    let (region, number) = zone.rsplit_once("-az")?;
    (!number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) && is_region(region))
        .then_some(region)
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
            "gp1.core.2",
            "24.04",
            "...",
            "a b",
        ] {
            assert!(is_resource_id(ok), "{ok}");
        }
        for bad in ["", ".", "..", "a/b", "/", "a/"] {
            assert!(!is_resource_id(bad), "{bad:?}");
        }
        assert!(is_region("eu-central1"));
        assert!(!is_region("EU-central1") && !is_region("eu.central1") && !is_region(""));
    }

    #[test]
    fn a_zone_names_its_region() {
        assert_eq!(region_of_zone("eu-central1-az1"), Some("eu-central1"));
        assert_eq!(region_of_zone("eu-central1-az12"), Some("eu-central1"));
        for other in [
            "eu-central1",
            "eu-central1-az",
            "eu-central1-aza",
            "-az1",
            "EU-az1",
            "z-a",
        ] {
            assert_eq!(region_of_zone(other), None, "{other}");
        }
    }
}
