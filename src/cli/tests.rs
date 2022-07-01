use super::*;

use std::iter;

use pretty_assertions::assert_eq;
use serde::Serialize;

use crate::cli::color_choice::ColorChoice;

fn setup() {
    for (k, _) in env::vars() {
        if k.starts_with(&format!("{}_", build::CRATE_NAME.to_uppercase())) || k.starts_with("XDG_")
        {
            env::remove_var(k);
        }
    }
}

fn raw_opt(args: &[&str]) -> RawOpt {
    RawOpt::try_parse_from(iter::once(build::CRATE_NAME).chain(args.iter().copied())).unwrap()
}

fn raw_opt_err(args: &[&str]) -> clap::Error {
    RawOpt::try_parse_from(iter::once(build::CRATE_NAME).chain(args.iter().copied())).unwrap_err()
}

#[test]
fn raw_opt_version() {
    setup();
    let err = raw_opt_err(&["-V"]);
    assert_eq!(
        err.to_string(),
        format!("sheldon {}\n", &*build::CRATE_RELEASE)
    );
    assert_eq!(err.kind, clap::ErrorKind::DisplayVersion);
}

#[test]
fn raw_opt_long_version() {
    setup();
    let err = raw_opt_err(&["--version"]);
    assert_eq!(
        err.to_string(),
        format!("sheldon {}\n", &*build::CRATE_LONG_VERSION)
    );
    assert_eq!(err.kind, clap::ErrorKind::DisplayVersion);
}

#[test]
fn raw_opt_help() {
    setup();

    #[derive(Serialize)]
    struct Context {
        version: &'static str,
    }

    let ctx = Context {
        version: build::CRATE_RELEASE,
    };

    for opt in &["-h", "--help"] {
        let err = raw_opt_err(&[opt]);
        goldie::assert_template!(&ctx, err.to_string());
        assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
    }
}

#[test]
fn raw_opt_no_options() {
    setup();
    assert_eq!(
        raw_opt(&["lock"]),
        RawOpt {
            quiet: false,
            verbose: false,
            color: Default::default(),
            home: None,
            config_dir: None,
            data_dir: None,
            config_file: None,
            lock_file: None,
            clone_dir: None,
            download_dir: None,
            command: RawCommand::Lock {
                update: false,
                reinstall: false
            },
        }
    );
}

#[test]
fn raw_opt_options() {
    setup();
    assert_eq!(
        raw_opt(&[
            "--quiet",
            "--verbose",
            "--color",
            "never",
            "--home",
            "/",
            "--config-dir",
            "/test",
            "--data-dir",
            "/test",
            "--config-file",
            "/plugins.toml",
            "--lock-file",
            "/test/plugins.lock",
            "--clone-dir",
            "/repos",
            "--download-dir",
            "/downloads",
            "lock",
        ]),
        RawOpt {
            quiet: true,
            verbose: true,
            color: ColorChoice::Never,
            home: Some("/".into()),
            config_dir: Some("/test".into()),
            data_dir: Some("/test".into()),
            config_file: Some("/plugins.toml".into()),
            lock_file: Some("/test/plugins.lock".into()),
            clone_dir: Some("/repos".into()),
            download_dir: Some("/downloads".into()),
            command: RawCommand::Lock {
                update: false,
                reinstall: false
            },
        }
    );
}

#[test]
fn raw_opt_subcommand_required() {
    setup();
    let err = raw_opt_err(&[]);
    goldie::assert!(err.to_string());
    assert_eq!(err.kind, clap::ErrorKind::MissingSubcommand);
}

#[test]
fn raw_opt_init_help() {
    setup();
    let err = raw_opt_err(&["init", "--help"]);
    goldie::assert!(err.to_string());
    assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
}

#[test]
fn raw_opt_init_with_invalid_shell() {
    setup();
    assert_eq!(
        raw_opt_err(&["init", "--shell", "ksh",]).kind,
        clap::ErrorKind::ValueValidation
    );
}

#[test]
fn raw_opt_add_help() {
    setup();
    let err = raw_opt_err(&["add", "--help"]);
    goldie::assert!(err.to_string());
    assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
}

