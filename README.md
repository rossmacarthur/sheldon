# sheldon

*WIP*

A fast, configurable, shell plugin manager.

## Features

- Can manage
  - Remote Git repositories.
  - GitHub repositories.
  - Local plugins.
- Configuration file using [TOML] syntax. See [here][configuration].
- Uses a lock file for much faster loading of plugins.
- Downloads plugins in parallel (planned).

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

```
# ~/.zshrc
source <(sheldon source)
```

## License

This project is dual licensed under the Apache 2.0 License and the MIT License.

See the [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) files.

[configuration]: docs/Configuration.md
[TOML]: https://github.com/toml-lang/toml
