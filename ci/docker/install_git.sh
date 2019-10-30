#!/usr/bin/env sh

# Install the latest stable git.

set -ex

main() {
    echo "deb http://ppa.launchpad.net/git-core/ppa/ubuntu trusty main" \
        > /etc/apt/sources.list.d/git-core.list
    apt-key adv --keyserver keyserver.ubuntu.com --recv E1DD270288B4E6030699E45FA1715D88E1DF1F24
    apt-get update \
        -o Dir::Etc::sourcelist="sources.list.d/git-core.list" \
        -o Dir::Etc::sourceparts="-" \
        -o APT::Get::List-Cleanup="0"
    apt-get -y --no-install-recommends install git --upgrade
    rm -rf /var/lib/apt/lists/* /etc/apt/sources.list.d/git-core.list
}

main
