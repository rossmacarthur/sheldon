# Releases

## 0.6.2

*March 13th, 2021*

- [Fix edit bug.][a4a0602] If the edit file existed and you chose the "Abort"
  option the file would be deleted by mistake.
- [Always include details section in version output.][92a23b5] This was
  previously excluded if there was no Git information.

[a4a0602]: https://github.com/rossmacarthur/sheldon/commit/a4a06023f5ec582964fdcf3ad036998dace02616
[92a23b5]: https://github.com/rossmacarthur/sheldon/commit/92a23b5289c4c206a228e6bf11ce937c4649047b

## 0.6.1

*February 12th, 2021*

- [Embed Git and Rustc information in binary.][f3c7483] Git (if available) and
  Rustc information will now be displayed when passing `--version` to Sheldon.
- [Switch to `curl` instead of `reqwest`.][129490a] This significantly reduces
  compile time and binary size.
- [Fix temporary file issues when using `edit`.][#111] Now the same file is used
  for editing, if it already exists then Sheldon will prompt the user to either
  re-open it or overwrite.

[f3c7483]: https://github.com/rossmacarthur/sheldon/commit/f3c748324fce1a098fd00f9b645771e1164d0a53
[129490a]: https://github.com/rossmacarthur/sheldon/commit/129490a08e893f2313f2da902bc4b53bfcd0d42c
[#111]: https://github.com/rossmacarthur/sheldon/issues/111

## 0.6.0

*October 16th, 2020*

### Breaking changes

- [Support XDG directory structure.][#110] If any XDG environment variable
  is set then Sheldon will adopt the [XDG directory structure] by default. The
  config file will be located at `$XDG_CONFIG_HOME/sheldon/plugins.toml` and
  downloaded data will be located in `$XDG_CONFIG_DATA/sheldon`.
  Contributed by Andrew [**@tapeinosyne**](https://github.com/tapeinosyne).
- [Change the default lock file location.][10c64a3] For non-XDG directory
  structures the lock file now always defaults to
  `$SHELDON_DATA_DIR/plugins.lock`. It previously was the config file path with
  a `.lock` extension.
- [Remove the Sheldon root.][#112] The `root` directory has been replaced by
  individual directories for configuration and data: `config_dir` and
  `data_dir`. Both default to `$HOME/.sheldon`, the old default `root`.
  Contributed by Andrew [**@tapeinosyne**](https://github.com/tapeinosyne).

  If you used Sheldon's defaults, everything will just keep working as it did;
  no action needs to be taken. Otherwise, you may refer to this migration table:

  |                    | Old                   | New                         |
  | -----------------: | --------------------- | --------------------------- |
  |       Config paths | `<root>/plugins.toml` | `<config_dir>/plugins.toml` |
  |         Data paths | `<root>/plugins.lock` | `<data_dir>/plugins.lock`   |
  |                    | `<root>/repos`        | `<data_dir>/repos`          |
  |                    | `<root>/downloads`    | `<data_dir>/downloads`      |
  |      Env variables | `SHELDON_ROOT`        | `SHELDON_CONFIG_DIR`        |
  |                    |                       | `SHELDON_DATA_DIR`          |
  |        CLI options | `--root`              | `--config-dir`              |
  |                    |                       | `--data-dir`                |
  | Template variables | `{{ root }}`          | `{{ data_dir }}`            |

- [Auto-detect whether to use colored output.][2be1da7] A new `--color` option
  was added with three values `always`, `auto`, or `never`. By default Sheldon
  will now automatically whether to use colored output or not (`auto`). But you
  can still force Sheldon to always use color or never use color with the
  `--color always` option or `--color never`. The previous `--no-color` option
  has been removed.

[XDG directory structure]: https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html
[#110]: https://github.com/rossmacarthur/sheldon/pull/110
[#112]: https://github.com/rossmacarthur/sheldon/pull/112
[10c64a3]: https://github.com/rossmacarthur/sheldon/commit/10c64a3cd0e1f95536a821016a165728dde59779
[2be1da7]: https://github.com/rossmacarthur/sheldon/commit/2be1da71076247518d1f5c78c190d488bb8743cf

### Fixes

- [Fix performance bug introduced in version 0.5.4.][abf2027] A significant
  drop in performance was introduced by switching to the Rust `rayon` package.
  This change has been reverted.
- [Fix `--relock` not being implied for other flags.][bc5f9d7] This fixes a bug
  where passing `--update` or `--reinstall` to the `source` command didn't imply
  `--relock` like the documentation says.

[abf2027]: https://github.com/rossmacarthur/sheldon/commit/abf202737fa30b30465fbed6c47a034d4d0e9911
[bc5f9d7]: https://github.com/rossmacarthur/sheldon/commit/bc5f9d7ae759cf5e3af7822d45e2bf7fb545d219

## 0.5.4

*August 14th, 2020*

### Features

- [Support extended glob syntax.][a972c35] This means that `{a,b}` and `!`
  glob patterns can now be used. For example, the following is now valid.

  ```toml
  [plugins.ohmyzsh]
  github = "ohmyzsh/ohmyzsh"
  dir = "lib"
  use = ["{!git,!nvm,*}.zsh]
  ```

### Fixes

- [Fix not erroring out when no files matched for plugin.][aa69e0c] This fixes
  cases where no files would be matched for a plugin and Sheldon would silently
  continue, resulting in no source rendered by `sheldon source`.
- [Update default templates for Bash, `path` and `fpath` are now
  removed.][2a60788] These templates were meaningless in a Bash context.

[a972c35]: https://github.com/rossmacarthur/sheldon/commit/a972c3543d3c5ed339028ab316b1c48f733aed7
[aa69e0c]: https://github.com/rossmacarthur/sheldon/commit/aa69e0c6c6ae56e98c9d19de3be191e9ce9a974b
[2a60788]: https://github.com/rossmacarthur/sheldon/commit/2a607885ead211ec78e3494f32cad91476bd4184

## 0.5.3

*July 28th, 2020*

### Features

- [Add `init` command.][131576d] Adds a new command to Sheldon which initializes
  a config file. Simply run `sheldon init`.
- [Add `shell` config key.][ed872e9] Indicates to Sheldon what type of shell is
  being used. Certain other config values will have different defaults if this
  value is set.
- [Support updating of plugins via `--update` option.][5a8254d] Simply run
  `sheldon lock --update` to update all plugin sources.

[131576d]: https://github.com/rossmacarthur/sheldon/commit/131576dfddc53ec76e87ce6ee64326ae119a383c
[ed872e9]: https://github.com/rossmacarthur/sheldon/commit/ed872e9ca6e7ca23569ed68bcdc09d43c6973374
[5a8254d]: https://github.com/rossmacarthur/sheldon/commit/5a8254d36c73e79cf67782160cf63e9e6f5c3d9b

## 0.5.2

*June 4th, 2020*

### Fixes

- [Fix not erroring out on a bad HTTP status code.][4ae6432] This fixes remote
  sources from silently not being downloaded correctly.
- [Fix missing status log.][4ba5822] This fixes a missing status log for when
  fetching remote sources.

[4ae6432]: https://github.com/rossmacarthur/sheldon/commit/4ae64325fd8239dfb6e35f9efc05067c9c4a24d4
[4ba5822]: https://github.com/rossmacarthur/sheldon/commit/4ba58227be394e961d721804d7e6b02b882495ec

### Other

- [Only ship musl binaries.][3ef9d7a] The [download
  script](https://github.com/rossmacarthur/install) will figure this out
  automatically.

[3ef9d7a]: https://github.com/rossmacarthur/sheldon/commit/3ef9d7a7a8fd6b429e8405ba3e9ba7c621326543

## 0.5.1

*May 11th, 2020*

- [Using `--reinstall` on source command now implies `--relock`.][081f940]
- [Support aarch64 linux.][eb6aaf4]
- [Update Docker images to use OpenSSL 1.1.1g.][4b14975] This affects the
  shipped musl binaries which statically bundle OpenSSL.

[081f940]: https://github.com/rossmacarthur/sheldon/commit/081f940bc75711d3a587673178c738dd9ad40258
[eb6aaf4]: https://github.com/rossmacarthur/sheldon/commit/eb6aaf49bacbccff00359ec86135d1f4050a6d35
[4b14975]: https://github.com/rossmacarthur/sheldon/commit/4b14975238412a9ae83fcc9a202586bd725b331b

## 0.5.0

*May 2nd, 2020*

### Features

- [Add `add` and `remove` commands to edit config.][140d171] These commands will
  edit the config file for you.

  For example

  ```sh
  sheldon add example --github owner/repo --tag v0.1.0
  ```

  will add the following to the config file

  ```toml
  [plugins.example]
  github = "owner/repo"
  tag = "v0.1.0"
  ```

  The following will remove it again.

  ```sh
  sheldon remove example
  ```

- [Add `edit` command.][5b63843] Adds a new command to Sheldon which allows you
  to open the config file in the default editor. Simply run `sheldon edit`.
- [Add initial config file.][75a39b3] When running `add` or `edit` Sheldon will
  attempt to initialize a new config file at `~/.sheldon/plugins.toml`.
- [Warn about unused config keys.][11ff287] Anytime Sheldon loads the config
  file it will log warnings when there are unused config keys. Great for
  catching typos!

[11ff287]: https://github.com/rossmacarthur/sheldon/commit/11ff2875e5ecb04851b435c3e95fecbe7e453a97
[75a39b3]: https://github.com/rossmacarthur/sheldon/commit/75a39b398bb2982ef77e44bf1269cbd6f762bf99
[5b63843]: https://github.com/rossmacarthur/sheldon/commit/5b6384370128d410e82153b52c398dcfd1f8422c
[140d171]: https://github.com/rossmacarthur/sheldon/commit/140d17142d5be2b0653f559612aebffbd3c39ce1

### Breaking changes

- [Update default root directory and clone directory.][1845483] The root
  directory now defaults to `~/.sheldon` and the clone directory now defaults to
  `{root}/repos`. To migrate you should do the following:

  ```sh
  mv ~/.zsh ~/.sheldon
  mv ~/.sheldon/repositories ~/.sheldon/repos
  ```

  Or to preserve the old behavior you should export the following before running
  Sheldon.

  ```sh
  export SHELDON_ROOT="$HOME/.zsh"
  export SHELDON_CLONE_DIR="$SHELDON_ROOT/repositories"
  ```

### Fixes

- [Download/clone sources to a temporary path first.][7293cbf]

  This fixes an issue ([#99]) where if someone tried to reinstall they would be
  left without any plugins because they would all be nuked up front prior to
  trying to download them.

[#99]: https://github.com/rossmacarthur/sheldon/issues/99
[7293cbf]: https://github.com/rossmacarthur/sheldon/commit/7293cbf61240a40333ef139b6ac6e7ab173f0f97

### Deprecations

Most of these are still supported, however Sheldon will log a deprecation
warning if you use them.

- [Rename `filename` to `file`][c62600a] This affects both the the config file
  and the template rendering context.
- [Rename `directory` to `dir`.][f8d5647] This affects both the the config file
  and the template rendering context.
- [Rename `protocol` plugin config key to `proto`.][ce4d8e2]
- [Rename `revision` plugin config key to `rev`.][92bb588]

[1845483]: https://github.com/rossmacarthur/sheldon/commit/18454834cb6f1b2b1ebf2ef52617449b58917f28
[c62600a]: https://github.com/rossmacarthur/sheldon/commit/c62600a46116457c4bd682e348af344c00709e67
[f8d5647]: https://github.com/rossmacarthur/sheldon/commit/f8d564770e5fac66d92511fdb404869f2cbb6f4f
[ce4d8e2]: https://github.com/rossmacarthur/sheldon/commit/ce4d8e29e19d52a7ab9d5c83b93cfe91b643a227
[92bb588]: https://github.com/rossmacarthur/sheldon/commit/92bb588612e58498fe668e0f7f4fa274b6f9cb11

## 0.4.8

*November 3rd, 2019*

- [Auto clean clone and download directories][#87]. Unused source directories
  and files will now be automatically removed.
- [Support Git submodules.][#84] After cloning and checking out a repository
  submodules will now be recursively fetched.
- [Support Git source cloning using Git and SSH protocols.][#83] This adds an
  optional `protocol` field to plugin configuration which can be used to specify
  the protocol for Gist and GitHub sources. Additionally, Git sources can now
  specify URLs with protocols `git://` and `ssh://`.

[#87]: https://github.com/rossmacarthur/sheldon/pull/87
[#84]: https://github.com/rossmacarthur/sheldon/pull/84
[#83]: https://github.com/rossmacarthur/sheldon/pull/83

## 0.4.7

*October 22nd, 2019*

- [Add `--clone-dir` and `--download-dir` options.][#76] The directories where
  Git plugin sources are cloned to, and remote sources are downloaded to, are
  now configurable. Environment variables for setting these options are also now
  available.
- [Fix `--config-file` and `--lock-file` options.][#72] These two options were
  previously ignored and only the environment variables were recognized.

[#76]: https://github.com/rossmacarthur/sheldon/pull/76
[#72]: https://github.com/rossmacarthur/sheldon/pull/72

## 0.4.6

*August 18th, 2019*

- [Support globs in local plugin directories.][#66] Globs should match only one
  directory.
- [Support for inline plugins.][#65]

[#66]: https://github.com/rossmacarthur/sheldon/pull/66
[#65]: https://github.com/rossmacarthur/sheldon/pull/65

## 0.4.5

*July 19th, 2019*

- [Require mutex to run a Sheldon command.][#58] Makes sure multiple instances
  of Sheldon do not interfere with each other!

[#58]: https://github.com/rossmacarthur/sheldon/pull/58

## 0.4.4

*July 7th, 2019*

- [Warn instead of erroring when running `sheldon source`.][#54] This allows at
  least some plugins to be sourced, this only happens if there is already a lock
  file.

[#54]: https://github.com/rossmacarthur/sheldon/pull/54

## 0.4.3

*July 3rd, 2019*

- [Verify that locked directories and filenames exist when running `sheldon
  source`.][#47] If they do not then `sheldon lock` will be run again.

[#47]: https://github.com/rossmacarthur/sheldon/pull/47

## 0.4.2

*June 27th, 2019*

- [Improve output granularity and add `--verbose` option.][#44]

[#44]: https://github.com/rossmacarthur/sheldon/pull/44

## 0.4.1

*June 2nd, 2019*

- [Add `--no-color` option.][#43]
- [Replace home directory with tilde in output.][#43]
- [Support directory key for plugins.][#42] The plugin directory can now be
  configured to be a sub directory of the source.

[#43]: https://github.com/rossmacarthur/sheldon/pull/43
[#42]: https://github.com/rossmacarthur/sheldon/pull/42

## 0.4.0

*May 26th, 2019*

Complete refactor including breaking changes to the configuration file from
prior versions.
