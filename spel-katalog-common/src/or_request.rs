//! [OrRequest] impl.

use ::derive_more::IsVariant;

/// Either a message or a request.
#[derive(Debug, Clone, Copy, IsVariant)]
pub enum OrRequest<M, R> {
    /// This is a message that should be returned.
    Message(M),
    /// This is a request to parent.
    Request(R),
}

impl<M, R> OrRequest<M, R> {
    /// Map the `Request` variant.
    pub fn map_request<T>(self, f: impl FnOnce(R) -> T) -> OrRequest<M, T> {
        match self {
            OrRequest::Message(m) => OrRequest::Message(m),
            OrRequest::Request(r) => OrRequest::Request(f(r)),
        }
    }

    /// Map the `Message` variant.
    pub fn map_message<T>(self, f: impl FnOnce(M) -> T) -> OrRequest<T, R> {
        match self {
            OrRequest::Message(m) => OrRequest::Message(f(m)),
            OrRequest::Request(r) => OrRequest::Request(r),
        }
    }

    /// Use the provided functions to unwrap either of the vraiants.
    pub fn unwrap_with<T>(self, message: impl FnOnce(M) -> T, request: impl FnOnce(R) -> T) -> T {
        match self {
            OrRequest::Message(m) => message(m),
            OrRequest::Request(r) => request(r),
        }
    }
}

impl<M, R> From<M> for OrRequest<M, R> {
    fn from(value: M) -> Self {
        Self::Message(value)
    }
}

/// Trait used to convert any value into [OrRequest::Message] or [OrRequest::Request].
pub trait IntoOrRequest {
    /// Convert into the [message][OrRequest::Message] variant of an [OrRequest].
    #[inline]
    fn into_message<R>(self) -> OrRequest<Self, R>
    where
        Self: Sized,
    {
        OrRequest::Message(self)
    }

    /// Convert into the [request][OrRequest::Request] variant of an [OrRequest].
    #[inline]
    fn into_request<M>(self) -> OrRequest<M, Self>
    where
        Self: Sized,
    {
        OrRequest::Request(self)
    }
}

impl<T> IntoOrRequest for T {}
