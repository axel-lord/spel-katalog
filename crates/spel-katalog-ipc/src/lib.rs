//! Inter process communication.

pub use crate::{
    incoming::{IncomingRequest, IncomingResponse},
    send::SendError,
};

pub mod generic {
    //! Generic ipc listen and send.
    pub use crate::send::{connect, send};
}

pub mod typed {
    //! Typed ipc.

    mod error;
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

    use ::serde::{Serialize, de::DeserializeOwned};
    use ::spel_katalog_formats::daemon::{self, Exchange};

    /// Type alias to allow naming of method resolvers.
    pub type MethodResolver = Resolver<crate::typed::resolver::kind::Method>;

    pub use crate::typed::{
        error::ListenerError,
        listen::IpcListener,
        resolver::Resolver,
        send::{GetError, PostError, get, post},
    };
}

pub mod http;

mod incoming;
mod send;
