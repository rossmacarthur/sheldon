<br><br>

**sheldon** is a fast, configurable command line tool to manage your shell
plugins.

## How does it work?

Plugins are specified in a [TOML](https://toml.io) configuration file and
**sheldon** renders an install script using user configurable
[handlebars](http://handlebarsjs.com) templates.

A `~/.zshrc` or `~/.bashrc` that uses **sheldon** simply contains the following.

```sh
source <(sheldon source)
```

**sheldon** can manage Git repositories, arbitrary remote scripts or binaries,
local plugins, and inline plugins. Plugins are installed and updated in parallel
and as a result **sheldon** is blazingly fast.

## Source code

**sheldon** open source and you can find the code on
[GitHub](https://github.com/rossmacarthur/sheldon).

## License

**sheldon** and its source code is licensed under either of

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](http://opensource.org/licenses/MIT)

at your option.
