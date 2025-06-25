//! Extra scripts to run for all or a subset of games.

pub mod dependency;
pub mod environment;
pub mod exec;
pub mod script;
pub mod script_file;

mod builder_push;

#[cfg(test)]
mod tests {
    use crate::{
        dependency::{Dependency, DependencyKind},
        exec::{Cmd, Program},
        script::Script,
        script_file::ScriptFile,
    };

    use ::pretty_assertions::assert_eq;

    #[test]
    fn cmd_script() -> ::color_eyre::Result<()> {
        let expected = ScriptFile::builder()
            .script(
                Script::builder("Exp-v2")
                    .exec(Cmd::from(vec![
                        String::from("echo"),
                        String::from("Hello world!"),
                    ]))
                    .build(),
            )
            .build();

        let toml = r#"
        [script]
        id = "Exp-v2"
        cmd = "echo 'Hello world!'"
        "#;

        assert_eq!(ScriptFile::from_toml(toml)?, expected.clone());
        Ok(())
    }

    #[test]
    fn simple_script() -> ::color_eyre::Result<()> {
        let expected = ScriptFile::builder()
            .script(
                Script::builder("Exp-v1")
                    .exec(Program::builder("script.sh").build())
                    .build(),
            )
            .require(
                Dependency::builder()
                    .kind(DependencyKind::id("PreExp"))
                    .try_dep(true)
                    .build(),
            )
            .build();

        let toml = r#"
        [script]
        id = "Exp-v1"
        exec = "script.sh"

        [[require]]
        id = "PreExp"
        try = true
        "#;

        assert_eq!(ScriptFile::from_toml(toml)?, expected.clone());

        let json = r#"
        {
            "script": {
                "id": "Exp-v1",
                "exec": "script.sh"
            },
            "require": [
                { "id": "PreExp", "try": true }
            ]
        }
        "#;

        assert_eq!(ScriptFile::from_json(json)?, expected);

        Ok(())
    }
}
