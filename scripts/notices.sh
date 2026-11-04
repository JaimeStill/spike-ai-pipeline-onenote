#!/usr/bin/env bash
# notices checks that THIRD-PARTY-NOTICES.md names exactly the crates the release binary links,
# at the versions Cargo.lock pins: every runtime crate of either release target, procedural
# macros and what only they use left out, and no crate the binary no longer links. A crate is
# named by a "## <crate> <version>: ..." section or a "| <crate> | <version> |" row. one2html,
# whose code is ported rather than linked as a crate, has a section of its own and is not
# compared. `mise run check` runs it; it reads only Cargo.lock's resolution and writes nothing.
set -euo pipefail
cd "$(dirname "$0")/.."

notices=THIRD-PARTY-NOTICES.md

linked=$(
	for target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
		cargo tree --locked --edges normal,no-proc-macro --target "$target" --prefix none \
			--format '{p}'
	done
)
linked=$(sed -nE 's/^([^ ]+) v([^ ]+)( \(\*\))?$/\1 \2/p' <<<"$linked" |
	grep -v '^spike-ai-pipeline-onenote ' | sort -u)

named=$(sed -nE \
	-e 's/^## ([A-Za-z0-9_-]+) ([0-9][^ :]*):.*$/\1 \2/p' \
	-e 's/^\| ([A-Za-z0-9_-]+) \| ([0-9][^ ]*) \|.*$/\1 \2/p' "$notices")
named=$(grep -v '^one2html ' <<<"$named" | sort -u)

missing=$(comm -23 <(echo "$linked") <(echo "$named"))
stale=$(comm -13 <(echo "$linked") <(echo "$named"))
while read -r crate; do
	[ -n "$crate" ] && echo "$notices: no notice for $crate, which the binary links" >&2
done <<<"$missing"
while read -r crate; do
	[ -n "$crate" ] && echo "$notices: a notice for $crate, which the binary doesn't link" >&2
done <<<"$stale"
[ -z "$missing" ] && [ -z "$stale" ]
