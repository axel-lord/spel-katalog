//! Formats used for communication with daemon.

use ::core::{convert::Infallible, marker::PhantomData};

use ::serde::{Serialize, de::DeserializeOwned};

use crate::daemon::sealed::Sealed;

mod sealed {
    //! Seal requests

    /// Seal a trait.
    pub trait Sealed {}
}

/// An ipc request method required for some response.
pub trait Method: Sealed {}

/// A typed exchange request to send to a daemon.
pub trait Exchange: Serialize + DeserializeOwned {
    /// Request/Method producing the response.
    ///
    /// If [Post], the trait is implemented for the request
    /// with the generic of [Post] specifying the response.
    ///
    /// If [Get], the trait is implemented for the response.
    type Method: Method;

    /// Uri of the request.
    const URI: &str;
}

/// A get request, has no body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Get {
    /// Make type impossible.
    _p: Infallible,
}

impl Sealed for Get {}
impl Method for Get {}

/// A post request, has a body which must be serializable and deserializable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Post<T: Serialize + DeserializeOwned> {
    /// Make type impossible, and use T.
    _p: (Infallible, PhantomData<fn() -> T>),
}

impl<T: Serialize + DeserializeOwned> Sealed for Post<T> {}
impl<T: Serialize + DeserializeOwned> Method for Post<T> {}

pub mod response {
    //! Daemon responses.
    use ::std::path::PathBuf;

    use ::derive_more::{Deref, DerefMut};
    use ::serde::{Deserialize, Serialize};

    use crate::daemon::{Exchange, Get};

    /// Response returned when running a game on a daemon.
    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub enum Run {
        /// A Pipe was created.
        CreatedPipe {
            /// Name of game.
            name: String,
            /// Path of pipe.
            path: PathBuf,
            /// Pid of process.
            pid: i64,
        },
        /// Could not run game config.
        CouldNotRun {
            /// Name of game.
            name: String,
        },
    }

    /// Response returned when asking for children of daemon.
    #[derive(
        Debug,
        Clone,
        Default,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Deserialize,
        Serialize,
        Deref,
        DerefMut,
    )]
    pub struct Children {
        /// List of current children.
        #[deref]
        #[deref_mut]
        pub children: Vec<i64>,
    }

    impl Exchange for Children {
        type Method = Get;
        const URI: &str = "children";
    }
}

pub mod request {
    //! Daemon requests.

    use ::serde::{Deserialize, Serialize, de::DeserializeOwned};

    use crate::{
        NativeGameConfig, RunMode,
        daemon::{Exchange, Post, response},
    };

    /// Run request sent to daemon.
    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct RunConfig<S> {
        /// Config of game to run.
        pub config: NativeGameConfig,
        /// How to run game.
        pub run_mode: RunMode,
        /// Settings to use when running game.
        pub settings: S,
    }

    impl<S: Serialize + DeserializeOwned> Exchange for RunConfig<S> {
        type Method = Post<response::Run>;

        const URI: &str = "run";
    }
}
