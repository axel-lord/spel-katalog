//! Types used to help resolving incoming requests.

use ::core::marker::PhantomData;

use ::bytes::Bytes;
use ::tap::Pipe;

use crate::{
    IncomingRequest,
    http::{HttpResponse, ResponseCode},
    typed,
};

pub mod kind {
    //! Resolver kinds.

    /// Resolver resolves methods.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Method {}

    /// Resolver resolves post requests.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Post {}

    /// Resolver resolves get request.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum Get {}
}

/// Enum used to handle requests,
/// dividing by mythod and uri.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "the larger variant is the most common, and the smaller is not expected to exist for long"
)]
enum Resolver_ {
    /// Request is active.
    Active {
        /// Current request.
        incoming: IncomingRequest,
    },
    /// Reqest has been resolved.
    Resolved(Result<Bytes, HttpResponse>),
}

/// Resolve incoming requests
/// using a type based api.
#[derive(Debug)]
pub struct Resolver<T> {
    /// Wrapped resolver.
    inner: Resolver_,

    /// Allow T.
    _p: PhantomData<fn() -> T>,
}

impl<T> Resolver<T> {
    /// Construct a new resolver.
    const fn with_incoming(incoming: IncomingRequest) -> Self {
        Self::with_inner(Resolver_::Active { incoming })
    }

    /// Construct a new resolver from inner value.
    const fn with_inner(inner: Resolver_) -> Self {
        Self {
            inner,
            _p: PhantomData,
        }
    }

    /// Construct a new resolver from a resolved request.
    const fn with_resolved(resolved: Result<Bytes, HttpResponse>) -> Self {
        Self::with_inner(Resolver_::Resolved(resolved))
    }

    /// Construct a new resolver from an http response.
    const fn with_response(response: HttpResponse) -> Self {
        Self::with_inner(Resolver_::Resolved(Err(response)))
    }
}

/// Resolve a post request.
async fn resolve_post<M: typed::Post>(
    incoming: IncomingRequest,
    resolver: impl AsyncFnOnce(M) -> Result<M::Response, HttpResponse>,
) -> Result<Bytes, HttpResponse> {
    let body = incoming.body().await?;
    let message = ::serde_json::from_slice::<M>(&body)?;
    let response = resolver(message).await?;
    ::serde_json::to_vec(&response)?
        .pipe(Bytes::from_owner)
        .pipe(Ok)
}

/// Resolve a get request.
async fn resolve_get<T: typed::Get>(
    resolver: impl AsyncFnOnce() -> Result<T, HttpResponse>,
) -> Result<Bytes, HttpResponse> {
    let response = resolver().await?;
    ::serde_json::to_vec(&response)?
        .pipe(Bytes::from_owner)
        .pipe(Ok)
}

impl Resolver<kind::Post> {
    /// Add resource to post resolver.
    pub async fn resource<M>(
        self,
        resolver: impl AsyncFnOnce(M) -> Result<M::Response, HttpResponse>,
    ) -> Self
    where
        M: typed::Post,
    {
        match self.inner {
            Resolver_::Active { incoming } if incoming.uri_path() == M::safe_uri() => {
                Self::with_resolved(resolve_post(incoming, resolver).await)
            }
            inner => Self::with_inner(inner),
        }
    }
}

impl Resolver<kind::Get> {
    /// Add resource to get resolver.
    pub async fn resource<T>(self, resolver: impl AsyncFnOnce() -> Result<T, HttpResponse>) -> Self
    where
        T: typed::Get,
    {
        match self.inner {
            Resolver_::Active { incoming } if incoming.uri_path() == T::safe_uri() => {
                Self::with_resolved(resolve_get(resolver).await)
            }
            inner => Self::with_inner(inner),
        }
    }
}

impl Resolver<kind::Method> {
    /// Construct a new method resolver.
    pub const fn new(incoming: IncomingRequest) -> Self {
        Self::with_incoming(incoming)
    }

    /// Resolve post methods.
    pub async fn post(
        self,
        on_post: impl AsyncFnOnce(Resolver<kind::Post>) -> Resolver<kind::Post>,
    ) -> Self {
        match self.inner {
            Resolver_::Active { incoming } if incoming.method().is_post() => {
                let resolver = Resolver::with_incoming(incoming);
                match on_post(resolver).await.inner {
                    Resolver_::Active { incoming } => ResponseCode::NotFound
                        .with_err(format!("could not post resource {}", incoming.uri_path()))
                        .pipe(Self::with_response),
                    resolved @ Resolver_::Resolved(..) => Self::with_inner(resolved),
                }
            }
            inner => Self::with_inner(inner),
        }
    }

    /// Resolve get methods.
    pub async fn get(
        self,
        on_post: impl AsyncFnOnce(Resolver<kind::Get>) -> Resolver<kind::Get>,
    ) -> Self {
        match self.inner {
            Resolver_::Active { incoming } if incoming.method().is_get() => {
                let resolver = Resolver::with_incoming(incoming);
                match on_post(resolver).await.inner {
                    Resolver_::Active { incoming } => ResponseCode::NotFound
                        .with_err(format!("could not get resource {}", incoming.uri_path()))
                        .pipe(Self::with_response),
                    resolved @ Resolver_::Resolved(..) => Self::with_inner(resolved),
                }
            }
            inner => Self::with_inner(inner),
        }
    }

    /// Finish resolving methods.
    ///
    /// # Errors
    /// If any resolver usage errored.
    /// Or if no method was resolved.
    pub async fn finish(self) -> Result<Bytes, HttpResponse> {
        match self.inner {
            Resolver_::Active { incoming } => ResponseCode::MethodNotAllowed
                .with_err(format!(
                    "method {:?} not allowed for requests",
                    incoming.method()
                ))
                .pipe(Err),
            Resolver_::Resolved(bytes) => bytes,
        }
    }
}
