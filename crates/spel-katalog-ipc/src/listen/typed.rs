//! Hidden utilities used by typed listener.

use ::std::borrow::Cow;

use ::bytes::Bytes;

use crate::http::HttpResponse;

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
