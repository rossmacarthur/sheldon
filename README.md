# sheldon

[![Crates.io Version](https://img.shields.io/crates/v/sheldon.svg?style=flat-square)][crates]
[![Docs.rs Latest](https://img.shields.io/badge/docs.rs-latest-brightgreen.svg?style=flat-square&color=blue)][docs]
[![Build Status](https://img.shields.io/travis/rossmacarthur/sheldon/master.svg?style=flat-square)][travis]

A fast, configurable, shell plugin manager.

## Features

- Can manage virtually anything.
  - Any public Git repository.
    - Branch / tag / commit support.
    - First class support for GitHub repositories.
    - First class support for Gists.
  - Arbitrary remote files.
  - Local plugins.
  - Inline plugins.
- Highly configurable install methods using [handlebars] templating.
- Shell agnostic, with sensible defaults for [Zsh].
- Super-fast parallel installation.
- Configuration file using [TOML] syntax.
- Uses a lock file for much faster loading of plugins.

## Table of Contents

- [Features](#features)
- [Table of Contents](#table-of-contents)
- [Installation](#installation)
  - [Pre-built binaries](#pre-built-binaries)
  - [Cargo](#cargo)
- [Getting started](#getting-started)
- [Command line interface](#command-line-interface)
  - [`lock` command](#lock-command)
  - [`source` command](#source-command)
  - [Flags](#flags)
  - [Options](#options)
- [Configuration: plugin sources](#configuration-plugin-sources)
  - [Git](#git)
    - [`github`](#github)
    - [`gist`](#gist)
    - [`git`](#git)
    - [Specifying a branch, tag, or commit](#specifying-a-branch-tag-or-commit)
    - [Cloning with Git or SSH protocols](#cloning-with-git-or-ssh-protocols)
  - [Remote](#remote)
  - [Local](#local)
- [Configuration: plugin options](#configuration-plugin-options)
  - [`use`](#use)
  - [`apply`](#apply)
- [Configuration: inline plugins](#configuration-inline-plugins)
- [Configuration: templates](#configuration-templates)
  - [Custom templates](#custom-templates)
  - [Example: symlinking files](#example-symlinking-files)
  - [Example: overriding the PATH template](#example-overriding-the-path-template)
- [Configuration: global options](#configuration-global-options)
  - [`match`](#match)
  - [`apply`](#apply-1)
- [License](#license)

## Installation

### Pre-built binaries

Pre-built binaries for Linux (x86-64, armv7) and macOS (x86-64) can be found on
[the releases page][releases].

Alternatively, the following script can be used to automatically detect your
host system, download the required artefact, and extract the **sheldon** binary.

```sh
curl --proto '=https' -fLsS https://rossmacarthur.github.io/install/crate.sh \
    | sh -s -- --repo "rossmacarthur/sheldon" --to /usr/local/bin
```

### Cargo

**sheldon** can be installed using [cargo], the Rust language package manager.
Install [cargo] using [rustup] then run

```sh
cargo install sheldon
```

Updating can be done using

```sh
cargo install sheldon --force
```

## Getting started

The config file uses the [TOML] file format. Create a configuration file at
`~/.zsh/plugins.toml` and add details for your first plugin by adding a unique
key to the `plugins` table. In the example configuration file below we add a new
Github type plugin with a unique name `oh-my-zsh`.

```toml
# ~/.zsh/plugins.toml

#            ┌─ Unique name for the plugin
#        ┌───┴───┐
[plugins.oh-my-zsh]
github = "robbyrussell/oh-my-zsh"
#         └─────┬────┘ └───┬───┘
#               │          └─ GitHub repository name
#               └─ GitHub user or organization
```

You can then use `sheldon source` to install the configured plugins, generate
the lock file, and print out the script to source. Simply add the following to
your `~/.zshrc` file

```sh
# ~/.zshrc

source <(sheldon source)
```

For a more fleshed out example configuration file see
[here](docs/plugins.example.toml).

## Command line interface

### `lock` command

This command installs the plugins sources and generates the lock file. If we ran
this on the example configuration file above, then the following output would be
produced.

<img width="437" alt="sheldon lock output" src="https://user-images.githubusercontent.com/17109887/60550355-059def80-9d28-11e9-8b1e-67b5fb10e74d.png">

Running it again would not redownload the plugin

<img width="437" alt="image" src="https://user-images.githubusercontent.com/17109887/60550441-4433aa00-9d28-11e9-8429-e6380889e348.png">

### `source` command

This command generates the shell script to be sourced. This command will first
check if there is an up to date lock file otherwise it will relock the
configuration file.

<img width="688" alt="image" src="https://user-images.githubusercontent.com/17109887/60550596-cae88700-9d28-11e9-906b-74f6f5d80149.png">

If we now modify our configuration file and run this command again it will
relock the configuration prior to generating the script.

<img width="691" alt="image" src="https://user-images.githubusercontent.com/17109887/60550665-02573380-9d29-11e9-84e9-5dfa89b11895.png">

The output of this command is highly configurable. You can define your own
[custom templates](#configuration-templates) to apply to your plugins.

### Flags

**sheldon** accepts the following global command line flags.

| Flag              | Description                        |
| ----------------- | ---------------------------------- |
| `-q`, `--quiet`   | Suppress any informational output. |
| `-v`, `--verbose` | Use verbose output.                |
| `--no-color`      | Do not use ANSI colored output.    |
| `-h`, `--help`    | Show the help message and exit.    |
| `-V`, `--version` | Show the version and exit.         |

### Options

**sheldon** accepts the following global command line options.

| Option                  | Environment variable   | Description                                                 |
| ------------------------| ---------------------- | ----------------------------------------------------------- |
| `--home <path>`         | `HOME`                 | Set the home directory.                                     |
| `--root <path>`         | `SHELDON_ROOT`         | Set the root directory. (*default:* `<home>/.zsh`)          |
| `--config-file <path>`  | `SHELDON_CONFIG_FILE`  | Set the config file. (*default:*  `<root>/plugins.toml`)    |
| `--lock-file <path>`    | `SHELDON_LOCK_FILE`    | Set the lock file. (*default:* `<config-file>.lock`)        |
| `--clone-dir <path>`    | `SHELDON_CLONE_DIR`    | Set the clone directory. (*default:* `<root>/repositories`) |
| `--download-dir <path>` | `SHELDON_DOWNLOAD_DIR` | Set the download directory. (*default:* `<root>/downloads`) |

**Note:** in rare circumstances **sheldon** will not be able to automatically
detect the user's home directory. You should only have to set the `--home`
option in these cases.

The priority order for setting these values is the following

1. Command line option.
2. Environment variable.
3. Default value.

## Configuration: plugin sources

A plugin is defined by adding a new unique name to the `plugins` table in the
[TOML] configuration file. A plugin must define the location of the source.
There are three types of sources, each kind is described below. A plugin may
only specify _one_ source type.

### Git

Git sources specify a remote Git repository that will be cloned to the
**sheldon** root directory. There are three flavors of Git sources.

#### `github`

A GitHub source must set the `github` field and specify the repository. This
should be the username or organization and the repository name separated by a
forward slash.

```toml
[plugins.pure]
github = "sindresorhus/pure"
```

#### `gist`

A Gist source must set the `gist` field and specify the repository. This should
be the hash or username and hash of the Gist.

```toml
[plugins.pure]
gist = "579d02802b1cc17baed07753d09f5009"
```

#### `git`

A Git source must set the `git` field and specify the URL.

```toml
[plugins.pure]
git = "https://github.com/sindresorhus/pure"
```

#### Specifying a branch, tag, or commit

All Git sources also allow setting of one of the `branch`, `tag` or `revision`
fields. **sheldon** will then checkout the repository at this reference.

```toml
[plugins.pure]
github = "sindresorhus/pure"
tag = "1.9.0"
```

#### Cloning with Git or SSH protocols

GitHub and Gist sources are cloned using HTTPS by default. You can specify that
Git or SSH should be used by setting the `protocol` field to the protocol type.
This must be one of `git`, `https`, or `ssh`.

```toml
[plugins.pure]
github = "sindresorhus/pure"
protocol = "ssh"
```

For a plain Git source you should specify the URL with a `git://` or `ssh://`
protocol. For SSH you will need to specify the username as well (it is `git` for
GitHub).

```toml
[plugins.pure]
git = "ssh://git@github.com/sindresorhus/pure"
```

**Note:** Currently **sheldon** only supports authentication when cloning using
SSH and only via the SSH agent.

### Remote

Remote sources specify a remote file that will be downloaded to the **sheldon**
root directory. A Remote source must set the  `remote` field and specify the
URL.

```toml
[plugins.pure]
remote = "https://github.com/rossmacarthur/pure/raw/master/pure.zsh"
```

### Local

Local sources reference local directories. A Local source must set the `local`
field and specify a `directory`. Tildes may be used and will be expanded to the
current user's home directory.

```toml
[plugins.pure]
local = "~/Downloads/repositories/pure"
```

## Configuration: plugin options

These are options that are common to all the above plugins.

### `use`

A list of files / globs to use in the plugin's source directory.

```toml
[plugins.pure]
github = "sindresorhus/pure"
use = ["*.zsh"]
```

If this field is not given then the first pattern in the global `match` field
that matches any files will be used.

### `apply`

A list of template names to apply to this plugin. This defaults to the global
[`apply`](#apply-1).

```toml
[plugins.pure]
github = "sindresorhus/pure"
apply = ["source", "PATH"]
```

You can define your own [custom templates](#custom-templates) to apply to your
plugins.

## Configuration: inline plugins

For convenience it also possible to define Inline plugins. An Inline plugin must
set the `inline` field and specify the raw source.

```toml
[plugins.pure]
inline = """
echo 'not really `pure`'
"""
```

## Configuration: templates

A template is a string that represents a generic action to take on a plugin. For
example the **PATH** template adds the plugin directory to the shell `PATH`
variable. A plugin will apply a template if you add the template name to the
[`apply`](#apply) field on a plugin.

Available built in templates are

- **source**: source each filename in a plugin.
- **PATH**: add the plugin directory to the `PATH` variable.
- **FPATH**: add the plugin directory to the `FPATH` variable.
- **path**: add the plugin directory to the `path` variable.
- **fpath**: add the plugin directory to the `fpath` variable.

As template strings they could be represented like this

```toml
[templates]
source = { value = 'source "{{ filename }}"', each = true }
PATH = 'export PATH="{{ directory }}:$PATH"'
FPATH = 'export FPATH="{{ directory }}:$FPATH"'
path = 'path=( "{{ directory }}" $path )'
fpath = 'fpath=( "{{ directory }}" $fpath )'
```

For example if we change the `apply` field for the below plugin, it will only
add the plugin directory to the `PATH` and append it to the `fpath`. The plugin
will not be sourced.

```toml
[plugins.pure]
source = "github"
repository = "sindresorhus/pure"
apply = ["PATH", "fpath"]
```

The `each` value, as used in the `source` template above, specifies that the
template should be applied to each matched filename for the plugin. This
defaults to `false`.

### Custom templates

It is possible to create your own custom templates, and you can even override
the built in ones.

Plugins all have the following information that can be used in templates

- **A unique name.** This is completely arbitrary, and it is the value specified
  for the plugin in the plugins table. However, it is often the name of the
  plugin, so it can be useful to use this name in templates with `{{ name }}`.
- **A directory.** In git sources this is the location of the cloned repository,
  for local sources, it is the `directory` specified. This `directory` can be
  used in templates with `{{ directory }}`.
- **Zero or more filenames.** These are the matched files in the plugin
  directory either discovered using the the global `match` field or specified as
  a plugin option with `use`. These can be used in templates using `{{ filename
  }}`.

You can use the following global information in templates

- **The sheldon root.** This folder can be used as `{{ root }}`.

### Example: symlinking files

Lets say we would like a template to symlink files into the `~/.zsh/functions`
directory. We could create a new template with name **function**, like this

```toml
[templates]
function = { value = 'ln -sf "{{ filename }}" "~/.zsh/functions/{{ name }}"', each = true }
```

It can then be applied to the plugin like this

```toml
[plugins.pure]
github = "sindresorhus/pure"
apply = ["function"]
```

### Example: overriding the PATH template

The built in **PATH** template adds the directory path to the beginning of the
`PATH` variable, we might want to change it to the be added at the end. We could
do this like this

```toml
[templates]
PATH = 'export PATH="$PATH:{{ directory }}"'
```

You can then apply it to the plugin like this

```toml
[plugins.pure]
github = "sindresorhus/pure"
apply = ["source", "PATH"]
```

**Note:** this would change the behavior of **PATH** for *all* plugins using it.

## Configuration: global options

### `match`

A list of glob patterns to match against a plugin's contents. The first pattern
that matches any files will be used by default as a plugin's `use` field. This
defaults to

```toml
match = [
  "{{ name }}.plugin.zsh",
  "{{ name }}.zsh",
  "{{ name }}.sh",
  "{{ name }}.zsh-theme",
  "*.plugin.zsh",
  "*.zsh",
  "*.sh",
  "*.zsh-theme",
]
```

**Note:** if you are not using [Zsh] then you should probably change this
setting.

### `apply`

A list of template names to apply to all plugins by default (see
[`apply`](#apply)). This defaults to

```toml
apply = ["source"]
```

## License

This project is dual licensed under the Apache 2.0 License and the MIT License.

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for more
details.

[cargo]: https://doc.rust-lang.org/cargo/
[configuration]: docs/Configuration.md
[crates]: https://crates.io/crates/sheldon
[docs]: https://docs.rs/sheldon
[handlebars]: http://handlebarsjs.com
[releases]: https://github.com/rossmacarthur/sheldon/releases
[rust-lang]: https://www.rust-lang.org/
[rustup]: https://rustup.rs/
[travis]: https://travis-ci.org/rossmacarthur/sheldon
[TOML]: https://github.com/toml-lang/toml
[Zsh]: http://www.zsh.org/
