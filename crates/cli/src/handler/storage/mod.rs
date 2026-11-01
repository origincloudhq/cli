//! `origin storage`: tag changes by key on volumes and snapshots, which the
//! PVS API (`origin pvs`) doesn't offer. `commands.rs` declares the clap
//! surface; `dispatch.rs` performs the work.

mod commands;
mod dispatch;

pub(crate) use commands::StorageCommands;
pub use dispatch::run_storage_command;
