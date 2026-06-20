//! Environment available for settings.
use ::core::fmt::Display;
use ::std::{fmt, path::PathBuf, sync::OnceLock};

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

/// Convert a path to a string.
fn path_into_string(path: PathBuf) -> Option<String> {
    path.into_os_string().into_string().ok()
}

/// Get base dirs.
fn base_dirs() -> &'static ::xdg::BaseDirectories {
    static BASE_DIRS: OnceLock<::xdg::BaseDirectories> = OnceLock::new();
    BASE_DIRS.get_or_init(::xdg::BaseDirectories::new)
}

/// Value for HOME.
fn home() -> String {
    ::std::env::home_dir()
        .and_then(path_into_string)
        .map_or_else(
            || {
                ::log::warn!("could not get home directory");
                format!("/tmp/spel-katalog.{}", User::current())
            },
            remove_trailing,
        )
}

/// Value fo CONFIG.
fn config() -> String {
    base_dirs()
        .get_config_home()
        .and_then(path_into_string)
        .map_or_else(
            || {
                ::log::warn!("could not get config directory");
                format!("{HOME}/.config")
            },
            remove_trailing,
        )
}

/// Value for CACHE.
fn cache() -> String {
    base_dirs()
        .get_cache_home()
        .and_then(path_into_string)
        .map_or_else(
            || {
                ::log::warn!("could not get cache directory");
                format!("{HOME}/.cache")
            },
            remove_trailing,
        )
}

/// Value for DATA.
fn data() -> String {
    base_dirs()
        .get_data_home()
        .and_then(path_into_string)
        .map_or_else(
            || {
                ::log::warn!("could not get data directory");
                format!("{HOME}/.local/share")
            },
            remove_trailing,
        )
}

/// Value for STATE.
fn state() -> String {
    base_dirs()
        .get_state_home()
        .and_then(path_into_string)
        .map_or_else(
            || {
                ::log::warn!("could not get state directory");
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
