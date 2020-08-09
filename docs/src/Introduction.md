<br><br>

Sheldon is a fast, configurable, command-line tool to manage your shell plugins.

## How does it work?

Plugins are specified in a [TOML](https://toml.io) configuration file and
Sheldon renders an install script using user configurable
[handlebars](http://handlebarsjs.com) templates.

A `~/.zshrc` or `~/.bashrc` that uses Sheldon simply contains the following.

```sh
source <(sheldon source)
```

Sheldon can manage GitHub or Git repositories, Gists, arbitrary remote scripts
or binaries, local plugins, and inline plugins. Plugins are installed and
updated in parallel and as a result Sheldon is blazingly fast.

## Source code

Sheldon is open source and you can find the code on
[GitHub](https://github.com/rossmacarthur/sheldon).

## License

Sheldon and its source code is licensed under either of

- [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
- [MIT license](http://opensource.org/licenses/MIT)

at your option.
