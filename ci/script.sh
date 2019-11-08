#!/usr/bin/env bash

# Test sheldon.

set -ex

main() {
    if [ "$LINT" = true ]; then
        cargo fmt --verbose -- --check
    fi

    if [ "$LINT" = true ] && [ "$TRAVIS_RUST_VERSION" = "stable" ]; then
        cross clippy --target "$TARGET" --verbose --all-targets --all-features -- \
            -D warnings -D clippy::use_self -D clippy::items-after-statements -D clippy::if-not-else
    else
        cross build --target "$TARGET" --verbose --all-targets --all-features
    fi

    cross test --target "$TARGET" --verbose --all-features

    if [ "$TRAVIS_RUST_VERSION" = "stable" ]; then
        git diff --exit-code
    fi
}

main
