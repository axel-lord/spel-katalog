//! [Chain] impl.

use ::std::borrow::Cow;

use ::bytes::Bytes;

use crate::{http::HttpResponse, typed::layer::Sealed};

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
