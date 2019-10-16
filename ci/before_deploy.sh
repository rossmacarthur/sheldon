#!/usr/bin/env bash

# Build sheldon and package it for release.

set -ex

main() {
    local src="$PWD"
    local release="$src/dist"
    local release_bin="$src/target/$TARGET/release/$CRATE"

    trap 'rm -rf $release' ERR

    cross build --target "$TARGET" --release

    mkdir -p "$release/docs"
    cp "$release_bin" "$release"
    cp LICENSE* "$release"
    cp README.md "$release"
    cp docs/plugins.example.toml "$release/docs"

    cd "$release"
    tar cfz "$src/$CRATE-$TRAVIS_TAG-$TARGET.tar.gz" -- *
    cd "$src"

    rm -r "$release"
}

main
