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
    /// `--zone` isn't given) and `region` (the region whose front door they
    /// call, for a service with regional hosts such as `pvs`).
    ProfileSet {
        /// The key to set.
        #[arg(index = 1)]
        key: String,
        /// Its value.
        #[arg(index = 2)]
        value: String,
    },
}
