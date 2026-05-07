use ::std::{path::PathBuf, sync::Arc};

use ::mlua::Lua;
use ::smol::stream::StreamExt as _;
use ::spel_katalog_batch::BatchInfo;
use ::spel_katalog_formats::{AdditionalConfig, Runner};
use ::spel_katalog_sink::SinkBuilder;

use crate::{
    app::LuaVt,
    run_game::{LuaError, ScriptGatherError},
};

#[derive(Debug, Clone, Copy)]
pub struct BatchView<'a> {
    pub id: i64,
    pub slug: &'a str,
    pub name: &'a str,
    pub runner: &'a Runner,
    pub config: &'a str,
    pub extra: Option<&'a AdditionalConfig>,
    pub hidden: bool,
}

impl From<BatchView<'_>> for BatchInfo {
    fn from(
        BatchView {
            id,
            slug,
            name,
            runner,
            config,
            extra,
            hidden,
        }: BatchView,
    ) -> Self {
        BatchInfo {
            id,
            slug: slug.to_owned(),
            name: name.to_owned(),
            runner: runner.to_string(),
            config: config.to_owned(),
            attrs: extra
                .map(|extra_config| extra_config.attrs.clone())
                .unwrap_or_default(),
            hidden,
        }
    }
}

pub async fn gather_scripts(script_dir: PathBuf) -> Result<Vec<PathBuf>, ScriptGatherError> {
    if !script_dir.exists() {
        ::log::info!("no script dir, skipping");
        return Ok(Vec::new());
    }
    let mut lua_scripts = Vec::new();
    let mut stack = Vec::new();

    stack.push(script_dir);

    while let Some(dir) = stack.pop() {
        let mut dir = ::smol::fs::read_dir(dir).await?;
        while let Some(entry) = dir.next().await.transpose()? {
            let ft = entry.file_type().await?;

            let path = entry.path();

            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("lua"))
                {
                    lua_scripts.push(path);
                }
            } else {
                ::log::warn!("non file or directory path in script dir, {path:?}")
            }
        }
    }

    lua_scripts.sort_unstable();

    Ok(lua_scripts)
}

pub async fn run_script(
    script_dir: PathBuf,
    batch_view: BatchView<'_>,
    sink_builder: &SinkBuilder,
    lua_vt: Arc<LuaVt>,
) -> Result<(), ScriptGatherError> {
    let lua_scripts = gather_scripts(script_dir).await?;

    if !lua_scripts.is_empty() {
        let batch_info = BatchInfo::from(batch_view);

        let sink_builder = sink_builder.clone();
        ::smol::unblock(move || {
            let scripts = lua_scripts
                .into_iter()
                .map(|path| match ::std::fs::read_to_string(&path) {
                    Ok(content) => Ok(content),
                    Err(err) => Err(ScriptGatherError::ReadLuaScript(err, path)),
                })
                .collect::<Result<Vec<_>, _>>()?;

            let lua = Lua::new();
            ::spel_katalog_lua::Module {
                sink_builder: &sink_builder,
                vt: lua_vt,
            }
            .register(&lua)
            .and_then(|skeleton| {
                let module = &skeleton.module;
                let game = batch_info.to_lua(&lua, &skeleton.game_data)?;

                module.set("game", game)?;

                for script in scripts {
                    lua.load(script).exec()?;
                }

                Ok(())
            })
            .map_err(|err| LuaError(err.to_string()))?;
            Ok::<_, ScriptGatherError>(())
        })
        .await?;
    }

    Ok(())
}

