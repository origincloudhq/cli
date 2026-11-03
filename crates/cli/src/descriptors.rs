//! The descriptors of the API's messages, which the generator checks in
//! beside the commands: what a message flag's JSON is checked against, and
//! what the human output formats know of a response's fields.

use std::sync::LazyLock;

use prost_reflect::{DescriptorPool, MessageDescriptor};

static DESCRIPTORS: LazyLock<Vec<DescriptorPool>> = LazyLock::new(|| {
    #[allow(unused_mut)]
    let mut pools =
        vec![
            DescriptorPool::decode(include_bytes!("sdk/generated/api.binpb").as_slice())
                .expect("the checked-in descriptors decode"),
        ];
    #[cfg(feature = "test-services")]
    pools.push(
        DescriptorPool::decode(include_bytes!("sdk/generated/test-api.binpb").as_slice())
            .expect("the checked-in descriptors decode"),
    );
    pools
});

/// The message `name` names in full, such as
/// `origincloud.pvs.v1beta1.Volume`.
pub(crate) fn message(name: &str) -> Option<MessageDescriptor> {
    DESCRIPTORS
        .iter()
        .find_map(|pool| pool.get_message_by_name(name))
}
