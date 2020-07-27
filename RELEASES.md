# Releases

## 0.5.3

*Unreleased*

### Features

- [Add `init` command.][131576d] Adds a new command to **sheldon** which
  initializes a config file. Simply run `sheldon init`.
- [Add `shell` config key.][ed872e9] Indicates to **sheldon** what type of shell
  is being used. Certain other config values will have different defaults if
  this value is set.

[131576d]: https://github.com/rossmacarthur/sheldon/commit/131576dfddc53ec76e87ce6ee64326ae119a383c
[ed872e9]: https://github.com/rossmacarthur/sheldon/commit/ed872e9ca6e7ca23569ed68bcdc09d43c6973374

## 0.5.2

*Released on June 4th, 2020*

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

*Released on May 11th, 2020*

- [Using `--reinstall` on source command now implies `--relock`.][081f940]
- [Support aarch64 linux.][eb6aaf4]
- [Update Docker images to use OpenSSL 1.1.1g.][4b14975] This affects the
  shipped musl binaries which statically bundle OpenSSL.

[081f940]: https://github.com/rossmacarthur/sheldon/commit/081f940bc75711d3a587673178c738dd9ad40258
[eb6aaf4]: https://github.com/rossmacarthur/sheldon/commit/eb6aaf49bacbccff00359ec86135d1f4050a6d35
[4b14975]: https://github.com/rossmacarthur/sheldon/commit/4b14975238412a9ae83fcc9a202586bd725b331b

## 0.5.0

*Released on May 2nd, 2020*

### Features

- [Add `add` and `remove` commands to edit config.](140d171) These commands will
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

- [Add `edit` command.](5b63843) Adds a new command to **sheldon** which allows
  you to open the config file in the default editor. Simply run `sheldon edit`.
- [Add initial config file.](75a39b3) When running `add` or `edit` **sheldon**
  will attempt to initialize a new config file at
  [`~/.sheldon/plugins.toml`](src/plugins.toml).
- [Warn about unused config keys.](11ff287) Anytime **sheldon** loads the config
  file it will log warnings when there are unused config keys. Great for
  catching typos!

[11ff287]: https://github.com/rossmacarthur/sheldon/commit/11ff2875e5ecb04851b435c3e95fecbe7e453a97
[75a39b3]: https://github.com/rossmacarthur/sheldon/commit/75a39b398bb2982ef77e44bf1269cbd6f762bf99
[5b63843]: https://github.com/rossmacarthur/sheldon/commit/5b6384370128d410e82153b52c398dcfd1f8422c
[140d171]: https://github.com/rossmacarthur/sheldon/commit/140d17142d5be2b0653f559612aebffbd3c39ce1

### Breaking changes

- [Update default root directory and clone directory.](1845483) The root
  directory now defaults to `~/.sheldon` and the clone directory now defaults to
  `{root}/repos`. To migrate you should do the following:

  ```sh
  mv ~/.zsh ~/.sheldon
  mv ~/.sheldon/repositories ~/.sheldon/repos
  ```

  Or to preserve the old behavior you should export the following before running
  **sheldon**.

  ```sh
  export SHELDON_ROOT="$HOME/.zsh"
  export SHELDON_CLONE_DIR="$SHELDON_ROOT/repositories"
  ```

### Fixes

- [Download/clone sources to a temporary path first.](7293cbf)

  This fixes an issue ([#99]) where if someone tried to reinstall they would be
  left without any plugins because they would all be nuked up front prior to
  trying to download them.

[#99]: https://github.com/rossmacarthur/sheldon/issues/99
[7293cbf]: https://github.com/rossmacarthur/sheldon/commit/7293cbf61240a40333ef139b6ac6e7ab173f0f97

### Deprecations

Most of these are still supported, however **sheldon** will log a deprecation
warning if you use them.

- [Rename `filename` to `file`](c62600a) This affects both the the config file
  and the template rendering context.
- [Rename `directory` to `dir`.](f8d5647) This affects both the the config file
  and the template rendering context.
- [Rename `protocol` plugin config key to `proto`.](ce4d8e2)
- [Rename `revision` plugin config key to `rev`.](92bb588)

[1845483]: https://github.com/rossmacarthur/sheldon/commit/18454834cb6f1b2b1ebf2ef52617449b58917f28
[c62600a]: https://github.com/rossmacarthur/sheldon/commit/c62600a46116457c4bd682e348af344c00709e67
[f8d5647]: https://github.com/rossmacarthur/sheldon/commit/f8d564770e5fac66d92511fdb404869f2cbb6f4f
[ce4d8e2]: https://github.com/rossmacarthur/sheldon/commit/ce4d8e29e19d52a7ab9d5c83b93cfe91b643a227
[92bb588]: https://github.com/rossmacarthur/sheldon/commit/92bb588612e58498fe668e0f7f4fa274b6f9cb11

## 0.4.8

*Released on November 3rd, 2019*

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

*Released on October 22nd, 2019*

- [Add `--clone-dir` and `--download-dir` options.][#76] The directories where
  Git plugin sources are cloned to, and remote sources are downloaded to, are
  now configurable. Environment variables for setting these options are also
  now available.
- [Fix `--config-file` and `--lock-file` options.][#72] These two options were
  previously ignored and only the environment variables were recognized.

[#76]: https://github.com/rossmacarthur/sheldon/pull/76
[#72]: https://github.com/rossmacarthur/sheldon/pull/72

## 0.4.6

*Released on August 18th, 2019*

- [Support globs in local plugin directories.][#66] Globs should match only one
  directory.
- [Support for inline plugins.][#65]

[#66]: https://github.com/rossmacarthur/sheldon/pull/66
[#65]: https://github.com/rossmacarthur/sheldon/pull/65

## 0.4.5

*Released on July 19th, 2019*

- [Require mutex to run a `sheldon` commmand.][#58] Makes sure multiple
  instances of `sheldon` do not interfere with each other!

[#58]: https://github.com/rossmacarthur/sheldon/pull/58

## 0.4.4

*Released on July 7th, 2019*

- [Warn instead of erroring when running `sheldon source`.][#54] This allows
  at least some plugins to be sourced, this only happens if there is already a
  lock file.

[#54]: https://github.com/rossmacarthur/sheldon/pull/54

## 0.4.3

*Released on July 3rd, 2019*

- [Verify that locked directories and filenames exist when running
  `sheldon source`.][#47] If they do not then `sheldon lock` will be run again.

[#47]: https://github.com/rossmacarthur/sheldon/pull/47

## 0.4.2

*Released on June 27th, 2019*

- [Improve output granularity and add `--verbose` option.][#44]

[#44]: https://github.com/rossmacarthur/sheldon/pull/44

## 0.4.1

*Released on June 2nd, 2019*

- [Add `--no-color` option.][#43]
- [Replace home directory with tilde in output.][#43]
- [Support directory key for plugins.][#42] The plugin directory can now be
  configured to be a sub directory of the source.

[#43]: https://github.com/rossmacarthur/sheldon/pull/43
[#42]: https://github.com/rossmacarthur/sheldon/pull/42

## 0.4.0

*Released on May 26th, 2019*

Complete refactor including breaking changes to the configuration file from
prior versions.
