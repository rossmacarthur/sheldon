#!/usr/bin/env bash

build_kcov() {
    wget -q -O - https://github.com/SimonKagstrom/kcov/archive/v36.tar.gz | tar xz &&
    mv kcov-36 kcov-source && cd kcov-source &&
    mkdir build && cd build &&
    cmake .. && make &&
    make install DESTDIR=../../kcov-build &&
    cd ../.. && rm -r kcov-source
    KCOV="$PWD/kcov-build/usr/local/bin/kcov"
}

if [[ -z "$KCOV" ]]; then
  if command -v kcov; then
    KCOV="kcov"
   else
    build_kcov
  fi
fi

for file in target/debug/*-*; do
  mkdir -p "target/cov/$(basename $file)"
  $KCOV --exclude-pattern=/.cargo --verify "target/cov/$(basename $file)" "$file"
done
