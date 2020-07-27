<h1 align="center">sheldon</h1>
<div align="center">
  <strong>
    <img class="emoji" title=":bowtie:" alt=":bowtie:" src="https://github.githubassets.com/images/icons/emoji/bowtie.png" width="20" height="20" align="absmiddle">
      A fast, configurable, shell plugin manager
    </strong>
</div>
<br />
<div align="center">
  <a href="https://crates.io/crates/sheldon">
    <img src="https://img.shields.io/crates/v/sheldon.svg" alt="Crates.io version" />
  </a>
  <a href="https://github.com/rossmacarthur/sheldon/releases/latest">
    <img src="https://img.shields.io/github/v/release/rossmacarthur/sheldon?label=download&scolor=yellow" alt="Download" />
  </a>
  <a href="https://github.com/rossmacarthur/sheldon/actions?query=workflow%3Abuild">
    <img src="https://img.shields.io/github/workflow/status/rossmacarthur/sheldon/build/master" alt="Build status" />
  </a>
  <a href="https://github.com/rossmacarthur/sheldon/actions?query=workflow%3Arelease">
    <img src="https://img.shields.io/github/workflow/status/rossmacarthur/sheldon/release?label=release" alt="Release status" />
  </a>
</div>

## Features

- Can manage virtually anything.
  - Any public Git repository.
    - Branch / tag / commit support.
    - Submodule support.
    - First class support for GitHub repositories.
    - First class support for Gists.
  - Arbitrary remote scripts or binaries.
  - Local plugins.
  - Inline plugins.
- Highly configurable install methods using [handlebars] templating.
- Shell agnostic, with sensible defaults for [Zsh].
- Super-fast parallel installation.
- Config file using [TOML] syntax.
- Uses a lock file for much faster loading of plugins.
- Clean `~/.zshrc` or `~/.bashrc` (just add 1 line).

## Table of Contents

