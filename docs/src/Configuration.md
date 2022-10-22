# ⚙️ Configuration

## Plugin sources

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

### Git

Git sources specify a remote Git repository that will be cloned to the Sheldon
data directory. There are three flavors of Git sources.

#### `github`

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

#### `gist`

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

#### `git`

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

#### Specifying a branch, tag, or commit

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

Currently Sheldon only supports authentication when cloning using SSH and
requires an SSH agent to provide credentials. This means if you have a plugin
source that is a private repository you will have to use the SSH protocol for
cloning.

### Remote

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

### Local

Local sources reference local directories. A local source must set the `local`
field and specify a directory. Tildes may be used and will be expanded to the
current user's home directory. Add the following to the Sheldon config file.

```toml
[plugins.example]
local = "~/Downloads/plugin"
```

Or run `add` with the `--local` option.

```sh
sheldon add example --local '~/Downloads/plugin'
```

## Plugin options

These are options that are common to all the above plugins.

### `use`

A list of files / globs to use in the plugin's source directory. If this field
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

### `apply`

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

### `profiles`

A list of profiles this plugin should be used in. If this field is not given the
plugin will be used regardless of the profile. Otherwise, the plugin is only
used if the specified [profile](Command-line-interface.md#--profile-profile) is
included in the configured list of profiles.

### `hooks`

Statements executed around plugin installation.

```toml
[plugins.example]
github = "owner/repo"

[plugins.example.hooks]
pre = "export TEST=test"
post = "unset TEST"
```

## Inline plugins

For convenience it also possible to define Inline plugins. An Inline plugin must
set the `inline` field and specify the raw source.

```toml
[plugins.example]
inline = 'example() { echo "Just an example of inline shell code" }'
```

## Templates

A template defines how the shell source for a particular plugin is generated.
For example the **PATH** template adds the plugin directory to the shell `PATH`
variable. A template will be applied to a plugin if you add the template name to
the [`apply`](#apply) field on a plugin.

Available built-in templates are different depending on what shell you are
using. The following are available for both Bash and Zsh.

* **source**: source each file in a plugin.
* **PATH**: add the plugin directory to the `PATH` variable.

If you are using Zsh then the following are also available.

* **path**: add the plugin directory to the `path` variable.
* **fpath**: add the plugin directory to the `fpath` variable.

As template strings in the config file they could be represented like the
following.

```toml
[templates]
source = "{% if hooks | contains: \"pre\" %}{{ hooks.pre }}\n{% endif %}{% for file in files %}source \"{{ file }}\"\n{% endfor %}{% if hooks | contains: \"post\" %}\n{{ hooks.post }}{% endif %}"
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

### Custom templates

It is possible to create your own custom templates, and you can even override
the built-in ones.

Plugins all have the following information that can be used in templates.

* **A unique name.** This is completely arbitrary, and it is the value specified
  for the plugin in the plugins table. However, it is often the name of the
  plugin, so it can be useful to use this name in templates with `{{ name }}`.

* **A directory.** For Git sources this is the location of the cloned
  repository, for local sources, it is the directory specified. This directory
  can be used in templates with `{{ dir }}`.

* **One or more files.** These are the matched files in the plugin directory
  either discovered using the the global `match` field or specified as a plugin
  option with `use`. These can be used in templates by iterating over the files.
  For example: `{% for file in  files %} ... {{ file }} ... {% endfor %}`.

* **Hooks** Hooks are taken directly from the configuration and can be used as
  `{{ hooks.[KEY] }}`.

To add or update a template add a new key to the `[templates]` table in the
config file. Take a look at the [examples](Examples.md) for some interesting
applications of this.

## Global options

### `shell`

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

### `apply`

A list of template names to apply to all plugins by default (see
[`apply`](#apply)). This defaults to

```toml
apply = ["source"]
```
