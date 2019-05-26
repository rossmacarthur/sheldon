# sheldon

[![Crates.io Version](https://img.shields.io/crates/v/sheldon.svg?style=flat-square)][crates]
[![Docs.rs Latest](https://img.shields.io/badge/docs.rs-latest-brightgreen.svg?style=flat-square&color=blue)][docs]
[![Build Status](https://img.shields.io/travis/rossmacarthur/sheldon/master.svg?style=flat-square)][travis]

A fast, configurable, shell plugin manager.

## Features

- Can manage virtually anything.
  - Any public Git repository.
    - Branch/tag/commit support.
    - Extra support for GitHub repositories.
    - Extra support for Gists.
  - Arbitrary remote files, simply specify the URL.
  - Local plugins, simply specify the directory path.
- Highly configurable install methods using [handlebars] templating.
- Super-fast parallel installation.
- Configuration file using [TOML] syntax.
- Uses a lock file for much faster loading of plugins.

## Getting started

You can install the `sheldon` command line tool using

```sh
cargo install sheldon
```

Create a configuration file at `~/.zsh/plugins.toml`.

```toml
[plugins.oh-my-zsh]
github = 'robbyrussell/oh-my-zsh'
```

Read up more about configuration [here][configuration].

You can then use the source command to generate the script

```sh
# ~/.zshrc
source <(sheldon source)
```

## License

This project is dual licensed under the Apache 2.0 License and the MIT License.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for more
details.

[configuration]: docs/Configuration.md
[crates]: https://crates.io/crates/sheldon
[docs]: https://docs.rs/sheldon
[handlebars]: http://handlebarsjs.com
[travis]: https://travis-ci.org/rossmacarthur/sheldon
[TOML]: https://github.com/toml-lang/toml
