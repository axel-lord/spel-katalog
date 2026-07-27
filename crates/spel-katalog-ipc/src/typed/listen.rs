//! Hidden utilities used by typed listener.

use ::core::convert::Infallible;

use ::bytes::Bytes;
use ::http_body_util::Full;
use ::hyper::{Response, StatusCode, server::conn::http1, service::service_fn};
use ::smol::{LocalExecutor, net::unix::UnixListener};
use ::smol_hyper::rt::FuturesIo;
use ::uuid::Uuid;

use crate::{
    IncomingRequest,
    http::{HttpResponse, ResponseCode},
    typed::{
        Layer, ListenerError,
        chain::Chain,
        layer::{Initial, UriMap},
    },
};

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
        let post = self.post.uri_map();
        let get = self.get.uri_map();
        let ex = LocalExecutor::new();
        ex.run(listen(socket, &post, &get, &ex)).await
    }
}

/// Implementation of listener.
///
/// # Errors
/// On fatal unix socket listener errors.
#[expect(clippy::future_not_send, reason = "not intended to be sent")]
async fn listen<'a: 'b, 'b>(
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
