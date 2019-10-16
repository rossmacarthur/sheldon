#!/usr/bin/env bash

# Install all dependencies required to test sheldon.

set -ex

build_cross_docker_image() {
    # Build a Docker image for this target
    local dockerfile="ci/docker/Dockerfile.$TARGET"
    local tag="$CRATE:$TARGET"

    if [ -f "$dockerfile" ]; then
        docker build --tag "$tag" --file "$dockerfile" ci/docker
    fi
}

main() {
    if [ "$TARGET" = "x86_64-apple-darwin" ]; then
        local cross_release="v0.1.16/cross-v0.1.16-x86_64-apple-darwin.tar.gz"
    else
        local cross_release="v0.1.16/cross-v0.1.16-x86_64-unknown-linux-musl.tar.gz"
        build_cross_docker_image
    fi

    local url="https://github.com/rust-embedded/cross/releases/download/$cross_release"
    curl -fLsS "$url" | tar xz -C ~/.cargo/bin cross

    rustup self update
    rustup component add rustfmt
    rustup component add clippy
}

main
