<!-- Generated by cargo-onedoc. DO NOT EDIT. -->

# sheldon

*Fast, configurable, shell plugin manager*

[![Crates.io version](https://img.shields.io/crates/v/sheldon.svg)](https://crates.io/crates/sheldon)
[![Download](https://img.shields.io/github/v/release/rossmacarthur/sheldon)](https://github.com/rossmacarthur/sheldon/releases/latest)
[![Build Status](https://img.shields.io/github/actions/workflow/status/rossmacarthur/sheldon/build.yaml?branch=trunk)](https://github.com/rossmacarthur/sheldon/actions/workflows/build.yaml)

## Features

- Plugins from Git repositories.
  - Branch / tag / commit support.
  - Submodule support.
  - First class support for GitHub repositories.
  - First class support for Gists.
- Arbitrary remote scripts or binary plugins.
- Local plugins.
- Inline plugins.
- Highly configurable install methods using templates.
- Shell agnostic, with sensible defaults for Zsh.
- Super-fast plugin loading and parallel installation. See [benchmarks].
- Config file using [TOML](https://toml.io) syntax.
- Clean `~/.zshrc` or `~/.bashrc` (just add 1 line).

[benchmarks]: https://github.com/rossmacarthur/zsh-plugin-manager-benchmark

## Table of Contents

- [📦 Installation](#-installation)
  - [Homebrew](#homebrew)
  - [Cargo](#cargo)
  - [Cargo BInstall](#cargo-binstall)
  - [Pre-built binaries](#pre-built-binaries)
  - [Building from source](#building-from-source)
- [🚀 Getting started](#-getting-started)
  - [Initializing](#initializing)
  - [Adding a plugin](#adding-a-plugin)
  - [Loading plugins](#loading-plugins)
- [💻 Command line interface](#-command-line-interface)
  - [`init`](#init)
  - [`lock`](#lock)
  - [`source`](#source)
  - [`add`](#add)
  - [`edit`](#edit)
  - [`remove`](#remove)
  - [Options](#options)
      - [`--color <when>`](#--color-when)
      - [`--config-dir <path>`](#--config-dir-path)
      - [`--data-dir <path>`](#--data-dir-path)
      - [`--config-file <path>`](#--config-file-path)
      - [`--profile <profile>`](#--profile-profile)
  - [Completions](#completions)
- [⚙️ Configuration](#️-configuration)
  - [Plugin sources](#plugin-sources)
    - [Git](#git)
      - [`github`](#github)
      - [`gist`](#gist)
      - [`git`](#git-1)
      - [Specifying a branch, tag, or commit](#specifying-a-branch-tag-or-commit)
      - [Cloning with Git or SSH protocols](#cloning-with-git-or-ssh-protocols)
      - [Private Git repositories](#private-git-repositories)
    - [Remote](#remote)
    - [Local](#local)
  - [Plugin options](#plugin-options)
    - [`use`](#use)
    - [`apply`](#apply)
    - [`profiles`](#profiles)
  - [Inline plugins](#inline-plugins)
  - [Templates](#templates)
    - [Custom templates](#custom-templates)
  - [Global options](#global-options)
    - [`shell`](#shell)
    - [`match`](#match)
    - [`apply`](#apply-1)
- [💡 Examples](#-examples)
- [License](#license)

## 📦 Installation

### Homebrew

Sheldon can be installed using Homebrew.

```sh
brew install sheldon
```

### Cargo

Sheldon can be installed from [Crates.io](https://crates.io/crates/sheldon)
using [Cargo](https://doc.rust-lang.org/cargo/), the Rust package manager.

```sh
cargo install sheldon
```

In some circumstances this can fail due to the fact that Cargo does not use
`Cargo.lock` file by default. You can force Cargo to use it using the `--locked`
option.

```sh
cargo install sheldon --locked
```

### Cargo BInstall

Sheldon can be installed using
[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall), which will
download the release artifacts directly from the GitHub release.

```sh
cargo binstall sheldon
```

### Pre-built binaries

Pre-built binaries for Linux (x86-64, aarch64, armv7) and macOS (x86-64) are
provided. These can be downloaded directly from the [the releases
page](https://github.com/rossmacarthur/sheldon/releases).

Alternatively, the following script can be used to automatically detect your host
system, download the required artifact, and extract the `sheldon` binary to the
given directory.

```sh
curl --proto '=https' -fLsS https://rossmacarthur.github.io/install/crate.sh \
    | bash -s -- --repo rossmacarthur/sheldon --to ~/.local/bin
```

### Building from source

Sheldon is written in Rust, so to install it from source you will first need to
install Rust and Cargo using [rustup](https://rustup.rs/). Then you can run the
following to build Sheldon.

```sh
git clone https://github.com/rossmacarthur/sheldon.git
cd sheldon
cargo build --release
```

The binary will be found at `target/release/sheldon`.

## 🚀 Getting started

### Initializing

Sheldon works by specifying plugin information in a [TOML](https://toml.io)
configuration file, `plugins.toml`. You can initialize this file by running
`sheldon init`.

```sh
sheldon init --shell bash
```

or

```sh
sheldon init --shell zsh
```

This will create `plugins.toml` under `$XDG_CONFIG_HOME/sheldon`, on most
systems this will be `~/.config/sheldon/plugins.toml`. You can either edit this
file directly or use the provided command line interface to add or remove
plugins.

### Adding a plugin

To add your first plugin append the following to the Sheldon config file.

```toml
# ~/.config/sheldon/plugins.toml

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

### Loading plugins

You can then use `sheldon source` to install this plugin, generate a lock file,
and print out the shell script to source. Simply add the following to your
`~/.zshrc` or `~/.bashrc` file.

```sh
# ~/.zshrc or ~/.bashrc

eval "$(sheldon source)"
```

## 💻 Command line interface

Sheldon has three different types of commands.

- [`init`](#init) initializes a new config file.
- [`lock`](#lock) and [`source`](#source) deal with plugin downloading,
  installation, and generation of shell source code.
- [`add`](#add), [`edit`](#edit), and [`remove`](#remove) automate editing of
  the config file.

### `init`

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

### `lock`

The `lock` command installs the plugins sources and generates the lock file.
Rerunning this command without any extra options will not reinstall plugin
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

### `source`

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

### `add`

This command adds a new plugin to the config file. It does nothing else but edit
the config file. In the following command we add a GitHub repository as a
source.

```sh
sheldon add my-repo --git https://github.com/owner/repo.git
```

An example usage of this command for each source type is shown in the
[Configuration](https://sheldon.cli.rs/Configuration.html) section.

### `edit`

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

### `remove`

This command removes a plugin from the config file. It does nothing else but
edit the config file. In the following command we remove the plugin with name
`my-repo`.

```sh
sheldon remove my-repo
```

### Options

Sheldon accepts the following global command line options and environment
variables. You can also view all options by running Sheldon with `-h` or
`--help`. The value that will be used for the option follows the following
priority.

1. Command line option.
1. Environment variable.
1. Default value.

##### `--color <when>`

Set the output coloring.

- `always`: Always use colored output.
- `auto`: Automatically determine whether to use colored output (*default*).
- `never`: Never use colored output.

##### `--config-dir <path>`

*Environment variable:* `SHELDON_CONFIG_DIR`

Set the config directory where the configuration file will be stored. This
defaults to `$XDG_CONFIG_HOME/sheldon` or `~/.config/sheldon`.

##### `--data-dir <path>`

*Environment variable:* `SHELDON_DATA_DIR`

Set the data directory where plugins will be downloaded to. This defaults to
`$XDG_DATA_HOME/sheldon` or `~/.local/share/sheldon`.

##### `--config-file <path>`

*Environment variable:* `SHELDON_CONFIG_FILE`

Set the path to the config file. This defaults to `<config-dir>/plugins.toml`
where `<config-dir>` is the config directory.

##### `--profile <profile>`

*Environment variable:* `SHELDON_PROFILE`

Specify the profile to match plugins against. Plugins which have
[profiles](https://sheldon.cli.rs/Configuration.html#profiles) configured will only get loaded if one of
the given profiles matches the profile.

### Completions

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

## ⚙️ Configuration

### Plugin sources

A plugin is defined by adding a new unique name to the `plugins` table in the
[TOML](https://toml.io) config file. This can be done by either editing the file
directly or using the provided Sheldon commands. A plugin must provide the
location of the source. There are three types of sources, each kind is described
in this section. A plugin may only specify *one* source type.

```toml
# ~/.config/sheldon/plugins.toml

#           ┌─ Unique name for the plugin
#        ┌──┴─┐
[plugins.base16]
github = "chriskempson/base16-shell"
#         └─────┬────┘ └─────┬────┘
#               │            └─ GitHub repository name
#               └─ GitHub user or organization
```

#### Git

Git sources specify a remote Git repository that will be cloned to the Sheldon
data directory. There are three flavors of Git sources.

##### `github`

A GitHub source must set the `github` field and specify the repository. This
should be the username or organization and the repository name separated by a
forward slash. Add the following to the Sheldon config file.

```toml
[plugins.example]
github = "owner/repo"
```

Or run `add` with the `--github` option.

```sh
sheldon add example --github owner/repo
```

##### `gist`

A Gist source must set the `gist` field and specify the repository. This should
be the hash or username and hash of the Gist. Add the following to the Sheldon
config file.

```toml
[plugins.example]
gist = "579d02802b1cc17baed07753d09f5009"
```

Or run `add` with the `--gist` option.

```sh
sheldon add example --gist 579d02802b1cc17baed07753d09f5009
```

##### `git`

A Git source must set the `git` field and specify the URL to clone. Add the
following to the Sheldon config file.

```toml
[plugins.example]
git = "https://github.com/owner/repo"
```

Or run `add` with the `--git` option.

```sh
sheldon add example --git https://github.com/owner/repo
```

##### Specifying a branch, tag, or commit

All Git sources also allow setting of one of the `branch`, `tag` or `rev`
fields. Sheldon will then checkout the repository at this reference.

```toml
[plugins.example]
github = "owner/repo"
tag = "v0.1.0"
```

Or run `add` with the `--tag`, `--branch`, or `--rev` option.

```sh
sheldon add example --github owner/repo --tag v0.1.0
```

##### Cloning with Git or SSH protocols

GitHub and Gist sources are cloned using HTTPS by default. You can specify that
Git or SSH should be used by setting the `proto` field to the protocol type.
This must be one of `git`, `https`, or `ssh`.

```toml
[plugins.example]
github = "owner/repo"
proto = "ssh"
```

For a plain Git source you should specify the URL with a `git://` or `ssh://`.
For SSH you will need to specify the username as well (it is `git` for GitHub).

```toml
[plugins.example]
git = "ssh://git@github.com/owner/repo"
```

##### Private Git repositories

Currently Sheldon only supports authentication when cloning using SSH and
requires an SSH agent to provide credentials. This means if you have a plugin
source that is a private repository you will have to use the SSH protocol for
cloning.

#### Remote

Remote sources specify a remote file that will be downloaded by Sheldon. A
remote source must set the `remote` field and specify the URL. Add the following
to the Sheldon config file.

```toml
[plugins.example]
remote = "https://github.com/owner/repo/raw/branch/plugin.zsh"
```

Or run `add` with the `--remote` option.

```sh
sheldon add example --remote https://github.com/owner/repo/raw/branch/plugin.zsh
```

#### Local

Local sources reference local directories. A local source must set the `local`
field and specify a directory. Tildes may be used and will be expanded to the
current user’s home directory. Add the following to the Sheldon config file.

```toml
[plugins.example]
local = "~/Downloads/plugin"
```

Or run `add` with the `--local` option.

```sh
sheldon add example --local '~/Downloads/plugin'
```

### Plugin options

These are options that are common to all the above plugins.

#### `use`

A list of files / globs to use in the plugin’s source directory. If this field
is not given then the first pattern in the global [`match`](#match) field that
matches any files will be used. Add the following to the Sheldon config file.

```toml
[plugins.example]
github = "owner/repo"
use = ["*.zsh"]
```

Or run `add` with the `--use` option when adding the plugin.

```sh
sheldon add example --github owner/repo --use '*.zsh'
```

#### `apply`

A list of template names to apply to this plugin. This defaults to the global
[`apply`](#apply-1).

```toml
[plugins.example]
github = "owner/repo"
apply = ["source", "PATH"]
```

Or run `add` with the `--apply` option when adding the plugin.

```sh
sheldon add example --github owner/repo --apply source PATH
```

You can define your own [custom templates](#custom-templates) to apply to your
plugins.

#### `profiles`

A list of profiles this plugin should be used in. If this field is not given the
plugin will be used regardless of the profile. Otherwise, the plugin is only
used if the specified [profile](https://sheldon.cli.rs/Command-line-interface.html#--profile-profile) is
included in the configured list of profiles.

### Inline plugins

For convenience it also possible to define Inline plugins. An Inline plugin must
set the `inline` field and specify the raw source.

```toml
[plugins.example]
inline = 'example() { echo "Just an example of inline shell code" }'
```

### Templates

A template defines how the shell source for a particular plugin is generated.
For example the **PATH** template adds the plugin directory to the shell `PATH`
variable. A template will be applied to a plugin if you add the template name to
the [`apply`](#apply) field on a plugin.

Available built-in templates are different depending on what shell you are
using. The following are available for both Bash and Zsh.

- **source**: source each file in a plugin.
- **PATH**: add the plugin directory to the `PATH` variable.

If you are using Zsh then the following are also available.

- **path**: add the plugin directory to the `path` variable.
- **fpath**: add the plugin directory to the `fpath` variable.

As template strings in the config file they could be represented like the
following.

```toml
[templates]
source = "{% for file in files %}source \"{{ file }}\"\n{% endfor %}"
PATH = 'export PATH="{{ dir }}:$PATH"'
path = 'path=( "{{ dir }}" $path )'
fpath = 'fpath=( "{{ dir }}" $fpath )'
```

For example if we change the `apply` field for the below plugin, it will only
add the plugin directory to the `PATH` and append it to the `fpath`. The plugin
will not be sourced.

```toml
[plugins.example]
github = "owner/repo"
apply = ["PATH", "fpath"]
```

#### Custom templates

It is possible to create your own custom templates, and you can even override
the built-in ones.

Plugins all have the following information that can be used in templates.

- **A unique name.** This is completely arbitrary, and it is the value specified
  for the plugin in the plugins table. However, it is often the name of the
  plugin, so it can be useful to use this name in templates with `{{ name }}`.

- **A directory.** For Git sources this is the location of the cloned
  repository, for local sources, it is the directory specified. This directory
  can be used in templates with `{{ dir }}`.

- **One or more files.** These are the matched files in the plugin directory
  either discovered using the the global `match` field or specified as a plugin
  option with `use`. These can be used in templates by iterating over the files.
  For example: `{% for file in  files %} ... {{ file }} ... {% endfor %}`.

To add or update a template add a new key to the `[templates]` table in the
config file. Take a look at the [examples](https://sheldon.cli.rs/Examples.html) for some interesting
applications of this.

### Global options

#### `shell`

Indicates the shell that you are using. This setting will affect the default
values for several global config settings. This includes the global
[`match`](#match) setting and the available templates. This defaults to `zsh`.

```toml
shell = "bash"
```

or

```toml
shell = "zsh"
```

#### `match`

A list of glob patterns to match against a plugin’s contents. The first pattern
that matches any files will be used by default as a plugin’s `use` field. This
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
    "*.zsh-theme"
]
```

If the shell is Bash then this defaults to

```toml
match = [
    "{{ name }}.plugin.bash",
    "{{ name }}.plugin.sh",
    "{{ name }}.bash",
    "{{ name }}.sh",
    "*.plugin.bash",
    "*.plugin.sh",
    "*.bash",
    "*.sh"
]
```

#### `apply`

A list of template names to apply to all plugins by default (see
[`apply`](#apply)). This defaults to

```toml
apply = ["source"]
```

## 💡 Examples

You can find many examples including deferred loading of plugins in the
[documentation](https://sheldon.cli.rs/Examples.html).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
