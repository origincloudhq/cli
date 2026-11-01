use crate::handler::tag_args::TagArg;
use crate::identity::Urn;
use clap::Subcommand;

/// Tag changes by key, which keep a resource's other tags. Everything else
/// about volumes, snapshots and storage classes is in `origin pvs`; its
/// `update-volume` and `update-snapshot` replace a resource's tags as a whole.
#[derive(Subcommand)]
pub(crate) enum StorageCommands {
    /// Set tags on a volume or snapshot. A key it already carries gets the
    /// new value; its other tags are kept. Prints all of its tags.
    SetResourceTags {
        /// URN of the volume or snapshot (its `urn`).
        #[arg(long)]
        resource_urn: Urn,

        /// A tag to set, as KEY=VALUE. Repeat for more.
        #[arg(long = "tag", value_name = "KEY=VALUE", required = true)]
        tags: Vec<TagArg>,
    },

    /// Remove tags from a volume or snapshot by key. Prints the tags left.
    RemoveResourceTags {
        /// URN of the volume or snapshot (its `urn`).
        #[arg(long)]
        resource_urn: Urn,

        /// Key of a tag to remove. Repeat for more.
        #[arg(long = "key", value_name = "KEY", required = true)]
        keys: Vec<String>,
    },
}
