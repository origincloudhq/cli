use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum ConfigCommands {
    /// Show the values stored in the local CLI profile.
    Profile,

    /// Set a value in the local CLI profile.
    ///
    /// Keys: `default-username` (prefilled at `origin id login`),
    /// `output-format` (`table`, `wide`, `json` or `name`; without it, `table`
    /// on a terminal and `json` otherwise), `default-project` and `default-zone` (used
    /// by the generated commands, such as `origin pvs`, when `--project` or
    /// `--zone` isn't given) and `region` (the region they work in: their
    /// `--region` when not given, and the front door of the services with
    /// regional hosts, such as `pvs`; without it, the region of
    /// `default-zone`), and `credentials-file` (a service account key's
    /// credentials file, as `origin iam create-service-account-key` writes
    /// it: the commands call the API as that service account, before a CI
    /// job's token and the login session), and `impersonate-service-account`
    /// (a service account's full name: the commands act as it, with the
    /// credentials found, as `--impersonate-service-account` does; an empty
    /// value removes it). `origin config profile-set region eu-central1`
    /// sets the region. The environment variables ORIGINCLOUD_PROJECT,
    /// ORIGINCLOUD_ZONE, ORIGINCLOUD_REGION, ORIGINCLOUD_CREDENTIALS_FILE and
    /// ORIGINCLOUD_IMPERSONATE_SERVICE_ACCOUNT override the profile's values,
    /// and a flag overrides both.
    ProfileSet {
        /// The key to set.
        #[arg(index = 1)]
        key: String,
        /// Its value.
        #[arg(index = 2)]
        value: String,
    },
}
