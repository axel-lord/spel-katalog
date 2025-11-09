use ::std::{path::Path, sync::Arc};

use ::color_eyre::eyre::eyre;
use ::rustc_hash::FxHashMap;
use ::spel_katalog_batch::Batcher;
use ::spel_katalog_cli::{Batch, Run};
use ::spel_katalog_settings::{CacheDir, ConfigDir, LutrisDb, YmlDir};
use ::spel_katalog_sink::SinkBuilder;

pub use self::exit_channel::{ExitReceiver, ExitSender, exit_channel};

pub(crate) use self::{
    app::App,
    message::{Message, QuickMessage, Safety},
};

mod app;
mod dialog;
mod exit_channel;
mod message;
mod process_info;
mod run_game;
mod subscription;
mod update;
mod view;

fn get_settings(
    config: &Path,
    overrides: ::spel_katalog_settings::Settings,
) -> ::spel_katalog_settings::Settings {
    fn read_settings(config: &Path) -> Result<::spel_katalog_settings::Settings, ()> {
        let content = ::std::fs::read_to_string(config).map_err(|err| {
            ::log::warn!("could not read {config:?}, does it exists an is it readable?\n{err}");
        })?;

        ::toml::from_str(&content).map_err(|err| {
            ::log::warn!("could not parse {config:?} as toml, is it a toml file?\n{err}")
        })
    }
    read_settings(config)
        .unwrap_or_default()
        .apply(::spel_katalog_settings::Delta::create(overrides))
}

fn get_modules(settings: &::spel_katalog_settings::Settings) -> FxHashMap<String, String> {
    let lib_dir = settings.get::<ConfigDir>().as_path().join("lib");
    ::std::fs::read_dir(&lib_dir)
        .map_err(|err| ::log::error!("could not read directory {lib_dir:?}\n{err}"))
        .ok()
        .into_iter()
        .flat_map(|read_dir| {
            read_dir.filter_map(|entry| {
                let entry = entry
                    .map_err(|err| {
                        ::log::error!("failed to get read_dir entry in {lib_dir:?}\n{err}")
                    })
                    .ok()?;

                let path = entry.path();
                let name = path.file_stem();
                if name.is_none() {
                    ::log::error!("could not get file stem for {path:?}")
                }
                let name = name?;
                let content = ::std::fs::read_to_string(&path)
                    .map_err(|err| {
                        ::log::error!("could not read content of {path:?} to a string\n{err}");
                    })
                    .ok()?;

                Some((name.to_string_lossy().into_owned(), content))
            })
        })
        .collect()
}

/// Run application.
pub fn run(
    run: Run,
    sink_builder: SinkBuilder,
    exit_recv: Option<ExitReceiver>,
) -> ::color_eyre::Result<()> {
    App::run(run, sink_builder, exit_recv)
}

/// Run batch scripts.
pub fn batch_run(batch: Batch) -> ::color_eyre::Result<()> {
    #[derive(Debug)]
    struct Vt {
        settings: ::spel_katalog_settings::Settings,
    }

    impl ::spel_katalog_lua::Virtual for Vt {
        fn open_dialog(
            &self,
            text: String,
            buttons: Vec<String>,
        ) -> ::mlua::Result<Option<String>> {
            println!("{text}");
            for (i, btn) in buttons.iter().enumerate() {
                println!("{i}. {btn}")
            }
            println!("Enter Index of action to take");
            let mut line = String::new();
            ::std::io::stdin()
                .read_line(&mut line)
                .map_err(::mlua::Error::external)?;

            Ok(line
                .trim_end()
                .parse::<usize>()
                .ok()
                .and_then(|idx| buttons.get(idx).cloned()))
        }

        fn available_modules(&self) -> ::rustc_hash::FxHashMap<String, String> {
            get_modules(&self.settings)
        }

        fn thumb_db_path(&self) -> ::mlua::Result<std::path::PathBuf> {
            Ok(self
                .settings
                .get::<CacheDir>()
                .as_path()
                .join("thumbnails.db"))
        }

        fn additional_config_path(&self, game_id: i64) -> ::mlua::Result<std::path::PathBuf> {
            Ok(self
                .settings
                .get::<ConfigDir>()
                .as_path()
                .join(format!("games/{game_id}.toml")))
        }

        fn settings(&self) -> ::mlua::Result<std::collections::HashMap<&'_ str, String>> {
            Ok(self.settings.generic())
        }
    }

    let Batch {
        script,
        settings,
        config,
    } = batch;
    let settings = get_settings(&config, settings);
    let games =
        ::spel_katalog_gather::load_games_from_database(settings.get::<LutrisDb>().as_path())?;
    let batch_infos = crate::update::gather(
        settings.get::<YmlDir>(),
        settings.get::<ConfigDir>(),
        &games,
    );
    drop(games);

    let vt = Vt { settings };

    let batcher = Batcher::new(batch_infos, &SinkBuilder::Inherit, Arc::new(vt))
        .map_err(|err| eyre!(err.to_string()))?;

    for script in script {
        let content = match ::std::fs::read_to_string(&script) {
            Ok(script) => script,
            Err(err) => {
                ::log::error!("could not read {script:?}\n{err}");
                continue;
            }
        };

        if let Err(err) = batcher.run_script(content) {
            ::log::error!("could not execute {script:?}\n{err}");
        }
    }

    Ok(())
}
