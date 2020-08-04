# Installation

## Pre-built binaries

Pre-built binaries for Linux (x86-64, aarch64, armv7) and macOS (x86-64) can be
found on [the releases page](https://github.com/rossmacarthur/sheldon/releases).

Alternatively, the following script can be used to automatically detect your
host system, download the required artefact, and extract the **sheldon** binary.

```sh
curl --proto '=https' -fLsS https://rossmacarthur.github.io/install/crate.sh \
    | bash -s -- --repo "rossmacarthur/sheldon" --to ~/.local/bin
```

## Cargo

**sheldon** can be installed using [Cargo](https://doc.rust-lang.org/cargo/),
the Rust package manager. Install Cargo using [rustup](https://rustup.rs/) then
run the following command to install or update **sheldon**.

```sh
cargo install sheldon
```
