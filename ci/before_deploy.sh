#!/usr/bin/env bash

# This script takes care of building sheldon and packaging it for release.

set -ex

main() {
    local target=$(rustup target list | grep '(default)' | awk '{print $1}')

    if [ "$TARGET" != "$target" ]; then
        exit 1
    fi

    cargo build --release
    cp "target/release/$CRATE" "$CRATE-$TRAVIS_TAG-$TARGET"
}

main
