# sheldon

[![Crates.io Version](https://img.shields.io/crates/v/sheldon.svg?style=flat-square)][crates]
[![Build Status](https://img.shields.io/travis/rossmacarthur/sheldon/master.svg?style=flat-square)][travis]
[![Code Coverage]( https://img.shields.io/codecov/c/github/rossmacarthur/sheldon.svg?style=flat-square)][codecov]
[![Docs.rs Status](https://img.shields.io/badge/docs-passing-brightgreen.svg?style=flat-square&colorB=4c1)][docs]

A fast, configurable, shell plugin manager.

## Features

- Can manage
  - [x] Remote Git repositories.
  - [x] GitHub repositories.
  - [x] Local plugins.
  - [ ] Gist files
  - [ ] Arbitrary binary downloads
- [x] Configuration file using [TOML] syntax.
- [x] Uses a lock file for much faster loading of plugins.
- [x] Highly configurable install methods using handlebars templating.
- [ ] Branch/tag/commit support.
- [ ] Downloads plugins in parallel.

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
[codecov]: https://codecov.io/gh/rossmacarthur/sheldon
[docs]: https://docs.rs/sheldon
[configuration]: docs/Configuration.md
[TOML]: https://github.com/toml-lang/toml
