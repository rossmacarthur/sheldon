# Configuration

The config file follows a [TOML] file format. Its fields are described in this
section. It consists of a list of plugins, a list of templates, and a few global
options. See an example config file [here](plugins.example.toml).

## Table of Contents

- [Table of Contents](#table-of-contents)
- [Plugin sources](#plugin-sources)
  - [Git](#git)
    - [`github`](#github)
    - [`gist`](#gist)
    - [`git`](#git)
    - [Specifying the branch / tag / commit](#specifying-the-branch--tag--commit)
  - [Remote](#remote)
  - [Local](#local)
- [Plugin options](#plugin-options)
  - [`use`](#use)
  - [`apply`](#apply)
- [Templates](#templates)
  - [Custom templates](#custom-templates)
  - [Example: symlinking files](#example-symlinking-files)
  - [Example: overriding the PATH template](#example-overriding-the-path-template)
- [Global options](#global-options)
  - [`match`](#match)
  - [`apply`](#apply-1)

## Plugin sources

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
github = 'sindresorhus/pure'
```

#### `gist`

A Gist source must set the `gist` field and specify the repository. This should
be the hash or username and hash of the Gist.

```toml
[plugins.pure]
gist = '579d02802b1cc17baed07753d09f5009'
```

#### `git`

A Git source must set the `git` field and specify the URL.

```toml
[plugins.pure]
git = 'https://github.com/sindresorhus/pure'
```

#### Specifying the branch / tag / commit

All Git sources also allow setting of one of the `branch`, `tag` or `revision`
fields. **sheldon** will then checkout the repository at this reference.

```toml
[plugins.pure]
github = 'sindresorhus/pure'
tag = '1.9.0'
```

### Remote

Remote sources specify a remote file that will be downloaded to the **sheldon**
root directory. A Remote source must set the  `remote` field and specify the
URL.

```toml
[plugins.pure]
remote = 'https://github.com/rossmacarthur/pure/raw/master/pure.zsh'
```

### Local

Local sources reference local directories. A Local source must set the `local`
field and specify a `directory`. Tildes may be used and will be expanded to the
current user's home directory.

```toml
[plugins.pure]
local = '~/Downloads/repositories/pure'
```

## Plugin options

These are options that are common to all plugins.

### `use`

A list of files / globs to use in the plugin's source directory.

```toml
[plugins.pure]
github = 'sindresorhus/pure'
use = ['*.zsh']
```

If this field is not given then the first pattern in the global `match` field
that matches any files will be used.

### `apply`

A list of template names to apply to this plugin. This defaults to the global
[`apply`](#apply-1).

```toml
[plugins.pure]
github = 'sindresorhus/pure'
apply = ['source', 'PATH']
```

You can define your own [custom templates](#custom-templates) to apply to your
plugins.

## Templates

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
source = 'github'
repository = 'sindresorhus/pure'
apply = ['PATH', 'fpath']
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
github = 'sindresorhus/pure'
apply = ['function']
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
github = 'sindresorhus/pure'
apply = ['source', 'PATH']
```

**Note:** this would change the behavior of **PATH** for *all* plugins using it.

## Global options

### `match`

A list of glob patterns to match against a plugin's contents. The first pattern
that matches any files will be used by default as a plugin's `use` field. This
defaults to

```toml
match = [
  '{{ name }}.plugin.zsh',
  '{{ name }}.zsh',
  '{{ name }}.sh',
  '{{ name }}.zsh-theme',
  '*.plugin.zsh',
  '*.zsh',
  '*.sh',
  '*.zsh-theme',
]
```

**Note:** if you are not using Zsh then you should probably change this setting.

### `apply`

A list of template names to apply to all plugins by default (see
[`apply`](#apply)). This defaults to

```toml
apply = ['source']
```

[TOML]: https://github.com/toml-lang/toml
