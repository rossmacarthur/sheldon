#!/usr/bin/env bash

# This script takes care of testing sheldon.

set -ex

main() {
    cargo fmt --verbose -- --check

    if [ "$TRAVIS_RUST_VERSION" = "stable" ]; then
        cargo clippy --verbose --all --all-targets --all-features -- -D warnings
    else
        cargo build --verbose --all --all-targets --all-features
    fi

    cargo test --verbose --all --all-features

    if [ "$TRAVIS_RUST_VERSION" = "stable" ]; then
        git diff --exit-code
    fi
}

main
