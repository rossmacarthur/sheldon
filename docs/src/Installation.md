# 📦 Installation

## Homebrew

Sheldon can be installed using Homebrew.

```sh
brew install sheldon
```

## Cargo

Sheldon can be installed from [Crates.io](https://crates.io/crates/sheldon)
using [Cargo](https://doc.rust-lang.org/cargo/), the Rust package manager.

```sh
cargo install sheldon
```

## Cargo BInstall

Sheldon can be installed using
[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall), which will
download the release artifacts directly from the GitHub release.

```sh
cargo binstall sheldon
```

## Pre-built binaries

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

## Building from source

Sheldon is written in Rust, so to install it from source you will first need to
install Rust and Cargo using [rustup](https://rustup.rs/). Then you can run the
following to build Sheldon.

```sh
git clone https://github.com/rossmacarthur/sheldon.git
cd sheldon
cargo build --release
```

The binary will be found at `target/release/sheldon`.