#[test]
fn raw_opt_add_no_source() {
    setup();
    assert_eq!(
        raw_opt_err(&["add", "test",]).kind,
        clap::ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn raw_opt_add_git_with_options() {
    setup();
    assert_eq!(
        raw_opt(&[
            "add",
            "test",
            "--git",
            "https://github.com/rossmacarthur/sheldon-test",
            "--rev",
            "ad149784a1538291f2477fb774eeeed4f4d29e45",
            "--dir",
            "missing",
            "--use",
            "{name}.sh",
            "*.zsh",
            "--apply",
            "something",
            "another-thing"
        ])
        .command,
        RawCommand::Add(Box::new(Add {
            name: "test".to_string(),
            git: Some(
                "https://github.com/rossmacarthur/sheldon-test"
                    .parse()
                    .unwrap()
            ),
            gist: None,
            github: None,
            remote: None,
            local: None,
            proto: None,
            branch: None,
            rev: Some("ad149784a1538291f2477fb774eeeed4f4d29e45".into()),
            tag: None,
            dir: Some("missing".into()),
            uses: Some(vec_into!["{name}.sh", "*.zsh"]),
            apply: Some(vec_into!["something", "another-thing"]),
            profiles: None,
        }))
    );
}

#[test]
fn raw_opt_add_gist_options() {
    setup();
    assert_eq!(
        raw_opt(&[
            "add",
            "test",
            "--gist",
            "579d02802b1cc17baed07753d09f5009",
            "--tag",
            "0.1.0",
            "--proto",
            "ssh",
            "--dir",
            "missing",
            "--use",
            "{name}.sh",
            "*.zsh",
            "--apply",
            "something",
            "another-thing"
        ])
        .command,
        RawCommand::Add(Box::new(Add {
            name: "test".to_string(),
            git: None,
            gist: Some("579d02802b1cc17baed07753d09f5009".parse().unwrap()),
            github: None,
            remote: None,
            local: None,
            proto: Some("ssh".parse().unwrap()),
            branch: None,
            rev: None,
            tag: Some("0.1.0".into()),
            dir: Some("missing".into()),
            uses: Some(vec_into!["{name}.sh", "*.zsh"]),
            apply: Some(vec_into!["something", "another-thing"]),
            profiles: None,
        }))
    );
}

#[test]
fn raw_opt_add_github_options() {
    setup();
    assert_eq!(
        raw_opt(&[
            "add",
            "test",
            "--github",
            "rossmacarthur/sheldon-test",
            "--branch",
            "feature",
            "--proto",
            "https",
            "--dir",
            "missing",
            "--use",
            "{name}.sh",
            "*.zsh",
            "--apply",
            "something",
            "another-thing"
        ])
        .command,
        RawCommand::Add(Box::new(Add {
            name: "test".to_string(),
            git: None,
            gist: None,
            github: Some("rossmacarthur/sheldon-test".parse().unwrap()),
            remote: None,
            local: None,
            proto: Some("https".parse().unwrap()),
            branch: Some("feature".into()),
            rev: None,
            tag: None,
            dir: Some("missing".into()),
            uses: Some(vec_into!["{name}.sh", "*.zsh"]),
            apply: Some(vec_into!["something", "another-thing"]),
            profiles: None,
        }))
    );
}

#[test]
fn raw_opt_add_remote_options() {
    setup();
    assert_eq!(
        raw_opt(&[
            "add",
            "test",
            "--remote",
            "https://raw.githubusercontent.com/rossmacarthur/sheldon-test/master/test.plugin.zsh",
            "--use",
            "{name}.sh",
            "*.zsh",
            "--apply",
            "something",
            "another-thing"
        ])
        .command,
        RawCommand::Add(Box::new(Add {
            name: "test".to_string(),
            git: None,
            gist: None,
            github: None,
            remote: Some("https://raw.githubusercontent.com/rossmacarthur/sheldon-test/master/test.plugin.zsh".parse().unwrap()),
            local: None,
            proto: None,
            branch: None,
            rev: None,
            tag: None,
            dir: None,
            uses: Some(vec_into!["{name}.sh", "*.zsh"]),
            apply: Some(vec_into!["something", "another-thing"]),
            profiles: None,
        }))
    );
}

#[test]
fn raw_opt_add_local_options() {
    setup();
    assert_eq!(
        raw_opt(&[
            "add",
            "test",
            "--local",
            "~/.dotfiles/zsh/pure",
            "--use",
            "{name}.sh",
            "*.zsh",
            "--apply",
            "something",
            "another-thing"
        ])
        .command,
        RawCommand::Add(Box::new(Add {
            name: "test".to_string(),
            git: None,
            gist: None,
            github: None,
            remote: None,
            local: Some("~/.dotfiles/zsh/pure".into()),
            proto: None,
            branch: None,
            rev: None,
            tag: None,
            dir: None,
            uses: Some(vec_into!["{name}.sh", "*.zsh"]),
            apply: Some(vec_into!["something", "another-thing"]),
            profiles: None,
        }))
    );
}

#[test]
fn raw_opt_add_remote_with_reference_expect_conflict() {
    setup();
    assert_eq!(
        raw_opt_err(&[
            "add",
            "test",
            "--remote",
            "https://raw.githubusercontent.com/rossmacarthur/sheldon-test/master/test.plugin.zsh",
            "--branch",
            "feature"
        ])
        .kind,
        clap::ErrorKind::ArgumentConflict
    );
}

#[test]
fn raw_opt_add_local_with_reference_expect_conflict() {
    setup();
    assert_eq!(
        raw_opt_err(&[
            "add",
            "test",
            "--local",
            "~/.dotfiles/zsh/pure",
            "--tag",
            "0.1.0"
        ])
        .kind,
        clap::ErrorKind::ArgumentConflict
    );
}

#[test]
fn raw_opt_add_git_with_github_expect_conflict() {
    setup();
    assert_eq!(
        raw_opt_err(&[
            "add",
            "test",
            "--git",
            "https://github.com/rossmacarthur/sheldon-test",
            "--github",
            "rossmacarthur/sheldon-test",
        ])
        .kind,
        clap::ErrorKind::ArgumentConflict
    );
}

#[test]
fn raw_opt_add_git_with_protocol_expect_conflict() {
    setup();
    assert_eq!(
        raw_opt_err(&[
            "add",
            "test",
            "--git",
            "https://github.com/rossmacarthur/sheldon-test",
            "--proto",
            "ssh",
        ])
        .kind,
        clap::ErrorKind::ArgumentConflict
    );
}

#[test]
fn raw_opt_add_remote_with_protocol_expect_conflict() {
    setup();
    assert_eq!(
        raw_opt_err(&[
            "add",
            "test",
            "--remote",
            "https://raw.githubusercontent.com/rossmacarthur/sheldon-test/master/test.plugin.zsh",
            "--proto",
            "ssh",
        ])
        .kind,
        clap::ErrorKind::ArgumentConflict
    );
}

#[test]
fn raw_opt_add_local_with_protocol_expect_conflict() {
    setup();
    assert_eq!(
        raw_opt_err(&[
            "add",
            "test",
            "--local",
            "~/.dotfiles/zsh/pure",
            "--proto",
            "ssh",
        ])
        .kind,
        clap::ErrorKind::ArgumentConflict
    );
}

#[test]
fn raw_opt_lock_help() {
    setup();
    let err = raw_opt_err(&["lock", "--help"]);
    goldie::assert!(err.to_string());
    assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
}

#[test]
fn raw_opt_lock_with_update_and_reinstall_expect_conflict() {
    setup();
    assert_eq!(
        raw_opt_err(&["lock", "--update", "--reinstall"]).kind,
        clap::ErrorKind::ArgumentConflict
    );
}

#[test]
fn raw_opt_source_help() {
    setup();
    let err = raw_opt_err(&["source", "--help"]);
    goldie::assert!(err.to_string());
    assert_eq!(err.kind, clap::ErrorKind::DisplayHelp);
}

#[test]
fn raw_opt_source_with_update_and_reinstall_expect_conflict() {
    setup();
    assert_eq!(
        raw_opt_err(&["source", "--update", "--reinstall"]).kind,
        clap::ErrorKind::ArgumentConflict
    );
}
