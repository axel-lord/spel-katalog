//! Hidden utilities used by typed listener.

use ::core::convert::Infallible;
use ::std::{borrow::Cow, collections::HashMap, path::PathBuf};

use ::bytes::Bytes;
use ::http_body_util::Full;
use ::hyper::{Response, StatusCode, server::conn::http1, service::service_fn};
use ::smol::{LocalExecutor, net::unix::UnixListener};
use ::smol_hyper::rt::FuturesIo;
use ::uuid::Uuid;

use crate::{
    IncomingRequest,
    http::{HttpResponse, ResponseCode},
};

/// Fatal listener errors.
#[derive(Debug, ::thiserror::Error)]
pub enum ListenerError {
    /// Listener failed to accept a connection.
    #[error("could not accept connection\n{0}")]
    Accept(::smol::io::Error),
    /// Could not get runtime directory.
    #[error("could not get runtime directory\n{0}")]
    GetRuntimeDir(::std::io::Error),
    /// Could not create runtime directory.
    #[error("could not create runtime directory {path:?}\n{err}")]
    CreateRuntimeDir {
        /// Forwarded error.
        err: ::smol::io::Error,
        /// path of directory.
        path: PathBuf,
    },
    /// Could not create socket.
    #[error("could not create unix socket at {path:?}\n{err}")]
    CreateSocket {
        /// Forwarded error.
        err: ::smol::io::Error,
        /// path of socket.
        path: PathBuf,
    },
    /// Could not rename socket after creation.
    #[error("could not rename socket {from:?} -> {to:?}\n{err}")]
    RenameSocket {
        /// Forwarded error.
        err: ::smol::io::Error,
        /// Original name.
        from: PathBuf,
        /// New name.
        to: PathBuf,
    },
    /// Could not rename socket after creation, then could not remove temp file.
    #[error(
        "could not rename socket {from:?} -> {to:?}\n{rename_err}\ncould not remove socket {from}\n{remove_err}"
    )]
    RemoveTempSocket {
        /// Forwarded rename error.
        rename_err: ::smol::io::Error,

        /// Forwarded remove error.
        remove_err: ::smol::io::Error,

        /// Original name. And name of temp socket.
        from: PathBuf,

        /// New name.
        to: PathBuf,
    },
}

/// A Typed listener.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Listener<Get, Post> {
    /// Get stack.
    get: Get,
    /// Post stack.
    post: Post,
}

impl<Get, Post> Listener<Get, Post> {
    /// Construct a new typed listener.
    pub const fn new() -> Listener<impl Layer, impl Layer> {
        Listener {
            get: Initial,
            post: Initial,
        }
    }

    /// Box layer stacks.
    pub fn boxed(self) -> Listener<Box<dyn Layer>, Box<dyn Layer>>
    where
        Post: 'static + Layer,
        Get: 'static + Layer,
    {
        let Self { get, post } = self;

        Listener {
            get: Box::new(get),
            post: Box::new(post),
        }
    }

    /// Get a reference typed listener.
    pub const fn by_ref(&self) -> Listener<&dyn Layer, &dyn Layer>
    where
        Get: Layer,
        Post: Layer,
    {
        let Self { get, post } = self;
        Listener { get, post }
    }

    /// Add post listener.
    pub fn post<M>(
        self,
        listener: impl Fn(M) -> Result<M::Response, HttpResponse>,
    ) -> Listener<Get, impl Layer>
    where
        Post: Layer,
        M: crate::typed::Post,
    {
        let Self { get, post } = self;

        Listener {
            get,
            post: Chain {
                prior: post,
                listener: move |incoming: Bytes| -> Result<Bytes, HttpResponse> {
                    let request = ::serde_json::from_slice::<M>(&incoming)?;
                    let response = listener(request)?;

                    Ok(Bytes::from_owner(::serde_json::to_vec(&response)?))
                },
                uri: M::safe_uri,
            },
        }
    }

    /// Add get listener.
    pub fn get<T>(
        self,
        listener: impl Fn() -> Result<T, HttpResponse>,
    ) -> Listener<impl Layer, Post>
    where
        Get: Layer,
        T: crate::typed::Get,
    {
        let Self { get, post } = self;

        Listener {
            get: Chain {
                prior: get,
                listener: move |_incoming: Bytes| -> Result<Bytes, HttpResponse> {
                    let response = listener()?;
                    Ok(Bytes::from_owner(::serde_json::to_vec(&response)?))
                },
                uri: T::safe_uri,
            },
            post,
        }
    }

    /// Create the given socket in runtime directory, and run listener.
    ///
    /// # Errors
    /// If creation of socket fails.
    /// Or on fatal unix socket listener errors.
    #[expect(clippy::future_not_send, reason = "not intended to be sent")]
    pub async fn create_socket(
        &self,
        name: &str,
        xdg: &::xdg::BaseDirectories,
    ) -> Result<Infallible, ListenerError>
    where
        Get: Layer,
        Post: Layer,
    {
        let runtime_dir = xdg
            .get_runtime_file("")
            .map_err(ListenerError::GetRuntimeDir)?;

        if let Err(err) = ::smol::fs::create_dir_all(&runtime_dir).await {
            return Err(ListenerError::CreateRuntimeDir {
                err,
                path: runtime_dir,
            });
        }

        let temp_path = runtime_dir.join(Uuid::new_v4().to_string());
        match UnixListener::bind(&temp_path) {
            Ok(socket) => {
                let socket_path = runtime_dir.join(name);
                match ::smol::fs::rename(&temp_path, &socket_path).await {
                    Ok(..) => self.listen_on_socket(socket).await,
                    Err(rename_err) => Err(match ::smol::fs::remove_file(&temp_path).await {
                        Ok(..) => ListenerError::RenameSocket {
                            err: rename_err,
                            from: temp_path,
                            to: socket_path,
                        },
                        Err(remove_err) => ListenerError::RemoveTempSocket {
                            rename_err,
                            remove_err,
                            from: temp_path,
                            to: socket_path,
                        },
                    }),
                }
            }
            Err(err) => Err(ListenerError::CreateSocket {
                err,
                path: temp_path,
            }),
        }
    }