- [Installation](#installation)
  - [Pre-built binaries](#pre-built-binaries)
  - [Cargo](#cargo)
  - [Release notes](#release-notes)
- [Getting started](#getting-started)
- [Command line interface](#command-line-interface)
  - [`lock` command](#lock-command)
  - [`source` command](#source-command)
  - [`init` command](#init-command)
  - [`add` command](#add-command)
  - [`edit` command](#edit-command)
  - [`remove` command](#remove-command)
  - [Flags](#flags)
  - [Options](#options)
- [Configuration: plugin sources](#configuration-plugin-sources)
  - [Git](#git)
    - [`github`](#github)
    - [`gist`](#gist)
    - [`git`](#git-1)
    - [Specifying a branch, tag, or commit](#specifying-a-branch-tag-or-commit)
    - [Cloning with Git or SSH protocols](#cloning-with-git-or-ssh-protocols)
    - [Private Git repositories](#private-git-repositories)
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
  - [`shell`](#shell)
  - [`match`](#match)
  - [`apply`](#apply-1)
- [Examples](#examples)
  - [Zsh frameworks](#zsh-frameworks)
    - [ohmyzsh](#ohmyzsh)
  - [Zsh plugins](#zsh-plugins)
    - [autosuggestions](#autosuggestions)
    - [autojump](#autojump)
    - [syntax-highlighting](#syntax-highlighting)
    - [blackbox](#blackbox)
    - [z.lua](#zlua)
    - [enhancd](#enhancd)
    - [base16](#base16)
  - [Zsh themes](#zsh-themes)
    - [powerlevel10k](#powerlevel10k)
    - [spaceship](#spaceship)
    - [pure](#pure)
- [License](#license)

## Installation

### Pre-built binaries

Pre-built binaries for Linux (x86-64, armv7) and macOS (x86-64) can be found on
[the releases page][releases].

Alternatively, the following script can be used to automatically detect your
host system, download the required artefact, and extract the **sheldon** binary.

```sh
curl --proto '=https' -fLsS https://rossmacarthur.github.io/install/crate.sh \
    | bash -s -- --repo "rossmacarthur/sheldon" --to /usr/local/bin
```

### Cargo

**sheldon** can be installed using [cargo], the Rust package manager. Install
[cargo] using [rustup] then run the following command to install or update
**sheldon**.

```sh
cargo install sheldon
```

### Release notes

Please see all release notes [here](RELEASES.md).

## Getting started

**sheldon** works by specifying all plugin information in a [TOML] configuration
file. Then sourcing the output of `sheldon source` in your `~/.zshrc` or
`~/.bashrc` file. When this command is run **sheldon** will download all the
required plugin sources, generate a lock file, and then output shell source.

By default the config file is located at `~/.sheldon/plugins.toml`. You can
either edit this file directly or use the provided command line interface to add
or remove plugins. To initialize this file run the following.

```sh
sheldon init --shell bash
```

or if you're using Zsh

```sh
sheldon init
```

To add your first plugin to the config file run the `sheldon add` command.

```sh
sheldon add oh-my-zsh --github "ohmyzsh/ohmyzsh"
```

The first argument given here `oh-my-zsh` is a unique name for the plugin. The
`--github` option specifies that we want **sheldon** to manage a clone of
http://github.com/ohmyzsh/ohmyzsh.

You can then use `sheldon source` to install the configured plugins, generate
the lock file, and print out the shell script to source. Simply add the
following to your `~/.zshrc` or `~/.bashrc` file.

```sh
# ~/.zshrc or ~/.bashrc

source <(sheldon source)
```

## Command line interface

### `lock` command

The `lock` command installs the plugins sources and generates the lock file.
Rerunning this command will not reinstall plugin sources, just check that they
are all okay. It will always regenerate the lock file.

```sh
sheldon lock
```

To force a reinstall of all plugin sources you can use the `--reinstall` flag.

```sh
sheldon lock --reinstall
```

### `source` command

This command generates the shell script. This command will first check if there
is an up to date lock file, if not, then it will first do the equivalent of the
lock command above. This command is usually used with the built-in shell
`source` command.

```sh
source <(sheldon source)
```

If we now modify our config file and run this command again it will relock the
configuration prior to generating the script. The output of this command is
highly configurable. You can define your own [custom
templates](#configuration-templates) to apply to your plugins.

### `init` command

This command initializes a new config file. If a config file exists then this
command is a noop.

For example

```sh
sheldon init
```

Or you can specify the shell.

```sh
sheldon init --shell bash
```

### `add` command

This command adds a new plugin to the config file. It does nothing else but edit
the config file. In the following command we add a GitHub repository as a
source.

```sh
sheldon add my-repo --git https://github.com/owner/repo.git
```

An example usage of this command for each source type is shown in the
[Configuration: plugin sources](#configuration-plugin-sources) section.

### `edit` command

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

### `remove` command

This command removes a plugin from the config file. In the following command we
remove the plugin with name `my-repo`.

```sh
sheldon remove my-repo
```

### Flags

**sheldon** accepts the following global command line flags.

| Flag              | Description                       |
| ----------------- | --------------------------------- |
| `-q`, `--quiet`   | Suppress any informational output |
| `-v`, `--verbose` | Use verbose output                |
| `--no-color`      | Do not use ANSI colored output    |
| `-h`, `--help`    | Show the help message and exit    |
| `-V`, `--version` | Show the version and exit         |

### Options

**sheldon** accepts the following global command line options.

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

## Configuration: plugin sources

A plugin is defined by adding a new unique name to the `plugins` table in the
[TOML] config file. This can be done by either editing the file directly or
using the provided **sheldon** commands. A plugin must the location of the
source. There are three types of sources, each kind is described in this
section. A plugin may only specify _one_ source type.

```toml
# ~/.sheldon/plugins.toml

#            ┌─ Unique name for the plugin
#        ┌───┴───┐
[plugins.oh-my-zsh]
github = "ohmyzsh/ohmyzsh"
#         └──┬──┘ └──┬──┘
#            │       └─ GitHub repository name
#            └─ GitHub user or organization
```

### Git

Git sources specify a remote Git repository that will be cloned to the
**sheldon** root directory. There are three flavors of Git sources.

#### `github`

A GitHub source must set the `github` field and specify the repository. This
should be the username or organization and the repository name separated by a
forward slash. Add the following to the **sheldon** config file.

```toml
[plugins.example]
github = "owner/repo"
```

Or run **sheldon add** with the `--github` option.

```sh
sheldon add example --github owner/repo
```

#### `gist`

A Gist source must set the `gist` field and specify the repository. This should
be the hash or username and hash of the Gist. Add the following to the
**sheldon** config file.

```toml
[plugins.example]
gist = "579d02802b1cc17baed07753d09f5009"
```

Or run **sheldon add** with the `--gist` option.

```sh
sheldon add example --gist 579d02802b1cc17baed07753d09f5009
```

#### `git`

A Git source must set the `git` field and specify the URL to clone. Add the
following to the **sheldon** config file.

```toml
[plugins.example]
git = "https://github.com/owner/repo"
```

Or run **sheldon add** with the `--git` option.

```sh
sheldon add example --git https://github.com/owner/repo
```

#### Specifying a branch, tag, or commit

All Git sources also allow setting of one of the `branch`, `tag` or `rev`
fields. **sheldon** will then checkout the repository at this reference.

```toml
[plugins.example]
github = "owner/repo"
tag = "v0.1.0"
```

Or run **sheldon add** with the `--tag`, `--branch`, or `--rev` option when
adding the plugin.

```sh
sheldon add example --github owner/repo --tag v0.1.0
```

#### Cloning with Git or SSH protocols

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

#### Private Git repositories

Currently **sheldon** only supports authentication when cloning using SSH and
only with authentication via the SSH agent. This means if you have a plugin
source that is a private repository you will have to use the SSH protocol for
cloning.

### Remote

Remote sources specify a remote file that will be downloaded to the **sheldon**
root directory. A Remote source must set the  `remote` field and specify the
URL. Add the following to the **sheldon** config file.

```toml
[plugins.example]
remote = "https://github.com/owner/repo/raw/master/plugin.zsh"
```

Or run **sheldon add** with the `--remote` option.

```sh
sheldon add example --remote https://github.com/owner/repo/raw/master/plugin.zsh
```

### Local

Local sources reference local directories. A Local source must set the `local`
field and specify a directory. Tildes may be used and will be expanded to the
current user's home directory. Add the following to the **sheldon** config file.

```toml
[plugins.example]
local = "~/Downloads/plugin"
```

Or run **sheldon add** with the `--local` option.

```sh
sheldon add example --local '~/Downloads/plugin'
```

## Configuration: plugin options

These are options that are common to all the above plugins.

### `use`

A list of files / globs to use in the plugin's source directory. If this field
is not given then the first pattern in the global [`match`](#match) field that
matches any files will be used. Add the following to the **sheldon** config
file.

```toml
[plugins.example]
github = "owner/repo"
use = ["*.zsh"]
```

Or run **sheldon add** with the `--use` option when adding the plugin.

```sh
sheldon add example --github owner/repo --use '*.zsh'
```

### `apply`

A list of template names to apply to this plugin. This defaults to the global
[`apply`](#apply-1).

```toml
[plugins.example]
github = "owner/repo"
apply = ["source", "PATH"]
```

Or run **sheldon add** with the `--apply` option when adding the plugin.

```sh
sheldon add example --github owner/repo --apply source PATH
```

You can define your own [custom templates](#custom-templates) to apply to your
plugins.

## Configuration: inline plugins

For convenience it also possible to define Inline plugins. An Inline plugin must
set the `inline` field and specify the raw source.

```toml
[plugins.example]
inline = 'example() { echo "Just an example of inline shell code" }'
```

## Configuration: templates

A template is a string that represents a generic action to take on a plugin. For
example the **PATH** template adds the plugin directory to the shell `PATH`
variable. A plugin will apply a template if you add the template name to the
[`apply`](#apply) field on a plugin.

Available built in templates are

- **source**: source each file in a plugin.
- **PATH**: add the plugin directory to the `PATH` variable.
- **path**: add the plugin directory to the `path` variable.
- **fpath**: add the plugin directory to the `fpath` variable.

As template strings they could be represented like this

```toml
[templates]
source = { value = 'source "{{ file }}"', each = true }
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

The `each` value, as used in the `source` template above, specifies that the
template should be applied to each matched file for the plugin. This defaults to
`false`.

### Custom templates

It is possible to create your own custom templates, and you can even override
the built in ones.

Plugins all have the following information that can be used in templates

- **A unique name.** This is completely arbitrary, and it is the value specified
  for the plugin in the plugins table. However, it is often the name of the
  plugin, so it can be useful to use this name in templates with `{{ name }}`.
- **A directory.** In git sources this is the location of the cloned repository,
  for local sources, it is the directory specified. This directory can be used
  in templates with `{{ dir }}`.
- **Zero or more files.** These are the matched files in the plugin directory
  either discovered using the the global `match` field or specified as a plugin
  option with `use`. These can be used in templates using `{{ file }}`.

You can use the following global information in templates

- **The sheldon root.** This folder can be used as `{{ root }}`.

### Example: symlinking files

Lets say we would like a template to symlink files into the
`~/.sheldon/functions` directory. We could create a new template with name
**function**, like this

```toml
[templates]
function = { value = 'ln -sf "{{ file }}" "~/.zsh/functions/{{ name }}"', each = true }
```

It can then be applied to the plugin like this

```toml
[plugins.example]
github = "owner/repo"
apply = ["function"]
```

### Example: overriding the PATH template

The built in **PATH** template adds the directory path to the beginning of the
`PATH` variable, we might want to change it to the be added at the end. We could
do this like this

```toml
[templates]
PATH = 'export PATH="$PATH:{{ dir }}"'
```

You can then apply it to the plugin like this

```toml
[plugins.example]
github = "owner/repo"
apply = ["source", "PATH"]
```

**Note:** this would change the behavior of **PATH** for *all* plugins using it.

## Configuration: global options

### `shell`

Indicates the shell that you are using **sheldon** with. If this field is set to
`bash` the global [`match`](#match) default configuration will use Bash relevant
defaults. If you are using Zsh you don't need to set this value but you may set
it to `zsh`. For example

```toml
shell = "bash"
```

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
    "*.zsh-theme"
]
```

If `shell = "bash"` then this defaults to

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

### `apply`

A list of template names to apply to all plugins by default (see
[`apply`](#apply)). This defaults to

```toml
apply = ["source"]
```

## Examples

This section demonstrates the configuration file contents for some popular
plugins and themes.

### Zsh frameworks

#### [ohmyzsh](https://github.com/ohmyzsh/ohmyzsh)

Add the following to the **sheldon** config file.

```toml
[plugins.oh-my-zsh]
github = "ohmyzsh/ohmyzsh"
````

Or run the following to automatically add it.

```sh
sheldon add oh-my-zsh --github "ohmyzsh/ohmyzsh"
```

Add the following to your `~/.zshrc` file.

```
# ~/.zshrc

export ZSH="$HOME/.sheldon/repos/github.com/ohmyzsh/ohmyzsh"

# Oh My Zsh settings here

source <(sheldon source)
```

### Zsh plugins

#### [autosuggestions](https://github.com/zsh-users/zsh-autosuggestions)

Add the following to the **sheldon** config file.

```toml
[plugins.zsh-autosuggestions]
github = "zsh-users/zsh-autosuggestions"
use = ["{{ name }}.zsh"]
```

Or run the following to automatically add it.

```sh
sheldon add zsh-autosuggestions --github zsh-users/zsh-autosuggestions --use '{{ name }}.zsh'
```

#### [autojump](https://github.com/wting/autojump)

Add the following to the **sheldon** config file.

```toml
[plugins.autojump]
github = "wting/autojump"
dir = "bin"
apply = ["PATH", "source"]
```

Or run the following to automatically add it.

```sh
sheldon add autojump --github wting/autojump --dir bin --apply PATH source
```

#### [syntax-highlighting](https://github.com/zsh-users/zsh-syntax-highlighting)

Add the following to the **sheldon** config file.

```toml
[plugins.zsh-syntax-highlighting]
github = "zsh-users/zsh-syntax-highlighting"
```

Or run the following to automatically add it.

```sh
sheldon add zsh-syntax-highlighting --github zsh-users/zsh-syntax-highlighting
```

#### [blackbox](https://github.com/StackExchange/blackbox)

Add the following to the **sheldon** config file.

```toml
[plugins.blackbox]
github = "StackExchange/blackbox"
```

Or run the following to automatically add it.

```sh
sheldon add blackbox --github StackExchange/blackbox
```

#### [z.lua](https://github.com/skywind3000/z.lua)

Add the following to the **sheldon** config file.

```toml
[plugins."z.lua"]
github = "skywind3000/z.lua"
```

Or run the following to automatically add it.

```sh
sheldon add z.lua --github skywind3000/z.lua
```

#### [enhancd](https://github.com/b4b4r07/enhancd)

Add the following to the **sheldon** config file.

```toml
[plugins.enhancd]
github = "b4b4r07/enhancd"
```

Or run the following to automatically add it.

```sh
sheldon add enhancd --github b4b4r07/enhancd
```

#### [base16](https://github.com/chriskempson/base16-shell)

Add the following to the **sheldon** config file.

```toml
[plugins.base16]
github = "chriskempson/base16-shell"
```

Or run the following to automatically add it.

```sh
sheldon add base16 --github chriskempson/base16-shell
```

### Zsh themes

#### [powerlevel10k](https://github.com/romkatv/powerlevel10k)

Add the following to the **sheldon** config file.

```toml
[plugins.powerlevel10k]
github = "romkatv/powerlevel10k"
```

Or run the following to automatically add it.

```
sheldon add powerlevel10k --github romkatv/powerlevel10k
```

#### [spaceship]( https://github.com/denysdovhan/spaceship-prompt)

Add the following to the **sheldon** config file.

```toml
[plugins.spaceship]
github = "denysdovhan/spaceship-prompt"
```

Or run the following to automatically add it.

```sh
sheldon add spaceship --github denysdovhan/spaceship-prompt
```

#### [pure](https://github.com/sindresorhus/pure)

Add the following to the **sheldon** config file.

```toml
[plugins.pure]
github = "sindresorhus/pure"
use = ["async.zsh", "pure.zsh"]
```

Or run the following to automatically add it.

```sh
sheldon add pure --github sindresorhus/pure --use async.zsh pure.zsh
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

[cargo]: https://doc.rust-lang.org/cargo/
[handlebars]: http://handlebarsjs.com
[releases]: https://github.com/rossmacarthur/sheldon/releases
[rust-lang]: https://www.rust-lang.org/
[rustup]: https://rustup.rs/
[TOML]: https://github.com/toml-lang/toml
[Zsh]: http://www.zsh.org/
