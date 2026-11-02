use crate::confirm::Confirmation;
use crate::handler::tag_args::TagArg;
use crate::identity::Urn;
use clap::{ArgGroup, Subcommand};

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum ComputeCommands {
    /// Launch a new instance.
    #[command(group(
        ArgGroup::new("image")
            .args(["image_urn", "image_family"])
            .required(true)
            .multiple(false)
    ))]
    LaunchInstance {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,

        /// Image to launch from, as `list-images` prints it. Exactly one of
        /// `--image-urn` and `--image-family` is required; a family's URN is
        /// refused here - pass its slug to `--image-family`.
        #[arg(long)]
        image_urn: Option<Urn>,

        /// Image family to launch from, e.g. "ubuntu-22-04-minimal": the
        /// image currently designated for it (see `list-image-families`).
        /// Exactly one of `--image-urn` and `--image-family` is required.
        #[arg(long)]
        image_family: Option<String>,

        /// Subnet the Instance will belong to. Omit to use the project's
        /// default subnet - lazily created, on first use, in a VPC
        /// with an Internet Gateway already attached and a public IP
        /// auto-assigned to the instance's interface.
        #[arg(long)]
        subnet_urn: Option<Urn>,

        /// SSH key pair to inject into the guest's default user.
        #[arg(long)]
        key_pair_urn: Urn,

        /// Instance type to launch, e.g. "gp1.core.2" - see
        /// `list-instance-types` for the full live catalog.
        #[arg(long)]
        instance_type: String,

        /// Storage backend for the instance's volumes. Only `pvs`, the
        /// default, is accepted; omit this option.
        #[arg(long)]
        volume_backend: Option<String>,

        /// Size of the instance's root disk, in GiB. Omitted takes the
        /// image's own size, which is also the smallest value accepted - a
        /// smaller one is refused rather than raised.
        #[arg(long)]
        boot_volume_size_gib: Option<u32>,

        /// Performance tier for the instance's root disk, e.g. "gp1" - see
        /// `origin storage list-storage-classes` for the tiers currently on
        /// offer. Omitted takes the default.
        #[arg(long)]
        boot_volume_storage_class: Option<String>,

        /// Security groups to attach to the primary network interface.
        /// Repeatable. Omitted, the interface gets the VPC's default security
        /// group when the VPC has one, and none otherwise: an interface in no
        /// security group admits and sends nothing but ARP, IPv6 neighbor
        /// discovery and replies to its own traffic.
        #[arg(long)]
        security_group_urn: Vec<Urn>,

        /// The account to create on the guest and inject the key pair into,
        /// overriding the image's default user for this instance. A POSIX
        /// username other than `root`. Omitted takes the image's default;
        /// either way the instance reports the result as `loginUser`.
        #[arg(long)]
        login_user: Option<String>,

        /// A tag to create the instance with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// List the live catalog of launchable instance types.
    ListInstanceTypes,

    /// List every image family on offer. A family is a stable name that
    /// resolves to the image currently designated for it; launch with
    /// `launch-instance --image-family <slug>`. `supportEndsAt`, when set, is
    /// when upstream support for that release ends; it is advisory and
    /// nothing is refused on it.
    ListImageFamilies,

    /// Import the public half of an SSH key pair you already have, for use
    /// when launching instances.
    ImportKeyPair {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,

        /// Name for the key pair.
        #[arg(long)]
        name: String,

        /// Full OpenSSH public key line, e.g. `ssh-ed25519 AAAA... comment`.
        #[arg(long)]
        public_key: String,
    },

    /// Generate an SSH key pair on the platform. The private key is in the
    /// response and is never retrievable again - save it immediately.
    GenerateKeyPair {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,

        /// Name for the key pair.
        #[arg(long)]
        name: String,
    },

    /// List the key pairs of a project.
    ListKeyPairs {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Get a single key pair.
    GetKeyPair {
        /// URN of the key pair.
        #[arg(long)]
        key_pair_urn: Urn,
    },

    /// Delete a key pair.
    DeleteKeyPair {
        /// URN of the key pair.
        #[arg(long)]
        key_pair_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// List the instances of a project.
    ListInstances {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Get one instance, including its volume attachments.
    GetInstance {
        /// URN of the Instance.
        #[arg(long)]
        instance_urn: Urn,
    },

    /// List the attachments that reference a Volume, each naming its Instance.
    ///
    /// Use it to find which Instance holds a volume that is INUSE: the entry
    /// whose `id` equals the volume's `claimRef` (see `origin storage
    /// get-volume`) is the one holding it, and its `instanceUrn` is the
    /// Instance. Normally there is exactly one entry, with status ATTACHED;
    /// detached attachments are not listed. A volume from another project
    /// returns an empty list.
    ListVolumeAttachments {
        /// URN of the project the volume belongs to.
        #[arg(long)]
        project_urn: Urn,

        /// URN of the Volume.
        #[arg(long)]
        volume_urn: Urn,
    },

    /// Start a stopped Instance.
    StartInstance {
        /// URN of the Instance.
        #[arg(long)]
        instance_urn: Urn,
    },

    /// Stop a running instance. Its network interface and volumes stay with
    /// it.
    StopInstance {
        /// URN of the Instance.
        #[arg(long)]
        instance_urn: Urn,
    },

    /// Change a stopped Instance's type, so it starts again at a different size.
    ///
    /// The Instance must be stopped. It keeps its identity, its address and
    /// its volumes; only the size changes. Growing is checked against the
    /// same vCPU quota a launch is. `list-instance-types` shows what you
    /// can change to.
    ChangeInstanceType {
        /// URN of the Instance.
        #[arg(long)]
        instance_urn: Urn,

        /// The type to change to, e.g. "gp1.core.4".
        #[arg(long)]
        instance_type: String,
    },

    /// Terminate an instance. Its network interface and the volumes its
    /// launch created are deleted; volumes you attached yourself are detached
    /// and kept.
    TerminateInstance {
        /// URN of the Instance.
        #[arg(long)]
        instance_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Attach an available volume to an instance, running or stopped.
    ///
    /// On a running instance the disk is hot-plugged: the attachment reports
    /// ATTACHING and settles to ATTACHED once the guest sees the device. On a
    /// stopped instance it appears at the next start.
    AttachVolume {
        /// URN of the Instance.
        #[arg(long)]
        instance_urn: Urn,
        /// URN of the Volume.
        #[arg(long)]
        volume_urn: Urn,
        /// Guest device name (vda..vdz). Assigned by the platform when omitted.
        #[arg(long)]
        device_name: Option<String>,
    },

    /// Detach a volume from an instance, running or stopped, making it
    /// available again.
    ///
    /// On a running instance the attachment reports DETACHING until the guest
    /// releases the device; the volume becomes available only after that.
    DetachVolume {
        /// URN of the Instance.
        #[arg(long)]
        instance_urn: Urn,
        /// URN of the Volume.
        #[arg(long)]
        volume_urn: Urn,
    },

    /// Create an image from a protected snapshot.
    CreateImage {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,

        /// Root device name: a bare virtio target, `vda`..`vdz` (`vdc` is
        /// reserved) - not a `/dev/` path.
        #[arg(long, default_value = "vda")]
        root_device_name: String,

        /// Root device size in GiB.
        #[arg(long, default_value = "8")]
        root_device_size: i64,

        /// Root device backing Snapshot.
        #[arg(long)]
        root_device_snapshot_urn: Urn,

        /// The account instances launched from this image log in as - the
        /// non-root user created at first boot, holding the launch's SSH
        /// key. A POSIX username other than `root`; defaults to `cloud`.
        #[arg(long)]
        default_user: Option<String>,

        /// Description of the Image.
        #[arg(long)]
        description: String,

        /// A tag to create the image with, as KEY=VALUE. Repeat for more.
        /// `--tag origin:name=<name>` names it.
        #[arg(long = "tag", value_name = "KEY=VALUE")]
        tags: Vec<TagArg>,
    },

    /// List the images available to a project, your own and
    /// platform-provided.
    ListImages {
        /// URN of the project.
        #[arg(long)]
        project_urn: Urn,
    },

    /// Get one image.
    GetImage {
        /// URN of the Image.
        #[arg(long)]
        image_urn: Urn,
    },

    /// Update a customer-owned Image's description - the only mutable field.
    UpdateImage {
        /// URN of the Image.
        #[arg(long)]
        image_urn: Urn,

        /// New description.
        #[arg(long)]
        description: String,
    },

    /// Delete a customer-owned Image.
    DeleteImage {
        /// URN of the Image.
        #[arg(long)]
        image_urn: Urn,

        #[command(flatten)]
        confirmation: Confirmation,
    },

    /// Set tags on an instance or one of the project's own images. A key it
    /// already carries gets the new value; its other tags are kept. Prints
    /// all of its tags.
    SetResourceTags {
        /// URN of the instance or image. Platform images take no tags.
        #[arg(long)]
        resource_urn: Urn,

        /// A tag to set, as KEY=VALUE. Repeat for more.
        #[arg(long = "tag", value_name = "KEY=VALUE", required = true)]
        tags: Vec<TagArg>,
    },

    /// Remove tags from an instance or one of the project's own images by
    /// key. Prints the tags left.
    RemoveResourceTags {
        /// URN of the instance or image.
        #[arg(long)]
        resource_urn: Urn,

        /// Key of a tag to remove. Repeat for more.
        #[arg(long = "key", value_name = "KEY", required = true)]
        keys: Vec<String>,
    },
}