    /// Run listener on the given socket.
    ///
    /// # Errors
    /// On fatal unix socket listener errors.
    #[expect(clippy::future_not_send, reason = "not intended to be sent")]
    pub async fn listen_on_socket(&self, socket: UnixListener) -> Result<Infallible, ListenerError>
    where
        Get: Layer,
        Post: Layer,
    {
        let post = uri_map(&self.post);
        let get = uri_map(&self.get);
        let ex = LocalExecutor::new();
        ex.run(Self::listen_(socket, &post, &get, &ex)).await
    }

    /// Implementation of listener.
    ///
    /// # Errors
    /// On fatal unix socket listener errors.
    #[expect(clippy::future_not_send, reason = "not intended to be sent")]
    async fn listen_<'a: 'b, 'b>(
        socket: UnixListener,
        post: &'a UriMap<'a>,
        get: &'a UriMap<'a>,
        ex: &LocalExecutor<'b>,
    ) -> Result<Infallible, ListenerError> {
        loop {
            let (mut stream, _) = socket.accept().await.map_err(ListenerError::Accept)?;

            ex.spawn(async move {
                let io = FuturesIo::new(&mut stream);
                let serve = http1::Builder::new().serve_connection(
                    io,
                    service_fn(|req| async move {
                        let incoming = IncomingRequest { inner: req };
                        let method = incoming.method();
                        let uri = incoming.uri_path();

                        if method.is_get() {
                            if let Some(listener) = get.get(uri) {
                                match incoming.body().await.and_then(listener) {
                                    Ok(body) => Response::builder()
                                        .status(StatusCode::OK)
                                        .body(Full::new(body)),
                                    Err(err) => err.into_http(),
                                }
                            } else {
                                ResponseCode::NotFound
                                    .with_err(format!("could not find get resource {uri}"))
                                    .into_http()
                            }
                        } else if method.is_post() {
                            if let Some(listener) = post.get(uri) {
                                match incoming.body().await.and_then(listener) {
                                    Ok(body) => Response::builder()
                                        .status(StatusCode::OK)
                                        .body(Full::new(body)),
                                    Err(err) => err.into_http(),
                                }
                            } else {
                                ResponseCode::NotFound
                                    .with_err(format!("could not find post resource {uri}"))
                                    .into_http()
                            }
                        } else {
                            HttpResponse::from(ResponseCode::MethodNotAllowed).into_http()
                        }
                    }),
                );

                if let Err(err) = serve.await {
                    ::log::error!("error serving http connection\n{err}");
                }
            })
            .detach();
        }
    }
}

/// Uri mapping.
type UriMap<'a> = HashMap<Cow<'static, str>, &'a dyn Fn(Bytes) -> Result<Bytes, HttpResponse>>;

/// Map a layer stack to a hash map.
fn uri_map(layer: &dyn Layer) -> UriMap<'_> {
    let mut mapping = HashMap::new();

    let mut layer: &dyn Sealed = layer;
    while let Some(uri) = layer.uri() {
        mapping.insert(uri, layer.listener());
        layer = layer.prior();
    }

    mapping
}

/// A layer of a typed listener.
pub trait Layer: Sealed {}

impl<T> Layer for T where T: Sealed {}

/// A layer in the listener stack.
pub trait Sealed {
    /// Get prior layer.
    fn prior(&self) -> &'_ dyn Sealed;

    /// Get uri of layer.
    fn uri(&self) -> Option<Cow<'static, str>>;

    /// Get listener of layer.
    fn listener(&self) -> &'_ dyn Fn(Bytes) -> Result<Bytes, HttpResponse>;
}

impl<T: Sealed> Sealed for &T {
    fn prior(&self) -> &'_ dyn Sealed {
        T::prior(self)
    }

    fn uri(&self) -> Option<Cow<'static, str>> {
        T::uri(self)
    }

    fn listener(&self) -> &'_ dyn Fn(Bytes) -> Result<Bytes, HttpResponse> {
        T::listener(self)
    }
}

/// Initial layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Initial;

impl Sealed for Initial {
    fn prior(&self) -> &'_ dyn Sealed {
        unreachable!("prior should never be called for the initial layer")
    }

    fn uri(&self) -> Option<Cow<'static, str>> {
        None
    }

    fn listener(&self) -> &'_ dyn Fn(Bytes) -> Result<Bytes, HttpResponse> {
        unreachable!("listener should never be called for the initial layer")
    }
}

/// Chain layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Chain<P, L, U> {
    /// Prior layer in chain.
    pub prior: P,
    /// Listener of chain.
    pub listener: L,
    /// Get uri.
    pub uri: U,
}

impl<P, L, U> Sealed for Chain<P, L, U>
where
    L: Fn(Bytes) -> Result<Bytes, HttpResponse>,
    U: Fn() -> Cow<'static, str>,
    P: Sealed,
{
    fn prior(&self) -> &'_ dyn Sealed {
        &self.prior
    }

    fn uri(&self) -> Option<Cow<'static, str>> {
        Some((self.uri)())
    }

    fn listener(&self) -> &'_ dyn Fn(Bytes) -> Result<Bytes, HttpResponse> {
        &self.listener
    }
}
