#!/bin/bash

set -o errexit
set -o pipefail

PROGENITOR=${PROGENITOR:-progenitor}

NAME=keeper
VERSION=0.0.0

root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/cache"

sf="$root/cache/openapi.json"

cd "$root"
rm -f "$sf"
cargo run --release -p "$NAME-server" -- -S "$sf"
"$PROGENITOR" -i "$sf" -o "$root/openapi" -n "$NAME-openapi" -v "$VERSION"
