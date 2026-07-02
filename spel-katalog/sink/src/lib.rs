//! Crate with functionality for shared output sinks.

pub use self::{
    builder::SinkBuilder,
    identity::{SinkIdentity, SinkIdentityFactory},
    writer::{AsyncSinkWriter, SinkWriter},
};

mod builder;
mod identity;
mod writer;
