#!/usr/bin/env bash

# Test sheldon.

set -ex

main() {
    if [ "$LINT" = true ]; then
        cargo fmt --verbose -- --check
    fi

    if [ "$LINT" = true ] && [ "$TRAVIS_RUST_VERSION" = "stable" ]; then
        cross clippy --target "$TARGET" --verbose --all --all-targets --all-features -- -D warnings -D clippy::use_self
    else
        cross build --target "$TARGET" --verbose --all --all-targets --all-features
    fi

    cross test --target "$TARGET" --verbose --all --all-features

    if [ "$TRAVIS_RUST_VERSION" = "stable" ]; then
        git diff --exit-code
    fi
}

main
