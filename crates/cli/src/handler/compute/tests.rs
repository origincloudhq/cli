//! Argument-surface tests for the compute subcommands.
//!
//! These parse through the real root command, so they pin the invocation a
//! user actually types - not just the shape of the enum.

use crate::handler::compute::ComputeCommands;
use crate::root_command::{Cli, Commands};
use clap::Parser;

const PROJECT_URN: &str = "urn:origin:cloud:admin:eu-central1:0a1b2c3d4e5f::my-project-123";
const VOLUME_URN: &str =
    "urn:origin:cloud:pvs:eu-central1:0a1b2c3d4e5f:my-project-123:volume/abc123";

fn parse_compute(args: &[&str]) -> Result<ComputeCommands, clap::Error> {
    let mut argv = vec!["origin", "compute"];
    argv.extend_from_slice(args);

    match Cli::try_parse_from(argv)?.command {
        Some(Commands::Compute(command)) => Ok(command),
        _ => panic!("expected a compute subcommand"),
    }
}

/// The CLI half of "which instance holds this volume". Both URNs
/// reach the command unchanged - the project is not derived from the volume,
/// because the API authorises the project and checks the volume against it.
#[test]
fn list_volume_attachments_takes_the_project_and_the_volume() {
    let command = parse_compute(&[
        "list-volume-attachments",
        "--project-urn",
        PROJECT_URN,
        "--volume-urn",
        VOLUME_URN,
    ])
    .unwrap_or_else(|err| panic!("the documented invocation must parse: {err}"));

    match command {
        ComputeCommands::ListVolumeAttachments {
            project_urn,
            volume_urn,
        } => {
            assert_eq!(project_urn.to_string(), PROJECT_URN);
            assert_eq!(volume_urn.to_string(), VOLUME_URN);
        }
        _ => panic!("expected ListVolumeAttachments"),
    }
}

#[test]
fn list_volume_attachments_requires_the_volume() {
    assert!(
        parse_compute(&["list-volume-attachments", "--project-urn", PROJECT_URN]).is_err(),
        "without a volume there is nothing to look up"
    );
}

/// The project is required rather than inferred: the API authorises it on
/// its own, and a volume from another project reads as an empty list - so a
/// silently wrong default would look like "not attached" instead of an error.
#[test]
fn list_volume_attachments_requires_the_project() {
    assert!(
        parse_compute(&["list-volume-attachments", "--volume-urn", VOLUME_URN]).is_err(),
        "the project must be stated, not guessed"
    );
}

const KEY_PAIR_URN: &str =
    "urn:origin:cloud:compute:eu-central1:0a1b2c3d4e5f:my-project-123:keypair/kp1";
const IMAGE_URN: &str =
    "urn:origin:cloud:compute:eu-central1:0a1b2c3d4e5f:my-project-123:image/img1";

fn launch(image_args: &[&str]) -> Result<ComputeCommands, clap::Error> {
    let mut args = vec![
        "launch-instance",
        "--project-urn",
        PROJECT_URN,
        "--key-pair-urn",
        KEY_PAIR_URN,
        "--instance-type",
        "gp1.core.2",
    ];
    args.extend_from_slice(image_args);
    parse_compute(&args)
}

/// `--image-family` launches whatever image the family currently
/// designates, as the API's `imageFamily` does.
#[test]
fn launch_instance_takes_an_image_family_instead_of_an_image() {
    match launch(&["--image-family", "ubuntu-22-04-minimal"]).expect("must parse") {
        ComputeCommands::LaunchInstance {
            image_urn,
            image_family,
            ..
        } => {
            assert!(image_urn.is_none());
            assert_eq!(image_family.as_deref(), Some("ubuntu-22-04-minimal"));
        }
        _ => panic!("expected launch-instance"),
    }
}

#[test]
fn launch_instance_takes_an_image_alone() {
    match launch(&["--image-urn", IMAGE_URN]).expect("must parse") {
        ComputeCommands::LaunchInstance {
            image_urn,
            image_family,
            ..
        } => {
            assert_eq!(
                image_urn.map(|urn| urn.to_string()).as_deref(),
                Some(IMAGE_URN)
            );
            assert!(image_family.is_none());
        }
        _ => panic!("expected launch-instance"),
    }
}

#[test]
fn launch_instance_refuses_both_an_image_and_a_family() {
    let err = launch(&[
        "--image-urn",
        IMAGE_URN,
        "--image-family",
        "ubuntu-22-04-minimal",
    ])
    .err()
    .expect("both together must be refused");
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

#[test]
fn launch_instance_refuses_neither_an_image_nor_a_family() {
    let err = launch(&[]).err().expect("one of the two is required");
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
}
