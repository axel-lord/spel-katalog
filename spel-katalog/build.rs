use ::std::{env, path::Path};

use ::spel_katalog_build::{format::Settings, settings};

fn main() {
    println!("cargo::rerun-if-changed=src/settings.toml");
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir);

    settings::write(
        Settings::read("src/settings.toml".as_ref()),
        &out_dir.join("settings.rs"),
    );
}
