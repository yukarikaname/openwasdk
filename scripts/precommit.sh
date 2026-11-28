#!/bin/sh
# What CI checks, run before the commit leaves the machine.
#
# The reasons are measured: fmt and clippy on the Windows target each failed a
# push in this repository, and both take seconds here. Host clippy cannot see the
# Windows-only crates (they are cfg'd out), so that pass runs before a push.
set -e
cd "$(git rev-parse --show-toplevel)"

echo "precommit: fmt"
cargo fmt --all -- --check

echo "precommit: clippy (host)"
cargo clippy --all-targets -q -- -D warnings

if [ "$1" = "--full" ]; then
    echo "precommit: clippy (windows target)"
    cargo clippy --workspace --all-targets --exclude xwinui \
        --target x86_64-pc-windows-msvc -q -- -D warnings
    echo "precommit: tests"
    cargo test -q --workspace
fi
