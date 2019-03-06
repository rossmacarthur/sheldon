use std::io::Write;

use tempfile::NamedTempFile;
use toml;

use sheldon::*;

/// A simple macro to call .into() on each element in a vec! initialization.
macro_rules! vec_into {
    ($($i:expr),*) => (vec![$($i.into()),*]);
}

#[test]
fn config_deserialize_empty() {
    assert_eq!(toml::from_str::<Config>("").unwrap(), Config::new())
}

#[test]
fn config_deserialize_templates_empty() {
    assert_eq!(
        toml::from_str::<Config>("[templates]").unwrap(),
        Config::new()
    )
}

#[test]
fn config_deserialize_global_every() {
    let expected = Config::new()
        .matches(vec_into!["*.zsh"])
        .apply(vec_into!["function"])
        .template("function", "echo {{ filename }}");
    assert_eq!(
        toml::from_str::<Config>(
            r#"
            match = ['*.zsh']
            apply = ['function']

            [templates]
            function = 'echo {{ filename }}'
            "#
        )
        .unwrap(),
        expected
    )
}

#[test]
fn config_deserialize_config_plugins() {
    let expected = Config::new()
        .matches(vec_into!["*.zsh"])
        .apply(vec_into!["function"])
        .template("function", "echo {{ filename }}")
        .plugin("pure", Plugin::new_github("sindresorhus/pure"));
    assert_eq!(
        toml::from_str::<Config>(
            r#"
            match = ['*.zsh']
            apply = ['function']

            [templates]
            function = 'echo {{ filename }}'

            [plugins.pure]
            source = 'github'
            repository = 'sindresorhus/pure'
            "#
        )
        .unwrap(),
        expected
    )
}

#[test]
fn config_from_path_empty() {
    let file = NamedTempFile::new().unwrap();
    assert_eq!(Config::from_path(file.path()).unwrap(), Config::new());
}

#[test]
fn config_from_path() {
    let expected = Config::new().matches(vec_into!["*.zsh"]);
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "match = ['*.zsh']").unwrap();
    assert_eq!(Config::from_path(file.path()).unwrap(), expected);
}
