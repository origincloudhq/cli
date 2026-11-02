use clap::Subcommand;

#[derive(Subcommand)]
pub(crate) enum ConfigCommands {
    /// Show the values stored in the local CLI profile.
    Profile,

    /// Set a value in the local CLI profile.
    ///
    /// Keys: `default-username` (prefilled at `origin id login`),
    /// `output-format` (`json`), `default-project` and `default-zone` (used
    /// by the generated commands, such as `origin pvs`, when `--project` or
    /// `--zone` isn't given) and `region` (the region they work in: their
    /// `--region` when not given, and the front door of the services with
    /// regional hosts, such as `pvs`; without it, the region of
    /// `default-zone`). `origin config profile-set region eu-central1` sets
    /// the region. The environment variables ORIGINCLOUD_PROJECT,
    /// ORIGINCLOUD_ZONE and ORIGINCLOUD_REGION override the profile's
    /// values, and a flag overrides both.
    ProfileSet {
        /// The key to set.
        #[arg(index = 1)]
        key: String,
        /// Its value.
        #[arg(index = 2)]
        value: String,
    },
}
