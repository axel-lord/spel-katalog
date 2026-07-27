//! [Layer] impl.

use ::std::{borrow::Cow, collections::HashMap};

use ::bytes::Bytes;

use crate::http::HttpResponse;

/// Uri mapping.
pub type UriMap<'a> = HashMap<Cow<'static, str>, &'a dyn Fn(Bytes) -> Result<Bytes, HttpResponse>>;

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

    /// Get a uri listener mapping.
    fn uri_map(&self) -> UriMap<'_>
    where
        Self: Sized,
    {
        uri_map(self)
    }
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
