# Installation

## Pre-built binaries

Pre-built binaries for Linux (x86-64, aarch64, armv7) and macOS (x86-64) are
provided. The following script can be used to automatically detect your host
system, download the required artefact, and extract the `sheldon` binary to the
given directory.

```sh
curl --proto '=https' -fLsS https://rossmacarthur.github.io/install/crate.sh \
    | bash -s -- --repo rossmacarthur/sheldon --to ~/.local/bin
```

Alternatively, you can download an artifact directly from the [the releases
page](https://github.com/rossmacarthur/sheldon/releases).

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
