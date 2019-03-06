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

[configuration]: docs/Configuration.md
[TOML]: https://github.com/toml-lang/toml
