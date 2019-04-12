# sheldon

[![Crates.io Version](https://img.shields.io/crates/v/sheldon.svg?style=flat-square)][crates]
[![Docs.rs Latest](https://img.shields.io/badge/docs.rs-latest-brightgreen.svg?style=flat-square&color=blue)][docs]
[![Build Status](https://img.shields.io/travis/rossmacarthur/sheldon/master.svg?style=flat-square)][travis]

A fast, configurable, shell plugin manager.

## Features

- Can manage everything
  - Any Git repository
    - Includes branch/tag/commit support.
    - Includes extra support for GitHub.
    - Includes extra support for Gist (planned).
    - Includes extra support for GitLab (planned).
  - Arbitrary remote files, simply specify the URL.
  - Local plugins, simply specify the file path.
- Configuration file using [TOML] syntax.
- Highly configurable install methods using handlebars templating.
- Super-fast parallel installation.
- Uses a lock file for much faster loading of plugins.

## Getting started

Install it using

```
cargo install sheldon
```

Then create a plugins file at `~/.zsh/plugins.toml`

```toml
[plugins.oh-my-zsh]
source = 'github'
repository = 'robbyrussell/oh-my-zsh'
```

Read up more about configuration [here][configuration].

You can then use the `source` command to generate the init script

```bash
# ~/.zshrc
source <(sheldon source)
```

## License

This project is dual licensed under the Apache 2.0 License and the MIT License.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for more
details.

[crates]: https://crates.io/crates/sheldon
[travis]: https://travis-ci.org/rossmacarthur/sheldon
[docs]: https://docs.rs/sheldon
[configuration]: docs/Configuration.md
[TOML]: https://github.com/toml-lang/toml
