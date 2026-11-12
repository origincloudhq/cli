//! Argument-surface tests for `origin id`, parsed through the real root
//! command.

use crate::handler::id::IdentityCommands;
use crate::root_command::{Cli, Commands};
use clap::Parser;

fn parse_id(args: &[&str]) -> Result<IdentityCommands, clap::Error> {
    let mut argv = vec!["origin", "id"];
    argv.extend_from_slice(args);

    match Cli::try_parse_from(argv)?.command {
        Some(Commands::Id(command)) => Ok(command),
        _ => panic!("expected an id subcommand"),
    }
}

#[test]
fn login_defaults_to_the_browser() {
    assert!(matches!(
        parse_id(&["login"]),
        Ok(IdentityCommands::Login {
            device: false,
            oidc_token_file: None,
            service_account: None
        })
    ));
}

#[test]
fn login_device_selects_the_device_code_flow() {
    assert!(matches!(
        parse_id(&["login", "--device"]),
        Ok(IdentityCommands::Login { device: true, .. })
    ));
}

/// The CLI no longer takes a username: it never sees credentials, so a
/// script still passing one fails loudly rather than being ignored.
#[test]
fn login_no_longer_accepts_a_username() {
    for args in [&["login", "--user", "ada"][..], &["login", "-u", "ada"][..]] {
        let err = parse_id(args).err().expect("a username must be refused");
        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}

#[test]
fn login_token_file_takes_a_path_or_stdin_and_a_service_account() {
    let Ok(IdentityCommands::Login {
        oidc_token_file: Some(file),
        service_account: Some(account),
        device: false,
    }) = parse_id(&[
        "login",
        "--oidc-token-file",
        "/run/token",
        "--service-account",
        "organizations/o/serviceAccounts/s",
    ])
    else {
        panic!("--oidc-token-file and --service-account parse");
    };
    assert_eq!(file, "/run/token");
    assert_eq!(account, "organizations/o/serviceAccounts/s");
    assert!(matches!(
        parse_id(&["login", "--oidc-token-file", "-"]),
        Ok(IdentityCommands::Login { oidc_token_file: Some(f), service_account: None, .. }) if f == "-"
    ));
}

#[test]
fn login_service_account_needs_a_token_file_and_device_excludes_one() {
    for args in [
        &[
            "login",
            "--service-account",
            "organizations/o/serviceAccounts/s",
        ][..],
        &["login", "--device", "--oidc-token-file", "t"][..],
    ] {
        assert!(parse_id(args).is_err(), "{args:?}");
    }
}
