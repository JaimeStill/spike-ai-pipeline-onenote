#!/usr/bin/env bash
# release-pack builds the static musl release binary for one target, refuses to
# pack it unless its --version is the tag's version, and packs it with LICENSE
# and THIRD-PARTY-NOTICES.md as
# target/pack/spike-ai-pipeline-onenote-<tag>-<os>-<arch>.tar.gz, the asset name
# mise's github: backend matches by its os and arch. The release workflow runs
# it once per architecture, natively; it writes only target/.
#
# usage: scripts/release-pack.sh <target> <tag>
#   target  x86_64-unknown-linux-musl or aarch64-unknown-linux-musl
#   tag     vX.Y.Z
set -euo pipefail
cd "$(dirname "$0")/.."

if [ $# -ne 2 ]; then
	echo "usage: scripts/release-pack.sh <target> <tag>" >&2
	exit 2
fi
target=$1
tag=$2
case "$target" in
	x86_64-unknown-linux-musl) platform=linux-x64 ;;
	aarch64-unknown-linux-musl) platform=linux-arm64 ;;
	*)
		echo "release: no release platform for target $target" >&2
		exit 2
		;;
esac

name=spike-ai-pipeline-onenote
cargo build --release --locked --target "$target"
binary="target/$target/release/$name"

# The binary's version is Cargo.toml's, so the tag must match the version Cargo.toml records.
version=$("$binary" --version)
if [ "$version" != "${tag#v}" ]; then
	echo "release: the binary's version is $version, the tag's is ${tag#v}" >&2
	exit 1
fi

# The binary links onenote_parser (MPL-2.0), one2html's ported math code (MIT), and the other
# crates THIRD-PARTY-NOTICES.md lists, so every archive carries their notices beside LICENSE.
stage="target/pack/$platform"
rm -rf "$stage"
mkdir -p "$stage"
cp "$binary" LICENSE THIRD-PARTY-NOTICES.md "$stage/"
tar -czf "target/pack/$name-$tag-$platform.tar.gz" -C "$stage" "$name" LICENSE THIRD-PARTY-NOTICES.md
echo "target/pack/$name-$tag-$platform.tar.gz"
