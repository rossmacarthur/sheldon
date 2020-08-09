# Command line interface

Sheldon has three different types of commands.

- [`init`](#init) initializes a new config file.
- [`lock`](#lock) and [`source`](#source) deal with plugin downloading,
  installation, and generation of shell source code.
- [`add`](#add), [`edit`](#edit), and [`remove`](#remove) automate editing of
  the config file.

## `lock`

The `lock` command installs the plugins sources and generates the lock file
(`~/.sheldon/plugins.lock`). Rerunning this command will not reinstall plugin
sources, just verify that they are correctly installed. It will always
regenerate the lock file.

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
lock command above. This command is usually used with the built-in shell
`source` command.

```sh
source <(sheldon source)
```

But you can also run it directly to inspect the output. The output of this
command is highly configurable. You can define your own custom templates to
apply to your plugins.

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

## `add`

This command adds a new plugin to the config file. It does nothing else but edit
the config file. In the following command we add a GitHub repository as a
source.

```sh
sheldon add my-repo --git https://github.com/owner/repo.git
```

An example usage of this command for each source type is shown in the
[Configuration: plugin sources](#configuration-plugin-sources) section.

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

## Flags

Sheldon accepts the following global command line flags.

| Flag              | Description                       |
| ----------------- | --------------------------------- |
| `-q`, `--quiet`   | Suppress any informational output |
| `-v`, `--verbose` | Use verbose output                |
| `--no-color`      | Do not use ANSI colored output    |
| `-h`, `--help`    | Show the help message and exit    |
| `-V`, `--version` | Show the version and exit         |

## Options

Sheldon accepts the following global command line options.

| Option                  | Environment variable   | Description                                                 |
| ----------------------- | ---------------------- | ----------------------------------------------------------- |
| `--home <path>`         | `HOME`                 | Set the home directory. (*default:* auto)                   |
| `--root <path>`         | `SHELDON_ROOT`         | Set the root directory. (*default:* `<home>/.sheldon`)      |
| `--config-file <path>`  | `SHELDON_CONFIG_FILE`  | Set the config file. (*default:*  `<root>/plugins.toml`)    |
| `--lock-file <path>`    | `SHELDON_LOCK_FILE`    | Set the lock file. (*default:* `<config-file>.lock`)        |
| `--clone-dir <path>`    | `SHELDON_CLONE_DIR`    | Set the clone directory. (*default:* `<root>/repos`)        |
| `--download-dir <path>` | `SHELDON_DOWNLOAD_DIR` | Set the download directory. (*default:* `<root>/downloads`) |

The priority order for setting these values is the following

1. Command line option.
2. Environment variable.
3. Default value.
