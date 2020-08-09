# Getting started

## Initializing

Sheldon works by specifying plugin information in a [TOML](https://toml.io)
configuration file. You can initialize this file by running `sheldon init`.

```sh
sheldon init --shell zsh
```

or

```sh
sheldon init --shell bash
```

This will create the `~/.sheldon` directory with an empty `plugins.toml` file.
You can either edit this file directly or use the provided command line
interface to add or remove plugins.

## Adding a plugin

To add your first plugin add the following to the Sheldon config file.

```toml
# ~/.sheldon/plugins.toml

[plugins.base16]
github = "chriskempson/base16-shell"
```

Or use the `add` command to automatically add it.

```sh
sheldon add base16 --github chriskempson/base16-shell
```

The first argument given here `base16` is a unique name for the plugin. The
`--github` option specifies that we want Sheldon to manage a clone of the
[https://github.com/chriskempson/base16-shell](https://github.com/chriskempson/base16-shell)
repository.

## Loading plugins

You can then use `sheldon source` to install this plugin, generate a lock file,
and print out the shell script to source. Simply add the following to your
`~/.zshrc` or `~/.bashrc` file.

```sh
# ~/.zshrc or ~/.bashrc

source <(sheldon source)
```
