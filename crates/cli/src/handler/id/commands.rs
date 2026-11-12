use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum IdentityCommands {
    /// Log in through your browser, with a code on another device, or as a
    /// service account with your identity provider's token
    ///
    /// Opens the Origin Cloud login page in your browser and waits for you to
    /// log in there. With --device, prints an address and a code to approve
    /// on any other device instead, for machines without a browser such as
    /// an SSH session. The CLI never asks for your password.
    ///
    /// With --oidc-token-file, assumes a `WORKLOAD` service account with a token
    /// from your own identity provider, which one of the account's federated
    /// credentials trusts, and runs the commands that follow as that account.
    /// Once its access token expires, the CLI reads the file again and
    /// assumes the account again, so a file the identity provider or a
    /// credential helper keeps current keeps the login going; a token from
    /// standard input (`-`) is read once.
    Login {
        /// Log in by approving a code on another device, instead of opening a
        /// browser on this machine
        #[arg(long, default_value_t = false, conflicts_with = "oidc_token_file")]
        device: bool,

        /// Log in as a service account with the identity provider's token in
        /// this file, or on standard input with `-`
        #[arg(long, value_name = "PATH")]
        oidc_token_file: Option<String>,

        /// The service account to assume with --oidc-token-file,
        /// `organizations/{organization}/serviceAccounts/{service_account}`;
        /// without it, the one whose federated credential trusts the token
        #[arg(long, value_name = "NAME", requires = "oidc_token_file")]
        service_account: Option<String>,
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
