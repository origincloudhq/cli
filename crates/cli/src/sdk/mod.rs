//! The command groups generated from the API definitions, one per service
//! (`origin pvs`, `origin location`, ...), calling the Rust SDK. The
//! generator (`generator/`) writes `generated/`: a file per service, and
//! `services.rs`, which makes each a group of `origin`. `runtime.rs` holds
//! what the generated commands share; `credentials.rs` hands the SDK the
//! CLI's login session.

mod credentials;
mod runtime;

// Included rather than declared as modules, so that rustfmt leaves the
// generated code as the generator wrote it.
mod generated {
    include!("generated/services.rs");
}

pub(crate) use generated::{run, ServiceCommands};
