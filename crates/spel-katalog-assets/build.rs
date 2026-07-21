//! Asset build script.

use ::std::{io::Write, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=assets");

    let out_path = PathBuf::from(::std::env::var_os("OUT_DIR").expect("OUT_DIR should exist"))
        .join("assets.rs");

    let mut output = ::std::fs::File::create(&out_path)
        .unwrap_or_else(|err| panic!("should be able to create {out_path:?}\n{err}"));

    for entry in ::std::fs::read_dir("./assets")
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
    {
        let Ok(filetype) = entry.file_type() else {
            continue;
        };
        if !filetype.is_file() {
            continue;
        }
        let path = entry.path();
        let path = path
            .canonicalize()
            .unwrap_or_else(|err| panic!("could not canonicalize {path:?}\n{err}"));
        let Some(name) = path.file_stem().and_then(|f| f.to_str()) else {
            continue;
        };
        let file_path = path.to_string_lossy();

        writeln!(output, r##"
        #[doc = "Get an svg handle to `{name}`"]
        pub fn {name}() -> &'static ::iced_core::svg::Handle {{
            static INNER: ::std::sync::OnceLock<::iced_core::svg::Handle> = ::std::sync::OnceLock::new();
            INNER.get_or_init(|| {{
                ::iced_core::svg::Handle::from_memory(::std::borrow::Cow::Borrowed(include_bytes!("{file_path}").as_slice()))
            }})
        }}
        "##).expect("should be able to write to assets.rs");
    }

    output.flush().expect("should be able to flush assets.rs");
}
