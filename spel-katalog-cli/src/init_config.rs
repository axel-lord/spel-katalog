//! Function to initialize config.

use ::std::{io::Write, path::PathBuf};

use ::tap::Pipe;

pub fn init_config(path: PathBuf, skip_lua_update: bool) {
    let lib_path = path.join("lib");
    let games_path = path.join("games");
    let batch_path = path.join("batch");
    let scripts_path = path.join("scripts");

    [&path, &lib_path, &games_path, &batch_path, &scripts_path]
        .into_iter()
        .for_each(create_dir_if_missing);

    let config_path = path.join("config.toml");
    create_file_if_missing(&config_path, "");

    let def_path = lib_path.join("spel-katalog.lua");
    let lua_rc_path = path.join(".luarc.json");

    if skip_lua_update {
        create_file_if_missing(&def_path, LUA_DEF);
        create_file_if_missing(&lua_rc_path, LUARC);
    } else {
        update_file(&def_path, LUA_DEF);
        update_file(&lua_rc_path, LUARC);
    }
}

const LUARC: &str = include_str!("../../lua/luarc.template.json");
const LUA_DEF: &str = include_str!("../../lua/spel-katalog.lua");

fn create_dir_if_missing(path: &PathBuf) {
    match ::std::fs::create_dir(path).map_err(|err| (err.kind(), err)) {
        Ok(_) => println!("created directory {path:?}"),
        Err((::std::io::ErrorKind::AlreadyExists, _)) => {
            println!("directory {path:?} already exists");
        }
        Err((_, err)) => {
            eprintln!("could not create directory {path:?}, {err}");
        }
    }
}

fn create_file_if_missing(path: &PathBuf, content: &str) {
    match ::std::fs::File::create_new(&path).map_err(|err| (err.kind(), err)) {
        Ok(file) => {
            println!("created file {path:?}");

            if !content.is_empty() {
                let result = file.pipe(|mut file| {
                    file.write_all(content.as_bytes())?;
                    file.flush()
                });

                if let Err(err) = result {
                    eprintln!("could not write default content to {path:?}, {err}")
                } else {
                    println!("wrote default content to {path:?}")
                }
            }
        }
        Err((::std::io::ErrorKind::AlreadyExists, _)) => {
            println!("file {path:?} already exists");
        }
        Err((_, err)) => {
            eprintln!("could not create file {path:?}, {err}");
        }
    }
}

fn update_file(path: &PathBuf, content: &str) {
    let Ok(current_content) = ::std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        ::std::io::ErrorKind::NotFound => {
            create_file_if_missing(&path, content);
        }
        _ => {
            eprintln!("could not read {path:?}, {err}")
        }
    }) else {
        return;
    };

    if current_content == content {
        println!("no need to update {path:?}");
    } else if let Err(err) = ::std::fs::write(path, content.as_bytes()) {
        eprintln!("could not update {path:?}, {err}");
    } else {
        println!("updated {path:?}");
    }
}
