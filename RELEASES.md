# Releases

## 0.4.8

*Unreleased*

- [Auto clean clone and download directories][#87]. Unused plugin source
  directories and files will now be automatically removed.
- [Support Git submodules.][#84] After cloning and checking out a repository
  submodules will now be recursively fetched.
- [Support Git source cloning using Git and SSH protocols.][#83] This adds an
  optional `protocol` field to plugin configuration which can be used to specify
  the protocol for Gist and GitHub sources. Additionally, Git sources can now
  specify URLs with protocols `git://` and `ssh://`.

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
