use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum IdentityCommands {
    /// Log in through your browser, or with a code on another device
    ///
    /// Opens the Origin Cloud login page in your browser and waits for you to
    /// log in there. With --device, prints an address and a code to approve
    /// on any other device instead, for machines without a browser such as
    /// an SSH session. The CLI never asks for your password.
    Login {
        /// Log in by approving a code on another device, instead of opening a
        /// browser on this machine
        #[arg(long, default_value_t = false)]
        device: bool,
    },

    /// Print the access token of the current login session, for calling the
    /// API directly. Without a login session, on an instance that runs as a
    /// service account, the credential of that service account.
    Token {
        /// Print the token as an HTTP `Authorization` header (`Authorization:
        /// Bearer <token>`).
        #[arg(short, long, default_value_t = false)]
        as_header: bool,

        /// Print plain text with no JSON envelope, for direct use in shell one-liners
        /// (e.g. `curl -H "$(origin id token --as-header --raw)"`)
        #[arg(short, long, default_value_t = false)]
        raw: bool,
    },
}
