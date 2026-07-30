//! Inter process communication.

use ::serde::{Serialize, de::DeserializeOwned};
use ::spel_katalog_formats::daemon::{self, Exchange};

pub use crate::{listen::IpcListener, resolver::Resolver, send::IpcSender};

pub mod error;
pub mod http;

mod listen;
mod resolver;
mod send;

/// Trait implemented for post exchanges.
pub trait Post: Exchange {
    /// Response to post.
    type Response: Serialize + DeserializeOwned;
}

impl<M, T> Post for M
where
    M: daemon::Exchange<Method = daemon::Post<T>>,
    T: Serialize + DeserializeOwned,
{
    type Response = T;
}

/// Trait implemented for get exchanges.
pub trait Get: Exchange {}

impl<T> Get for T where T: daemon::Exchange<Method = daemon::Get> {}

/// Type alias to allow naming of method resolvers.
pub type MethodResolver = Resolver<crate::resolver::kind::Method>;
