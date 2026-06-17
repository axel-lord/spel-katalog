//! Environment available for settings.
use ::core::fmt::Display;
use ::std::fmt;

use ::rustix::{fs::Uid, process::getuid};
use ::spel_katalog_common::lazy::Lazy;

/// Displayable user, may be either a numeric id or a username.
#[derive(Debug)]
enum User {
    /// Display a user id.
    Id(Uid),
    /// Display a username.
    Name(String),
}

impl User {
    /// Get a User for current process.
    fn current() -> Self {
        ::whoami::username()
            .map(Self::Name)
            .unwrap_or_else(|_| Self::Id(getuid()))
    }
}

impl Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            User::Id(uid) => Display::fmt(uid, f),
            User::Name(name) => Display::fmt(name, f),
        }
    }
}

/// Remove trailing newlines.
fn remove_trailing(mut s: String) -> String {
    let len = s.trim_end_matches('/').len();
    s.truncate(len);
    s
}

/// Value for HOME.
fn home() -> String {
    ::std::env::var("HOME").map_or_else(
        |err| {
            ::log::warn!("could not get home directory, {err}");
            format!("/tmp/spel-katalog.{}", User::current())
        },
        remove_trailing,
    )
}

/// Value fo CONFIG.
fn config() -> String {
    ::std::env::var("XDG_CONFIG_HOME").map_or_else(
        |err| {
            ::log::warn!("could not get config directory, {err}");
            format!("{HOME}/.config")
        },
        remove_trailing,
    )
}

/// Value for CACHE.
fn cache() -> String {
    ::std::env::var("XDG_CACHE_HOME").map_or_else(
        |err| {
            ::log::warn!("could not get cache directory, {err}");
            format!("{HOME}/.cache")
        },
        remove_trailing,
    )
}

/// Value for DATA.
fn data() -> String {
    ::std::env::var("XDG_DATA_HOME").map_or_else(
        |err| {
            ::log::warn!("could not get data directory, {err}");
            format!("{HOME}/.local/share")
        },
        remove_trailing,
    )
}

/// Value for STATE.
fn state() -> String {
    ::std::env::var("XDG_STATE_HOME").map_or_else(
        |err| {
            ::log::warn!("could not get state directory, {err}");
            format!("{HOME}/.local/state")
        },
        remove_trailing,
    )
}

/// User home directory.
pub static HOME: Lazy = Lazy::new(home);

/// User config directory, defaults to `~/.config`.
pub static CONFIG: Lazy = Lazy::new(config);

/// User config directory, defaults to `~/.cache`.
pub static CACHE: Lazy = Lazy::new(cache);

/// User config directory, defaults to `~/.local/share`.
pub static DATA: Lazy = Lazy::new(data);

/// User config directory, defaults to `~/.local/state`.
pub static STATE: Lazy = Lazy::new(state);

