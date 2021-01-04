#!/bin/bash

set -o errexit
set -o pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/cache"

sf="$root/cache/openapi.json"

ver="5.0.0"
sha256="839fade01e54ce1eecf012b8c33adb1413cff0cf2e76e23bc8d7673f09626f8e"
base="https://repo1.maven.org/maven2/org/openapitools/openapi-generator-cli"
url="$base/$ver/openapi-generator-cli-$ver.jar"
jar="$root/cache/openapi-generator-cli-$ver.jar"

while :; do
	if [[ -f "$jar" ]]; then
		actual=$(digest -a sha256 "$jar")
		if [[ $actual != $sha256 ]]; then
			printf 'actual %s != expected %s\n' "$actual" "$sha256"
			rm -f "$jar"
		else
			break
		fi
	fi

	printf 'downloading %s\n' "$url"
	if ! curl -o "$jar" -sSfL "$url"; then
		sleep 2
	fi
done

cd "$root"
rm -f "$sf"
cargo run --release -p keeper-server -- -S "$sf"
java -jar "$jar" generate -i "$sf" \
    -p packageVersion=0.0.0 \
    -p packageName=keeper-openapi \
    -g rust -o "$root/openapi"
