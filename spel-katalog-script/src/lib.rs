//! Extra scripts to run for all or a subset of games.

pub mod dependency;
pub mod environment;
pub mod exec;
pub mod maybe_single;
pub mod script;
pub mod script_file;
pub mod string_visitor;

mod builder_push;

#[doc(inline)]
pub use crate::script_file::RunError as Error;

#[doc(inline)]
pub use crate::script_file::ReadError;

#[cfg(test)]
mod tests {
    use crate::{
        dependency::{Dependency, DependencyKind, DependencyResult},
        exec::{Cmd, Program},
        script::Script,
        script_file::ScriptFile,
    };

    use ::pretty_assertions::assert_eq;
    use ::spel_katalog_terminal::SinkBuilder;

    type Result = ::core::result::Result<(), ::color_eyre::Report>;

    #[tokio::test]
    async fn dep_multiple() -> Result {
        let scripts = [
            r#"
            [script]
            id = "1"

            [[require]]
            value = "hello"
            in = ["hello", "world"]
            "#,
            r#"
            [script]
            id = "2"

            [[require]]
            script = "1"

            [[require]]
            equals = ["hello", "hi"]
            try = true
            "#,
            r#"
            [script]
            id = "3"

            [[require]]
            script = "1"

            [[require]]
            script = "2"
            try = true
            "#,
            r#"
            [script]
            id = "4"

            [[require]]
            script = "1"
            "#,
        ]
        .into_iter()
        .map(ScriptFile::from_toml)
        .collect::<::core::result::Result<Vec<_>, _>>()?;

        let (results, _) = ScriptFile::pre_run_check(&scripts, &SinkBuilder::Inherit).await?;

        assert_eq!(results.get("1").copied(), Some(DependencyResult::Success));
        assert_eq!(results.get("2").copied(), Some(DependencyResult::Failure));
        assert_eq!(results.get("3").copied(), Some(DependencyResult::Failure));
        assert_eq!(results.get("4").copied(), Some(DependencyResult::Success));

        Ok(())
    }

    #[tokio::test]
    async fn dep_match() -> Result {
        let toml = r#"
        [script]
        id = "1"
        [[require]]
        value = "Hello world!"
        matches = "^He[li].*!$"
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Success
        );

        Ok(())
    }

    #[tokio::test]
    async fn dep_imatch() -> Result {
        let toml = r#"
        [script]
        id = "1"
        [[require]]
        value = "Hello world!"
        imatches = "^hE[li].*!$"
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Success
        );

        Ok(())
    }

    #[tokio::test]
    async fn dep_in() -> Result {
        let toml = r#"
        [script]
        id = "1"
        [[require]]
        values = ["hello"]
        in = ["hello", "world"]
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Success
        );

        let toml = r#"
        [script]
        id = "2"
        [[require]]
        values = ["hello", "world", "hello"]
        in = ["hello", "world", "world"]
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Success
        );

        let toml = r#"
        [script]
        id = "3"
        [[require]]
        values = ["hello", "world"]
        in = ["hello"]
        panic = true
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Panic
        );

        let toml = r#"
        [script]
        id = "3"
        [[require]]
        values = ["hello", "world"]
        in = ["hello"]
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Failure
        );

        let toml = r#"
        [script]
        id = "1"
        [[require]]
        value = "hello"
        in = ["hello", "world"]
        panic = true
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Success
        );

        Ok(())
    }

    #[tokio::test]
    async fn dep_equals() -> Result {
        let toml = r#"
        [script]
        id = "1"
        [[require]]
        equals = ["hello", "hello"]
        panic = true
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Success
        );

        let toml = r#"
        [script]
        id = "2"
        [[require]]
        equals = ["hello", "world"]
        panic = true
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Panic
        );

        let toml = r#"
        [script]
        id = "3"
        [[require]]
        equals = ["hello", "world"]
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Failure
        );

        Ok(())
    }

    #[tokio::test]
    async fn dep_not_equals() -> Result {
        let toml = r#"
        [script]
        id = "1"
        [[require]]
        not-equals = ["hello", "world"]
        panic = true
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Success
        );

        let toml = r#"
        [script]
        id = "2"
        [[require]]
        not-equals = ["hello", "hello"]
        panic = true
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Panic
        );

        let toml = r#"
        [script]
        id = "3"
        [[require]]
        not-equals = ["hello", "hello"]
        "#;

        assert_eq!(
            ScriptFile::from_toml(toml)?
                .check_require(|_| None, &SinkBuilder::Inherit)
                .await?,
            DependencyResult::Failure
        );

        Ok(())
    }

    #[test]
    fn cmd_script() -> Result {
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
    fn simple_script() -> Result {
        let expected = ScriptFile::builder()
            .script(
                Script::builder("Exp-v1")
                    .exec(Program::builder("script.sh").build())
                    .build(),
            )
            .require(
                Dependency::builder()
                    .kind(DependencyKind::script("PreExp"))
                    .build(),
            )
            .build();

        let toml = r#"
        [script]
        id = "Exp-v1"
        exec = "script.sh"

        [[require]]
        script = "PreExp"
        "#;

        assert_eq!(ScriptFile::from_toml(toml)?, expected.clone());

        let json = r#"
        {
            "script": {
                "id": "Exp-v1",
                "exec": "script.sh"
            },
            "require": [
                { "script": "PreExp", "try": true }
            ]
        }
        "#;

        assert_eq!(ScriptFile::from_json(json)?, expected);

        Ok(())
    }
}
