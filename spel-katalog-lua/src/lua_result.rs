use ::mlua::IntoLuaMulti;

/// A result which may be converted to a lua value of `T | nil, E`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LuaResult<T, E> {
    /// Has a valid value.
    Ok(T),
    /// Is an error.
    Err(E),
}

impl<T, E> IntoLuaMulti for LuaResult<T, E>
where
    T: IntoLuaMulti,
    E: IntoLuaMulti,
{
    #[inline]
    fn into_lua_multi(self, lua: &mlua::Lua) -> mlua::Result<mlua::MultiValue> {
        match self {
            LuaResult::Ok(value) => value.into_lua_multi(lua),
            LuaResult::Err(err) => (::mlua::Value::Nil, err).into_lua_multi(lua),
        }
    }
}

impl<T, E> From<Result<T, E>> for LuaResult<T, E> {
    #[inline]
    fn from(value: Result<T, E>) -> Self {
        LuaResult::from_result(value)
    }
}

impl<T, E> From<LuaResult<T, E>> for Result<T, E> {
    #[inline]
    fn from(value: LuaResult<T, E>) -> Self {
        value.into_result()
    }
}

impl<T, E> LuaResult<T, E> {
    /// Wrap into an regular result ok.
    #[inline]
    pub fn wrap_ok<E2>(self) -> Result<Self, E2> {
        Ok(self)
    }

    /// Get a new `LuaResult` with variants as references.
    #[inline]
    pub fn as_ref(&self) -> LuaResult<&T, &E> {
        match self {
            LuaResult::Ok(value) => LuaResult::Ok(value),
            LuaResult::Err(err) => LuaResult::Err(err),
        }
    }

    /// Get a new `LuaResult` with variants as mutable references.
    #[inline]
    pub fn as_mut(&mut self) -> LuaResult<&mut T, &mut E> {
        match self {
            LuaResult::Ok(value) => LuaResult::Ok(value),
            LuaResult::Err(err) => LuaResult::Err(err),
        }
    }

    /// Create from a standard library result type.
    #[inline]
    pub fn from_result(result: Result<T, E>) -> Self {
        result.map_or_else(Self::Err, Self::Ok)
    }

    /// Convert to a standard library result type.
    #[inline]
    pub fn into_result(self) -> Result<T, E> {
        match self {
            LuaResult::Ok(value) => Ok(value),
            LuaResult::Err(err) => Err(err),
        }
    }

    /// Convert the error variant using a `ToString` implementation.
    #[inline]
    pub fn display_err(self) -> LuaResult<T, String>
    where
        E: ToString,
    {
        match self {
            LuaResult::Ok(value) => LuaResult::Ok(value),
            LuaResult::Err(err) => LuaResult::Err(err.to_string()),
        }
    }
}
