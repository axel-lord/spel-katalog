//! Game installation cli to ipc message.

use ::std::collections::HashMap;

use ::color_eyre::{Result, Section, eyre::eyre};
use ::spel_katalog_cli::InstallGame;
use ::spel_katalog_formats::{Bind, InstallerConfig};

/// Send ipc message to install game.
///
/// # Errors
/// If message cannot be sent, or on misconfiguration.
pub fn install_game(
    InstallGame {
        game,
        thumbnail,
        hidden,
        no_move,
        exe,
        installer_dir,
        dummy,
    }: InstallGame,
) -> Result<()> {
    let base_dirs = ::xdg::BaseDirectories::with_prefix("spel-katalog");
    let drives = installer_dir
        .as_ref()
        .map(|d| HashMap::from_iter([('i', d.clone())]))
        .unwrap_or_default();
    let ro_bind = installer_dir
        .map(|d| Vec::from_iter([Bind::mirrored(d)]))
        .unwrap_or_default();
    let game_dir = game
        .canonicalize()
        .map_err(|err| eyre!(err).note(format!("is {game:?} a valid path?")))?;
    let exe = if !dummy {
        exe
    } else {
        Some(game_dir.join("dummy.exe"))
    };

    if let Err(err) = ::spel_katalog_ipc::send(
        &base_dirs,
        ::spel_katalog_ipc::Message::InstallGame(InstallerConfig {
            game_dir,
            exe,
            hidden: Some(hidden),
            thumbnail: thumbnail
                .map(|t| {
                    t.canonicalize()
                        .map_err(|err| eyre!(err).note(format!("is {t:?} a valid path?")))
                })
                .transpose()?,
            move_game: Some(!no_move),
            drives,
            bind: Default::default(),
            ro_bind,
            env: Default::default(),
        }),
    ) {
        Err(eyre!(err).note("is the application open?"))
    } else {
        Ok(())
    }
}
