# 💻 Command line interface

Sheldon has three different types of commands.

- [`init`](#init) initializes a new config file.
- [`lock`](#lock) and [`source`](#source) deal with plugin downloading,
  installation, and generation of shell source code.
- [`add`](#add), [`edit`](#edit), and [`remove`](#remove) automate editing of
  the config file.

## `init`

This command initializes a new config file. If a config file exists then this
command does nothing.

For example

```sh
sheldon init
```

Or you can specify the shell.

```sh
sheldon init --shell bash
```

or

```sh
sheldon init --shell zsh
```

## `lock`

The `lock` command installs the plugins sources and generates the lock file
(`~/.sheldon/plugins.lock`). Rerunning this command without any extra options
will not reinstall plugin sources, just verify that they are correctly
installed. It will always regenerate the lock file.

```sh
sheldon lock
```

To update all plugin sources you can use the `--update` flag.

```sh
sheldon lock --update
```

To force a reinstall of all plugin sources you can use the `--reinstall` flag.

```sh
sheldon lock --reinstall
```

## `source`

This command generates the shell script. This command will first check if there
is an up to date lock file, if not, then it will first do the equivalent of the
lock command above. This command is usually used with the built-in shell `eval`
command.

```sh
eval "$(sheldon source)"
```

But you can also run it directly to inspect the output. The output of this
command is highly configurable. You can define your own custom templates to
apply to your plugins.

## `add`

This command adds a new plugin to the config file. It does nothing else but edit
the config file. In the following command we add a GitHub repository as a
source.

```sh
sheldon add my-repo --git https://github.com/owner/repo.git
```

An example usage of this command for each source type is shown in the
[Configuration](Configuration.md) section.

## `edit`

This command will open the config file in the default editor and only overwrite
the contents if the updated config file is valid. To override the editor that is
used you should set the `EDITOR` environment variable.

For example using `vim`

```sh
EDITOR=vim sheldon edit
```

Or with Visual Studio Code

```sh
EDITOR="code --wait" sheldon edit
```

## `remove`

This command removes a plugin from the config file. It does nothing else but
edit the config file. In the following command we remove the plugin with name
`my-repo`.

```sh
sheldon remove my-repo
```

## Options

Sheldon accepts the following global command line options and environment
variables. You can also view all options by running Sheldon with `-h` or
`--help`. The value that will be used for the option follows the following
priority.

1. Command line option.
2. Environment variable.
3. Default value.

#### `--color <when>`

Set the output coloring.

- `always`: Always use colored output.
- `auto`: Automatically determine whether to use colored output (*default*).
- `never`: Never use colored output.

#### `--home <home>`

*Environment variable:* `HOME`

Set the users home directory. This is usually automatically detected but might
be required if you are using an obscure operating system.

#### `--config-dir <path>`

*Environment variable:* `SHELDON_CONFIG_DIR`

Set the config directory where config will store the configuration file. If
Sheldon detects an XDG directory structure  ([as described
below](#xdg-directory-structure)) then this will default to
`XDG_CONFIG_HOME/sheldon` otherwise it will default to `<home>/.sheldon` where
`<home>` is the users home directory.

#### `--data-dir <path>`

*Environment variable:* `SHELDON_DATA_DIR`

Set the data directory where plugins will be downloaded to. If Sheldon detects
an XDG directory structure ([as described below](#xdg-directory-structure)) then
this will default to `XDG_DATA_HOME/sheldon` otherwise it will default to
`<home>/.sheldon` where `<home>` is the users home directory.

#### `--config-file <path>`

*Environment variable:* `SHELDON_CONFIG_FILE`

Set the path to the config file. This defaults to `<config-dir>/plugins.toml`
where `<config-dir>` is the config directory.

#### `--profile <profile>`

*Environment variable:* `SHELDON_PROFILE`

Specify the profile to match plugins against. Plugins which have
[profiles](Configuration.md#profiles) configured will only get loaded if one of
the given profiles matches the profile.

### XDG directory structure

If any of the following
[XDG](https://wiki.archlinux.org/title/XDG_Base_Directory) environment variables
are set then the default [config](#--config-dir-path) and
[data](#--data-dir-path) directories will change as specified above.

- `XDG_CONFIG_HOME`, defaults to `<home>/.config` where `<home>` is the users
  home directory.
- `XDG_CACHE_HOME`
- `XDG_DATA_HOME`, defaults to `<home>/.local/share` where `<home>` is the users
  home directory.
- `XDG_DATA_DIRS`
- `XDG_CONFIG_DIRS`

## Completions

Shell completion scripts for Bash and Zsh are available. If Sheldon was
installed via Homebrew then the completions should have been installed
automatically.

They can also be generated by Sheldon using the `completions` subcommand which
will output the completions to stdout. Refer to your specific shell
documentation for more details on how to install these.

```
sheldon completions --shell bash > /path/to/completions/sheldon.bash
```

or

```
sheldon completions --shell zsh > /path/to/completions/_sheldon
```
