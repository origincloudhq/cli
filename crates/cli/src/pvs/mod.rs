//! `origin pvs`: the persistent volume service's commands, generated from
//! the API definitions (`generated/commands.rs`, by `generator/`) and
//! calling the Rust SDK. `runtime.rs` holds what they share;
//! `credentials.rs` hands the SDK the CLI's login session.

mod credentials;
mod runtime;

mod generated {
    include!("generated/commands.rs");
}

pub(crate) use generated::{run, PvsCommands};
pub(crate) use runtime::Context;
